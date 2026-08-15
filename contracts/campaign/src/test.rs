#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _},
    vec, Event, String,
};

/// Built by the workspace before `cargo test` (see `make test` in the repo).
const TOKEN_WASM: &[u8] = include_bytes!("../../target/wasm32v1-none/release/cfx_token.wasm");

const DAY: u64 = 86_400;
const BASE_TIME: u64 = 1_752_600_000;

// NOTE: the test host clears the event buffer on every new top-level
// invocation, so event snapshots must be taken immediately after the call
// that emits them.

struct Setup {
    env: Env,
    creator: Address,
    token: Address,
    campaign: Address,
}

fn setup(goal: i128, deadline_offset: u64, milestones: &[i128]) -> Setup {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(BASE_TIME);

    let admin = Address::generate(&env);
    let token = env.register(
        TOKEN_WASM,
        (
            admin,
            7u32,
            String::from_str(&env, "CrowdfundX Token"),
            String::from_str(&env, "CFX"),
        ),
    );

    let creator = Address::generate(&env);
    let factory = Address::generate(&env); // dummy; the real factory is covered in its own crate
    let campaign = env.register(
        Campaign,
        CampaignArgs::__constructor(
            &String::from_str(&env, "Test Campaign"),
            &creator,
            &token,
            &factory,
            &goal,
            &(BASE_TIME + deadline_offset),
            &Vec::from_slice(&env, milestones),
        ),
    );

    Setup {
        env,
        creator,
        token,
        campaign,
    }
}

fn mint_token(env: &Env, token: &Address, to: &Address, amount: i128) {
    TokenClient::new(env, token).mint(to, &amount);
}

// ---------------------------------------------------------------------------
// Happy paths
// ---------------------------------------------------------------------------

#[test]
fn test_contribute_updates_state_and_emits_event() {
    let s = setup(1_000, DAY, &[1_000i128]);
    let env = &s.env;
    let campaign_client = CampaignClient::new(env, &s.campaign);
    let token_client = TokenClient::new(env, &s.token);
    let alice = Address::generate(env);

    mint_token(env, &s.token, &alice, 500);
    campaign_client.contribute(&alice, &300);
    let events = env.events().all().filter_by_contract(&s.campaign);

    let (config, milestones) = campaign_client.get_state();
    assert_eq!(config.status, Status::Active);
    assert_eq!(config.total_raised, 300);
    assert_eq!(milestones.len(), 1);
    assert_eq!(campaign_client.get_contribution(&alice), 300);

    // Vault received the tokens; contributor was debited.
    assert_eq!(token_client.balance(&s.campaign), 300);
    assert_eq!(token_client.balance(&alice), 200);

    // Contribution event emitted with correct topics + data.
    let expected = ContributionEvent {
        contributor: alice.clone(),
        amount: 300,
        total_raised: 300,
    }
    .to_xdr(env, &s.campaign);
    assert!(events.events().contains(&expected));
}

#[test]
fn test_goal_reached_and_milestone_release() {
    let s = setup(1_000, DAY, &[400i128, 600i128]);
    let env = &s.env;
    let campaign_client = CampaignClient::new(env, &s.campaign);
    let token_client = TokenClient::new(env, &s.token);
    let alice = Address::generate(env);
    let bob = Address::generate(env);

    mint_token(env, &s.token, &alice, 700);
    mint_token(env, &s.token, &bob, 500);

    // Alice contributes 600, bob 400 → goal reached.
    campaign_client.contribute(&alice, &600);
    campaign_client.contribute(&bob, &400);
    let funding_events = env.events().all().filter_by_contract(&s.campaign);

    let (config, _) = campaign_client.get_state();
    assert_eq!(config.status, Status::Funded);
    assert_eq!(config.total_raised, 1_000);

    // Vault holds the full goal.
    assert_eq!(token_client.balance(&s.campaign), 1_000);
    assert_eq!(token_client.balance(&alice), 100);
    assert_eq!(token_client.balance(&bob), 100);

    // Events from the funding phase: bob's contribution + goal_reached.
    let goal_event = GoalReachedEvent { total_raised: 1_000 }.to_xdr(env, &s.campaign);
    assert!(funding_events.events().contains(&goal_event));
    let contribution_event = ContributionEvent {
        contributor: bob.clone(),
        amount: 400,
        total_raised: 1_000,
    }
    .to_xdr(env, &s.campaign);
    assert!(funding_events.events().contains(&contribution_event));

    // Creator releases both milestones → payout + Closed.
    campaign_client.release_milestone(&0);
    let first_release_events = env.events().all().filter_by_contract(&s.campaign);
    campaign_client.release_milestone(&1);
    let second_release_events = env.events().all().filter_by_contract(&s.campaign);
    assert_eq!(token_client.balance(&s.creator), 1_000);

    let (config, milestones) = campaign_client.get_state();
    assert_eq!(config.status, Status::Closed);
    assert!(milestones.iter().all(|m| m.released));

    let release_event = MilestoneReleasedEvent {
        index: 1,
        amount: 600,
    }
    .to_xdr(env, &s.campaign);
    assert!(second_release_events.events().contains(&release_event));
    let first_release_event = MilestoneReleasedEvent {
        index: 0,
        amount: 400,
    }
    .to_xdr(env, &s.campaign);
    assert!(first_release_events.events().contains(&first_release_event));
}

