#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _, MockAuth, MockAuthInvoke},
    IntoVal,
    vec, Event, String,
};

/// Built by the workspace before `cargo test` (see `make test` in the repo).
const TOKEN_WASM: &[u8] = include_bytes!("../../target/wasm32v1-none/release/cfx_token.wasm");
const CAMPAIGN_WASM: &[u8] = include_bytes!("../../target/wasm32v1-none/release/cfx_campaign.wasm");

const DAY: u64 = 86_400;
const BASE_TIME: u64 = 1_752_600_000;

/// Client surface the factory test needs from a deployed campaign.
#[contractclient(name = "CampaignClient")]
pub trait CampaignTrait {
    fn contribute(env: &Env, from: &Address, amount: &i128);
}

/// Re-define the campaign's state shape locally so the factory test can read
/// it without depending on the campaign crate (contracts are independent
/// crates; we only share the wire format).
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum CampaignStatus {
    Active,
    Funded,
    Refunding,
    Closed,
}

#[derive(Clone)]
#[contracttype]
pub struct MilestoneState {
    pub amount: i128,
    pub released: bool,
}

#[derive(Clone)]
#[contracttype]
pub struct CampaignConfigState {
    pub name: String,
    pub creator: Address,
    pub token: Address,
    pub factory: Address,
    pub goal: i128,
    pub deadline: u64,
    pub total_raised: i128,
    pub refunded_total: i128,
    pub status: CampaignStatus,
}

#[contractclient(name = "StateClient")]
pub trait StateTrait {
    fn get_state(env: &Env) -> (CampaignConfigState, Vec<MilestoneState>);
}

struct Setup {
    env: Env,
    admin: Address,
    token: Address,
    factory: Address,
}

fn setup(creation_fee: i128) -> Setup {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(BASE_TIME);

    let admin = Address::generate(&env);
    let token = env.register(
        TOKEN_WASM,
        (
            admin.clone(),
            7u32,
            String::from_str(&env, "CrowdfundX Token"),
            String::from_str(&env, "CFX"),
        ),
    );

    let campaign_wasm_hash = env.deployer().upload_contract_wasm(CAMPAIGN_WASM);
    let factory = env.register(
        Factory,
        FactoryArgs::__constructor(&admin, &campaign_wasm_hash, &creation_fee),
    );

    Setup {
        env,
        admin,
        token,
        factory,
    }
}

#[test]
fn test_create_campaign_end_to_end_with_inter_contract_contribution() {
    let s = setup(0);
    let env = &s.env;
    let factory_client = FactoryClient::new(env, &s.factory);
    let token_client = TokenClient::new(env, &s.token);
    let creator = Address::generate(env);
    let contributor = Address::generate(env);

    // Creator deploys a campaign through the factory (inter-contract deploy).
    let campaign_addr = factory_client.create_campaign(
        &creator,
        &String::from_str(env, "Moonbase One"),
        &s.token,
        &1_000i128,
        &(BASE_TIME + 7 * DAY),
        &vec![env, 500i128, 500i128],
    );
    // NOTE: the test host clears the event buffer on each new invocation,
    // so snapshot events immediately after the call that emitted them.
    let factory_events = env.events().all().filter_by_contract(&s.factory);

    // Registry reflects the new campaign.
    let campaigns = factory_client.get_campaigns();
    assert_eq!(campaigns.len(), 1);
    assert_eq!(campaigns.get(0).unwrap().address, campaign_addr);
    let info = factory_client.get_campaign_info(&campaign_addr);
    assert_eq!(info.creator, creator);
    assert_eq!(info.goal, 1_000);
    assert_eq!(info.milestones.len(), 2);

    // Factory stats start at zero.
    let (count, total, fee) = factory_client.get_stats();
    assert_eq!(count, 1);
    assert_eq!(total, 0);
    assert_eq!(fee, 0);

    // Event on the factory side for the deployment.
    let created_event = CampaignCreatedEvent {
        campaign: campaign_addr.clone(),
        name: String::from_str(env, "Moonbase One"),
        creator: creator.clone(),
        token: s.token.clone(),
        goal: 1_000,
        deadline: BASE_TIME + 7 * DAY,
        milestones: vec![env, 500i128, 500i128],
        created_at: BASE_TIME,
    }
    .to_xdr(env, &s.factory);
    assert!(factory_events.events().contains(&created_event));

    // Contributor funds the campaign directly on the campaign contract.
    token_client.mint(&contributor, &1_000);
    CampaignClient::new(env, &campaign_addr).contribute(&contributor, &1_000);
    let factory_events_after_contribution = env.events().all().filter_by_contract(&s.factory);

    // Inter-contract callback propagated the contribution to the factory.
    let (_, total, _) = factory_client.get_stats();
    assert_eq!(total, 1_000);
    let tracked_event = ContributionTrackedEvent {
        campaign: campaign_addr.clone(),
        amount: 1_000,
        total_raised: 1_000,
    }
    .to_xdr(env, &s.factory);
    assert!(factory_events_after_contribution
        .events()
        .contains(&tracked_event));

    // The campaign reached its goal and the vault holds the tokens.
    let (campaign_state, milestones) = StateClient::new(env, &campaign_addr).get_state();
    assert_eq!(campaign_state.status, CampaignStatus::Funded);
    assert_eq!(campaign_state.total_raised, 1_000);
    assert_eq!(milestones.len(), 2);
    assert_eq!(token_client.balance(&campaign_addr), 1_000);
}

