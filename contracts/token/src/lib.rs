//! CrowdfundX Token (CFX) — a SEP-41 style fungible token on Soroban.
//!
//! Powers the CrowdfundX platform: campaign contributions are made in CFX,
//! and the factory charges its creation fee in CFX. Implements the standard
//! transfer / transfer_from / approve / allowance surface plus admin-only
//! mint and burn.

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, Address,
    Env, String,
};

/// Roughly 5s per ledger on the Stellar network.
const DAY_IN_LEDGERS: u32 = 17_280;
/// Bump persistent entries to ~60 days of live time.
const PERSISTENT_BUMP: u32 = 60 * DAY_IN_LEDGERS;
/// Re-bump when remaining live time drops below ~45 days.
const PERSISTENT_THRESHOLD: u32 = 45 * DAY_IN_LEDGERS;

#[derive(Clone)]
#[contracttype]
pub struct AllowanceKey {
    pub from: Address,
    pub spender: Address,
}

#[derive(Clone)]
#[contracttype]
pub struct AllowanceValue {
    pub amount: i128,
    pub expiration_ledger: u32,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    Balance(Address),
    Allowance(AllowanceKey),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotAuthorized = 2,
    NegativeAmount = 3,
    InsufficientBalance = 4,
    InsufficientAllowance = 5,
    AllowanceExpired = 6,
}

// ---------------------------------------------------------------------------
// Events — streamable from RPC `getEvents`; fixed topics make client-side
// filtering cheap, and non-topic fields land in a named data map.
// ---------------------------------------------------------------------------

#[contractevent(topics = ["token", "transfer"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferEvent {
    #[topic]
    pub from: Address,
    #[topic]
    pub to: Address,
    pub amount: i128,
}

#[contractevent(topics = ["token", "mint"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MintEvent {
    #[topic]
    pub to: Address,
    pub amount: i128,
}

#[contractevent(topics = ["token", "burn"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BurnEvent {
    #[topic]
    pub from: Address,
    pub amount: i128,
}

#[contractevent(topics = ["token", "approve"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApproveEvent {
    #[topic]
    pub from: Address,
    #[topic]
    pub spender: Address,
    pub amount: i128,
    pub expiration_ledger: u32,
}

#[contractevent(topics = ["token", "set_admin"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetAdminEvent {
    pub new_admin: Address,
}

// ---------------------------------------------------------------------------
// Storage helpers
// ---------------------------------------------------------------------------

fn check_nonnegative(env: &Env, amount: i128) {
    if amount < 0 {
        panic_with_error!(env, Error::NegativeAmount);
    }
}

fn read_balance(env: &Env, owner: Address) -> i128 {
    let key = DataKey::Balance(owner);
    if let Some(amount) = env.storage().persistent().get(&key) {
        env.storage()
            .persistent()
            .extend_ttl(&key, PERSISTENT_THRESHOLD, PERSISTENT_BUMP);
        amount
    } else {
        0
    }
}

fn write_balance(env: &Env, owner: Address, amount: i128) {
    let key = DataKey::Balance(owner);
    env.storage().persistent().set(&key, &amount);
    env.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_THRESHOLD, PERSISTENT_BUMP);
}

fn read_allowance(env: &Env, from: &Address, spender: &Address) -> AllowanceValue {
    let key = DataKey::Allowance(AllowanceKey {
        from: from.clone(),
        spender: spender.clone(),
    });
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or(AllowanceValue {
            amount: 0,
            expiration_ledger: 0,
        })
}

fn write_allowance(env: &Env, from: &Address, spender: &Address, value: &AllowanceValue) {
    let key = DataKey::Allowance(AllowanceKey {
        from: from.clone(),
        spender: spender.clone(),
    });
    env.storage().persistent().set(&key, value);
    env.storage()
        .persistent()
        .extend_ttl(&key, PERSISTENT_THRESHOLD, PERSISTENT_BUMP);
}