#[test]
fn test_failed_campaign_refund_flow() {
    let s = setup(1_000, DAY, &[1_000i128]);
    let env = &s.env;
    let campaign_client = CampaignClient::new(env, &s.campaign);
    let token_client = TokenClient::new(env, &s.token);
    let alice = Address::generate(env);
    let bob = Address::generate(env);

    mint_token(env, &s.token, &alice, 300);
    mint_token(env, &s.token, &bob, 200);
    campaign_client.contribute(&alice, &300);
    campaign_client.contribute(&bob, &200);

    // Deadline passes under goal.
    env.ledger().set_timestamp(BASE_TIME + DAY + 1);
    campaign_client.close_failed();

    let (config, _) = campaign_client.get_state();
    assert_eq!(config.status, Status::Refunding);

    // Alice claims her refund; vault returns the tokens.
    let refunded = campaign_client.refund(&alice);
    let alice_refund_events = env.events().all().filter_by_contract(&s.campaign);
    assert_eq!(refunded, 300);
    assert_eq!(token_client.balance(&alice), 300);
    assert_eq!(campaign_client.get_refunded(&alice), 300);
    assert_eq!(campaign_client.get_contribution(&alice), 300);

    // Bob claims → campaign closes once fully refunded.
    campaign_client.refund(&bob);
    let bob_refund_events = env.events().all().filter_by_contract(&s.campaign);
    let (config, _) = campaign_client.get_state();
    assert_eq!(config.status, Status::Closed);
    assert_eq!(config.refunded_total, 500);

    let refund_event = RefundedEvent {
        contributor: bob.clone(),
        amount: 200,
    }
    .to_xdr(env, &s.campaign);
    assert!(bob_refund_events.events().contains(&refund_event));
    let alice_event = RefundedEvent {
        contributor: alice.clone(),
        amount: 300,
    }
    .to_xdr(env, &s.campaign);
    assert!(alice_refund_events.events().contains(&alice_event));
}

// ---------------------------------------------------------------------------
// Guards / failure paths
// ---------------------------------------------------------------------------

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn test_overfunding_rejected() {
    let s = setup(1_000, DAY, &[1_000i128]);
    let campaign_client = CampaignClient::new(&s.env, &s.campaign);
    let alice = Address::generate(&s.env);
    mint_token(&s.env, &s.token, &alice, 1_500);
    // Goal is 1000; a single 1500 contribution would overfund.
    campaign_client.contribute(&alice, &1_500);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn test_contribute_after_deadline_rejected() {
    let s = setup(1_000, DAY, &[1_000i128]);
    let campaign_client = CampaignClient::new(&s.env, &s.campaign);
    let alice = Address::generate(&s.env);
    mint_token(&s.env, &s.token, &alice, 100);
    s.env.ledger().set_timestamp(BASE_TIME + DAY + 1);
    campaign_client.contribute(&alice, &100);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_release_milestone_before_funded_rejected() {
    let s = setup(1_000, DAY, &[1_000i128]);
    let campaign_client = CampaignClient::new(&s.env, &s.campaign);
    campaign_client.release_milestone(&0);
}

#[test]
#[should_panic(expected = "Error(Contract, #11)")]
fn test_refund_before_failure_rejected() {
    let s = setup(1_000, DAY, &[1_000i128]);
    let campaign_client = CampaignClient::new(&s.env, &s.campaign);
    let alice = Address::generate(&s.env);
    mint_token(&s.env, &s.token, &alice, 100);
    campaign_client.contribute(&alice, &100);
    // Campaign is still Active — no refunds yet.
    campaign_client.refund(&alice);
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn test_milestones_must_sum_to_goal() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let token = env.register(
        TOKEN_WASM,
        (
            admin,
            7u32,
            String::from_str(&env, "CrowdfundX Token"),
            String::from_str(&env, "CFX"),
        ),
    );
    let creator = Address::generate(&env);
    let factory = Address::generate(&env);
    // 400 + 500 = 900 ≠ 1000 → constructor must reject.
    let _campaign = env.register(
        Campaign,
        CampaignArgs::__constructor(
            &String::from_str(&env, "Broken Milestones"),
            &creator,
            &token,
            &factory,
            &1_000i128,
            &(BASE_TIME + DAY),
            &vec![&env, 400i128, 500i128],
        ),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_release_unknown_milestone_rejected() {
    let s = setup(1_000, DAY, &[1_000i128]);
    let campaign_client = CampaignClient::new(&s.env, &s.campaign);
    let alice = Address::generate(&s.env);
    mint_token(&s.env, &s.token, &alice, 1_000);
    campaign_client.contribute(&alice, &1_000); // now Funded
    campaign_client.release_milestone(&5); // out of range
}

// ---------------------------------------------------------------------------
// Inter-contract robustness
// ---------------------------------------------------------------------------

/// The factory notification is best-effort: pointing the campaign at a
/// non-existent factory must NOT revert the user's contribution.
#[test]
fn test_factory_failure_does_not_revert_contribution() {
    let s = setup(1_000, DAY, &[1_000i128]);
    // s.factory is a generated address with no contract behind it, so the
    // `try_record_contribution` call inside `contribute` fails — and is
    // swallowed.
    let campaign_client = CampaignClient::new(&s.env, &s.campaign);
    let token_client = TokenClient::new(&s.env, &s.token);
    let alice = Address::generate(&s.env);
    mint_token(&s.env, &s.token, &alice, 250);

    campaign_client.contribute(&alice, &250);

    let (config, _) = campaign_client.get_state();
    assert_eq!(config.total_raised, 250);
    assert_eq!(token_client.balance(&s.campaign), 250);
}