#[test]
fn test_creation_fee_is_charged_in_campaign_token() {
    let s = setup(10);
    let env = &s.env;
    let factory_client = FactoryClient::new(env, &s.factory);
    let token_client = TokenClient::new(env, &s.token);
    let creator = Address::generate(env);

    token_client.mint(&creator, &100);
    factory_client.create_campaign(
        &creator,
        &String::from_str(env, "Fee Test"),
        &s.token,
        &1_000i128,
        &(BASE_TIME + 7 * DAY),
        &vec![env, 1_000i128],
    );

    // Fee flows creator → factory vault.
    assert_eq!(token_client.balance(&creator), 90);
    assert_eq!(token_client.balance(&s.factory), 10);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn test_record_contribution_rejects_unregistered_caller() {
    let s = setup(0);
    let env = &s.env;
    let factory_client = FactoryClient::new(env, &s.factory);
    // Auth is mocked, but the address is not a registered campaign, so the
    // registry check rejects it.
    factory_client.record_contribution(&Address::generate(env), &100);
}

/// The invoker-contract auth check: `require_auth` on a contract address only
/// passes for the contract actually making the call. Here the caller is the
/// test harness, not the campaign, so auth must fail even though the address
/// is registered.
#[test]
#[should_panic]
fn test_record_contribution_rejects_non_invoker_contract() {
    let env = Env::default();
    env.ledger().set_timestamp(BASE_TIME);

    let admin = Address::generate(&env);
    let token = env.register(
        TOKEN_WASM,
        (
            admin.clone(),
            7u32,
            String::from_str(&env, "CrowdfundX Token"),
            String::from_str(&env, "CFX"),
        ),
    );
    let campaign_wasm_hash = env.deployer().upload_contract_wasm(CAMPAIGN_WASM);
    let factory = env.register(
        Factory,
        FactoryArgs::__constructor(&admin, &campaign_wasm_hash, &0i128),
    );

    let creator = Address::generate(&env);
    // Mock only the creator's create_campaign invocation (no mock_all_auths).
    env.mock_auths(&[MockAuth {
        address: &creator,
        invoke: &MockAuthInvoke {
            contract: &factory,
            fn_name: "create_campaign",
            args: soroban_sdk::vec![
                &env,
                creator.into_val(&env),
                String::from_str(&env, "Invoker Test").into_val(&env),
                token.into_val(&env),
                1_000i128.into_val(&env),
                (BASE_TIME + 7 * DAY).into_val(&env),
                soroban_sdk::Vec::from_slice(&env, &[1_000i128]).into_val(&env),
            ],
            sub_invokes: &[],
        },
    }]);

    let factory_client = FactoryClient::new(&env, &factory);
    let campaign_addr = factory_client.create_campaign(
        &creator,
        &String::from_str(&env, "Invoker Test"),
        &token,
        &1_000i128,
        &(BASE_TIME + 7 * DAY),
        &soroban_sdk::vec![&env, 1_000i128],
    );

    // The harness (not the campaign contract) calls in → require_auth on the
    // campaign's contract address has no invoker tracker → must fail.
    factory_client.record_contribution(&campaign_addr, &100);
}

#[test]
fn test_campaign_list_ordering_and_multiple_campaigns() {
    let s = setup(0);
    let env = &s.env;
    let factory_client = FactoryClient::new(env, &s.factory);
    let creator = Address::generate(env);

    let first = factory_client.create_campaign(
        &creator,
        &String::from_str(env, "First"),
        &s.token,
        &500i128,
        &(BASE_TIME + 7 * DAY),
        &vec![env, 500i128],
    );
    let second = factory_client.create_campaign(
        &creator,
        &String::from_str(env, "Second"),
        &s.token,
        &2_000i128,
        &(BASE_TIME + 14 * DAY),
        &vec![env, 2_000i128],
    );

    let campaigns = factory_client.get_campaigns();
    assert_eq!(campaigns.len(), 2);
    assert_eq!(campaigns.get(0).unwrap().address, first);
    assert_eq!(campaigns.get(1).unwrap().address, second);
    assert_ne!(first, second);

    let (count, _, _) = factory_client.get_stats();
    assert_eq!(count, 2);
}

#[test]
#[should_panic(expected = "Error(Contract, #4)")]
fn test_invalid_milestone_sum_rejected_before_deploy() {
    let s = setup(0);
    let env = &s.env;
    let factory_client = FactoryClient::new(env, &s.factory);
    let creator = Address::generate(env);
    // 400 + 500 ≠ 1000 → rejected without deploying anything.
    factory_client.create_campaign(
        &creator,
        &String::from_str(env, "Broken"),
        &s.token,
        &1_000i128,
        &(BASE_TIME + 7 * DAY),
        &vec![env, 400i128, 500i128],
    );
}
