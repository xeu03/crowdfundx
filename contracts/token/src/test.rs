#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events as _, Ledger as _, MockAuth, MockAuthInvoke},
    Event, IntoVal, String, vec,
};

fn setup() -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let contract_id = env.register(
        Token,
        TokenArgs::__constructor(
            &admin,
            &7u32,
            &String::from_str(&env, "CrowdfundX Token"),
            &String::from_str(&env, "CFX"),
        ),
    );
    (env, admin, contract_id)
}

#[test]
fn test_mint_transfer_and_balance() {
    let (env, _admin, contract_id) = setup();
    let client = TokenClient::new(&env, &contract_id);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.mint(&alice, &1000);
    client.transfer(&alice, &bob, &300);

    // NOTE: the test host clears the event buffer on each new invocation,
    // so events must be snapshotted immediately after the emitting call.
    let events = env.events().all().filter_by_contract(&contract_id);

    assert_eq!(client.balance(&alice), 700);
    assert_eq!(client.balance(&bob), 300);

    // Transfer event emitted with the expected topics + data map.
    let expected = TransferEvent {
        from: alice.clone(),
        to: bob.clone(),
        amount: 300,
    }
    .to_xdr(&env, &contract_id);
    assert!(events.events().contains(&expected));
}

#[test]
fn test_transfer_insufficient_balance_fails() {
    let (env, _admin, contract_id) = setup();
    let client = TokenClient::new(&env, &contract_id);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);

    client.mint(&alice, &50);
    // `try_transfer` returns the error instead of panicking.
    let result = client.try_transfer(&alice, &bob, &100);
    assert!(result.is_err());
    assert_eq!(client.balance(&alice), 50);
    assert_eq!(client.balance(&bob), 0);
}

#[test]
fn test_allowance_and_transfer_from() {
    let (env, _admin, contract_id) = setup();
    let client = TokenClient::new(&env, &contract_id);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let spender = Address::generate(&env);

    client.mint(&alice, &1000);
    let expiration = env.ledger().sequence() + 10_000;
    client.approve(&alice, &spender, &200, &expiration);
    let events = env.events().all().filter_by_contract(&contract_id);

    assert_eq!(client.allowance(&alice, &spender), 200);

    client.transfer_from(&spender, &alice, &bob, &150);
    assert_eq!(client.allowance(&alice, &spender), 50);
    assert_eq!(client.balance(&bob), 150);
    assert_eq!(client.balance(&alice), 850);

    // Approve event was emitted.
    let expected = ApproveEvent {
        from: alice.clone(),
        spender: spender.clone(),
        amount: 200,
        expiration_ledger: expiration,
    }
    .to_xdr(&env, &contract_id);
    assert!(events.events().contains(&expected));
}

#[test]
fn test_transfer_from_exceeds_allowance_fails() {
    let (env, _admin, contract_id) = setup();
    let client = TokenClient::new(&env, &contract_id);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let spender = Address::generate(&env);

    client.mint(&alice, &1000);
    let expiration = env.ledger().sequence() + 10_000;
    client.approve(&alice, &spender, &100, &expiration);

    let result = client.try_transfer_from(&spender, &alice, &bob, &101);
    assert!(result.is_err());
    assert_eq!(client.balance(&bob), 0);
}

#[test]
fn test_expired_allowance_returns_zero() {
    let (env, _admin, contract_id) = setup();
    let client = TokenClient::new(&env, &contract_id);
    let alice = Address::generate(&env);
    let spender = Address::generate(&env);

    client.mint(&alice, &1000);
    // Approve with an expiration at the current ledger...
    client.approve(&alice, &spender, &100, &env.ledger().sequence());
    // ...then advance past it.
    env.ledger()
        .set_sequence_number(env.ledger().sequence() + 1);

    // Storage still holds the raw value, but the getter reports 0.
    assert_eq!(client.allowance(&alice, &spender), 0);
}

#[test]
fn test_burn() {
    let (env, _admin, contract_id) = setup();
    let client = TokenClient::new(&env, &contract_id);
    let alice = Address::generate(&env);

    client.mint(&alice, &500);
    client.burn(&alice, &200);
    let events = env.events().all().filter_by_contract(&contract_id);

    assert_eq!(client.balance(&alice), 300);

    let expected = BurnEvent {
        from: alice.clone(),
        amount: 200,
    }
    .to_xdr(&env, &contract_id);
    assert!(events.events().contains(&expected));
}

/// Only the admin (explicitly mocked here) may mint; anyone else's call
/// fails the `require_auth` check.
#[test]
fn test_mint_by_non_admin_fails() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let outsider = Address::generate(&env);
    let contract_id = env.register(
        Token,
        TokenArgs::__constructor(
            &admin,
            &7u32,
            &String::from_str(&env, "CrowdfundX Token"),
            &String::from_str(&env, "CFX"),
        ),
    );
    // Mock only the admin's mint invocation.
    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &contract_id,
            fn_name: "mint",
            args: vec![&env, admin.into_val(&env), 1000i128.into_val(&env)],
            sub_invokes: &[],
        },
    }]);

    let client = TokenClient::new(&env, &contract_id);
    // Admin mint passes...
    client.mint(&admin, &1000);
    assert_eq!(client.balance(&admin), 1000);
    // ...but the outsider has no auth entry for `mint`.
    let result = client.try_mint(&outsider, &1000);
    assert!(result.is_err());
    assert_eq!(client.balance(&outsider), 0);
}

#[test]
fn test_transfer_by_unauthorized_address_fails() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let alice = Address::generate(&env);
    let bob = Address::generate(&env);
    let contract_id = env.register(
        Token,
        TokenArgs::__constructor(
            &admin,
            &7u32,
            &String::from_str(&env, "CrowdfundX Token"),
            &String::from_str(&env, "CFX"),
        ),
    );
    // Only the admin mint is authorized.
    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &contract_id,
            fn_name: "mint",
            args: vec![&env, alice.into_val(&env), 1000i128.into_val(&env)],
            sub_invokes: &[],
        },
    }]);

    let client = TokenClient::new(&env, &contract_id);
    client.mint(&alice, &1000);
    // Alice has balance but no auth entry for `transfer`.
    let result = client.try_transfer(&alice, &bob, &100);
    assert!(result.is_err());
    assert_eq!(client.balance(&alice), 1000);
}