fn spend_allowance(env: &Env, from: &Address, spender: &Address, amount: i128) {
    let allowance = read_allowance(env, from, spender);
    if allowance.expiration_ledger < env.ledger().sequence() {
        panic_with_error!(env, Error::AllowanceExpired);
    }
    if allowance.amount < amount {
        panic_with_error!(env, Error::InsufficientAllowance);
    }
    let remaining = allowance.amount - amount;
    if remaining == 0 {
        // Clean up zeroed allowances so storage doesn't accumulate garbage.
        let key = DataKey::Allowance(AllowanceKey {
            from: from.clone(),
            spender: spender.clone(),
        });
        env.storage().persistent().remove(&key);
    } else {
        write_allowance(
            env,
            from,
            spender,
            &AllowanceValue {
                amount: remaining,
                expiration_ledger: allowance.expiration_ledger,
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    /// Deploy-time constructor. Only callable during `contract deploy`.
    pub fn __constructor(env: Env, admin: Address, decimal: u32, name: String, symbol: String) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(env, Error::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_THRESHOLD, PERSISTENT_BUMP);
        // Name/symbol/decimals are accepted so tooling can read the intent
        // from the deployment transaction; the values themselves stay static
        // in the contract (single source of truth, see the getters below).
        let _ = (decimal, name, symbol);
    }

    // ------------------------------------------------------------------
    // Metadata (SEP-41)
    // ------------------------------------------------------------------

    pub fn decimals(_env: Env) -> u32 {
        7
    }

    pub fn name(env: Env) -> String {
        String::from_str(&env, "CrowdfundX Token")
    }

    pub fn symbol(env: Env) -> String {
        String::from_str(&env, "CFX")
    }

    // ------------------------------------------------------------------
    // Admin
    // ------------------------------------------------------------------

    pub fn set_admin(env: Env, new_admin: Address) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        env.events()
            .publish_event(&SetAdminEvent { new_admin });
    }

    pub fn admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    /// Admin-only mint. Used by the platform faucet flow during the demo.
    pub fn mint(env: Env, to: Address, amount: i128) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        check_nonnegative(&env, amount);
        let balance = read_balance(&env, to.clone());
        write_balance(&env, to.clone(), balance + amount);
        env.events().publish_event(&MintEvent { to, amount });
    }

    // ------------------------------------------------------------------
    // Transfer surface
    // ------------------------------------------------------------------

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        Self::execute_transfer(&env, &from, &to, amount);
    }

    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        spend_allowance(&env, &from, &spender, amount);
        Self::execute_transfer(&env, &from, &to, amount);
    }

    pub fn approve(env: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
        from.require_auth();
        check_nonnegative(&env, amount);
        write_allowance(
            &env,
            &from,
            &spender,
            &AllowanceValue {
                amount,
                expiration_ledger,
            },
        );
        env.events().publish_event(&ApproveEvent {
            from,
            spender,
            amount,
            expiration_ledger,
        });
    }

    pub fn allowance(env: Env, from: Address, spender: Address) -> i128 {
        let allowance = read_allowance(&env, &from, &spender);
        if allowance.expiration_ledger < env.ledger().sequence() {
            return 0;
        }
        allowance.amount
    }

    pub fn balance(env: Env, owner: Address) -> i128 {
        read_balance(&env, owner)
    }

    // ------------------------------------------------------------------
    // Burn surface
    // ------------------------------------------------------------------

    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();
        Self::execute_burn(&env, &from, amount);
    }

    pub fn burn_from(env: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();
        spend_allowance(&env, &from, &spender, amount);
        Self::execute_burn(&env, &from, amount);
    }

    // ------------------------------------------------------------------
    // Internal helpers (also exposed for SEP-41 compat tooling)
    // ------------------------------------------------------------------

    pub fn spend_allowance(env: Env, from: Address, spender: Address, amount: i128) {
        spend_allowance(&env, &from, &spender, amount);
    }

    fn execute_transfer(env: &Env, from: &Address, to: &Address, amount: i128) {
        check_nonnegative(env, amount);
        if amount == 0 {
            return;
        }
        let from_balance = read_balance(env, from.clone());
        if from_balance < amount {
            panic_with_error!(env, Error::InsufficientBalance);
        }
        let to_balance = read_balance(env, to.clone());
        write_balance(env, from.clone(), from_balance - amount);
        write_balance(env, to.clone(), to_balance + amount);
        env.events().publish_event(&TransferEvent {
            from: from.clone(),
            to: to.clone(),
            amount,
        });
    }

    fn execute_burn(env: &Env, from: &Address, amount: i128) {
        check_nonnegative(env, amount);
        if amount == 0 {
            return;
        }
        let balance = read_balance(env, from.clone());
        if balance < amount {
            panic_with_error!(env, Error::InsufficientBalance);
        }
        write_balance(env, from.clone(), balance - amount);
        env.events()
            .publish_event(&BurnEvent { from: from.clone(), amount });
    }
}

#[cfg(test)]
mod test;
