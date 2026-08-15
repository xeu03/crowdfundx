//! CrowdfundX Campaign — an all-or-nothing crowdfunding campaign.
//!
//! Lifecycle:
//!   Active → (goal reached by deadline) → Funded → milestones released by creator
//!   Active → (deadline passed under goal) → Refunding → contributors claim refunds
//!
//! Inter-contract communication:
//!   - Pulls CFX from contributors via the token contract (`TokenClient`).
//!   - Pushes payout / refund transfers from the campaign vault.
//!   - Notifies the factory of every contribution so platform-wide stats
//!     stay live (`FactoryClient::try_record_contribution`, best-effort —
//!     a platform-stats failure never reverts a user's contribution).
//!
//! Events (streamable from the RPC `getEvents` endpoint):
//!   `campaign/created`, `campaign/contributed`, `campaign/goal_reached`,
//!   `campaign/milestone_released`, `campaign/failed`, `campaign/refunded`,
//!   `campaign/extended`

#![no_std]

use soroban_sdk::{
    contract, contractclient, contracterror, contractevent, contractimpl, contracttype,
    panic_with_error, vec, Address, Env, IntoVal, Vec,
};

const DAY_IN_LEDGERS: u32 = 17_280;
const PERSISTENT_BUMP: u32 = 60 * DAY_IN_LEDGERS;
const PERSISTENT_THRESHOLD: u32 = 45 * DAY_IN_LEDGERS;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Clone)]
#[contracttype]
pub struct Milestone {
    pub amount: i128,
    pub released: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum Status {
    /// Raising, deadline in the future.
    Active,
    /// Goal reached — creator may release milestones.
    Funded,
    /// Deadline passed under goal — contributors may claim refunds.
    Refunding,
    /// All funds distributed / fully refunded. Terminal.
    Closed,
}

#[derive(Clone)]
#[contracttype]
pub struct CampaignConfig {
    pub creator: Address,
    pub token: Address,
    pub factory: Address,
    pub goal: i128,
    pub deadline: u64,
    pub total_raised: i128,
    pub refunded_total: i128,
    pub status: Status,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Config,
    Milestones,
    /// Amount a given contributor has contributed, in the campaign token.
    Contribution(Address),
    /// Amount a given contributor has already claimed back.
    RefundClaimed(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotActive = 2,
    DeadlinePassed = 3,
    DeadlineNotExtended = 4,
    NegativeAmount = 5,
    Overfunding = 6,
    InvalidMilestones = 7,
    MilestoneNotFound = 8,
    MilestoneReleased = 9,
    NotFunded = 10,
    NotRefundable = 11,
    NothingToRefund = 12,
    GoalNotReached = 13,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[contractevent(topics = ["campaign", "created"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatedEvent {
    pub goal: i128,
    pub deadline: u64,
}

#[contractevent(topics = ["campaign", "contributed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContributionEvent {
    #[topic]
    pub contributor: Address,
    pub amount: i128,
    pub total_raised: i128,
}

#[contractevent(topics = ["campaign", "goal_reached"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GoalReachedEvent {
    pub total_raised: i128,
}

#[contractevent(topics = ["campaign", "milestone_released"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneReleasedEvent {
    pub index: u32,
    pub amount: i128,
}

#[contractevent(topics = ["campaign", "failed"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignFailedEvent {
    pub total_raised: i128,
}

#[contractevent(topics = ["campaign", "refunded"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefundedEvent {
    #[topic]
    pub contributor: Address,
    pub amount: i128,
}

#[contractevent(topics = ["campaign", "extended"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeadlineExtendedEvent {
    pub new_deadline: u64,
}

// ---------------------------------------------------------------------------
// Cross-contract clients (inter-contract communication)
// ---------------------------------------------------------------------------

#[contractclient(name = "TokenClient")]
pub trait TokenTrait {
    fn transfer(env: &Env, from: &Address, to: &Address, amount: &i128);
    fn balance(env: &Env, owner: &Address) -> i128;
    /// Only used by tests and the demo scripts (admin-minted faucet).
    fn mint(env: &Env, to: &Address, amount: &i128);
}

#[contractclient(name = "FactoryClient")]
pub trait FactoryTrait {
    fn record_contribution(env: &Env, campaign: &Address, amount: &i128);
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct Campaign;

fn bump(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_THRESHOLD, PERSISTENT_BUMP);
}

fn read_config(env: &Env) -> CampaignConfig {
    bump(env, &DataKey::Config);
    env.storage().persistent().get(&DataKey::Config).unwrap()
}

fn write_config(env: &Env, config: &CampaignConfig) {
    env.storage().persistent().set(&DataKey::Config, config);
    bump(env, &DataKey::Config);
}

fn read_milestones(env: &Env) -> Vec<Milestone> {
    env.storage().persistent().get(&DataKey::Milestones).unwrap()
}

#[contractimpl]
impl Campaign {
    /// Deploy-time constructor, invoked by the factory when it deploys this
    /// campaign via `Deployer::deploy_v2`.
    pub fn __constructor(
        env: Env,
        creator: Address,
        token: Address,
        factory: Address,
        goal: i128,
        deadline: u64,
        milestones: Vec<i128>,
    ) {
        if env.storage().instance().has(&DataKey::Config) {
            panic_with_error!(env, Error::AlreadyInitialized);
        }
        if goal <= 0 {
            panic_with_error!(env, Error::NegativeAmount);
        }
        if deadline <= env.ledger().timestamp() {
            panic_with_error!(env, Error::DeadlinePassed);
        }
        // Milestones must be non-empty and sum exactly to the goal, so every
        // contributed token is accounted for by the payout schedule.
        let mut sum: i128 = 0;
        let mut parsed: Vec<Milestone> = Vec::new(&env);
        for amount in milestones.iter() {
            if amount <= 0 {
                panic_with_error!(env, Error::InvalidMilestones);
            }
            sum += amount;
            parsed.push_back(Milestone {
                amount,
                released: false,
            });
        }
        if sum != goal {
            panic_with_error!(env, Error::InvalidMilestones);
        }

        env.storage().persistent().set(&DataKey::Milestones, &parsed);
        write_config(
            &env,
            &CampaignConfig {
                creator,
                token,
                factory,
                goal,
                deadline,
                total_raised: 0,
                refunded_total: 0,
                status: Status::Active,
            },
        );
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_THRESHOLD, PERSISTENT_BUMP);
        env.events().publish_event(&CreatedEvent { goal, deadline });
    }

    // ------------------------------------------------------------------
    // Contribution flow
    // ------------------------------------------------------------------

    /// Contribute `amount` of the campaign token. Tokens are pulled from the
    /// contributor straight into the campaign vault (contract balance).
    pub fn contribute(env: Env, from: Address, amount: i128) {
        from.require_auth();
        if amount <= 0 {
            panic_with_error!(env, Error::NegativeAmount);
        }
        let mut config = read_config(&env);
        if config.status != Status::Active {
            panic_with_error!(env, Error::NotActive);
        }
        if env.ledger().timestamp() > config.deadline {
            panic_with_error!(env, Error::DeadlinePassed);
        }
        if config.total_raised + amount > config.goal {
            panic_with_error!(env, Error::Overfunding);
        }

        // Inter-contract call: vault receives tokens.
        TokenClient::new(&env, &config.token).transfer(
            &from,
            &env.current_contract_address(),
            &amount,
        );

        let contrib_key = DataKey::Contribution(from.clone());
        let prev: i128 = env
            .storage()
            .persistent()
            .get(&contrib_key)
            .unwrap_or(0i128);
        env.storage()
            .persistent()
            .set(&contrib_key, &(prev + amount));
        bump(&env, &contrib_key);

        config.total_raised += amount;
        let reached = config.total_raised >= config.goal;
        if reached {
            config.status = Status::Funded;
        }
        write_config(&env, &config);

        env.events().publish_event(&ContributionEvent {
            contributor: from,
            amount,
            total_raised: config.total_raised,
        });
        if reached {
            env.events()
                .publish_event(&GoalReachedEvent { total_raised: config.total_raised });
        }

        // Inter-contract call: keep platform-wide stats live. Best-effort —
        // a factory-side failure must never revert a user's contribution.
        let _ = FactoryClient::new(&env, &config.factory)
            .try_record_contribution(&env.current_contract_address(), &amount);
    }

    // ------------------------------------------------------------------
    // Creator flow (milestone payouts)
    // ------------------------------------------------------------------

    /// Creator releases one milestone of the payout schedule.
    pub fn release_milestone(env: Env, index: u32) {
        let mut config = read_config(&env);
        if config.status != Status::Funded {
            panic_with_error!(env, Error::NotFunded);
        }
        config.creator.require_auth_for_args(vec![
            &env,
            index.into_val(&env),
            env.ledger().timestamp().into_val(&env),
        ]);

        let mut milestones = read_milestones(&env);
        let milestone = match milestones.get(index) {
            Some(m) => m,
            None => panic_with_error!(env, Error::MilestoneNotFound),
        };
        if milestone.released {
            panic_with_error!(env, Error::MilestoneReleased);
        }
        milestones.set(index, Milestone {
            amount: milestone.amount,
            released: true,
        });
        env.storage()
            .persistent()
            .set(&DataKey::Milestones, &milestones);

        // Inter-contract call: pay the creator from the vault.
        TokenClient::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &config.creator,
            &milestone.amount,
        );

        let all_released = milestones.iter().all(|m| m.released);
        if all_released {
            config.status = Status::Closed;
        }
        write_config(&env, &config);

        env.events().publish_event(&MilestoneReleasedEvent {
            index,
            amount: milestone.amount,
        });
    }

    // ------------------------------------------------------------------
    // Refund flow (all-or-nothing)
    // ------------------------------------------------------------------

    /// Anyone may flag a failed campaign once its deadline has passed under
    /// goal. Flips the campaign into `Refunding` so contributors can claim.
    pub fn close_failed(env: Env) {
        let mut config = read_config(&env);
        if config.status != Status::Active {
            panic_with_error!(env, Error::NotActive);
        }
        if env.ledger().timestamp() <= config.deadline {
            panic_with_error!(env, Error::NotRefundable);
        }
        if config.total_raised >= config.goal {
            panic_with_error!(env, Error::GoalNotReached);
        }
        config.status = Status::Refunding;
        write_config(&env, &config);
        env.events().publish_event(&CampaignFailedEvent {
            total_raised: config.total_raised,
        });
    }

    /// A contributor claims their funds back from a failed campaign.
    /// Returns the amount refunded.
    pub fn refund(env: Env, contributor: Address) -> i128 {
        contributor.require_auth();
        let mut config = read_config(&env);
        if config.status != Status::Refunding {
            panic_with_error!(env, Error::NotRefundable);
        }

        let contrib_key = DataKey::Contribution(contributor.clone());
        let contributed: i128 = env
            .storage()
            .persistent()
            .get(&contrib_key)
            .unwrap_or(0i128);
        let claim_key = DataKey::RefundClaimed(contributor.clone());
        let claimed: i128 = env
            .storage()
            .persistent()
            .get(&claim_key)
            .unwrap_or(0i128);
        let owed = contributed - claimed;
        if owed <= 0 {
            panic_with_error!(env, Error::NothingToRefund);
        }

        // Inter-contract call: vault returns tokens to the contributor.
        TokenClient::new(&env, &config.token).transfer(
            &env.current_contract_address(),
            &contributor,
            &owed,
        );

        env.storage()
            .persistent()
            .set(&claim_key, &(claimed + owed));
        bump(&env, &claim_key);
        config.refunded_total += owed;
        if config.refunded_total >= config.total_raised {
            config.status = Status::Closed;
        }
        write_config(&env, &config);

        env.events().publish_event(&RefundedEvent {
            contributor,
            amount: owed,
        });
        owed
    }

    // ------------------------------------------------------------------
    // Admin / getters
    // ------------------------------------------------------------------

    /// Creator may extend the deadline while the campaign is still active.
    pub fn extend_deadline(env: Env, new_deadline: u64) {
        let mut config = read_config(&env);
        if config.status != Status::Active {
            panic_with_error!(env, Error::NotActive);
        }
        config.creator.require_auth_for_args(vec![
            &env,
            new_deadline.into_val(&env),
            env.ledger().timestamp().into_val(&env),
        ]);
        if new_deadline <= config.deadline {
            panic_with_error!(env, Error::DeadlineNotExtended);
        }
        config.deadline = new_deadline;
        write_config(&env, &config);
        env.events()
            .publish_event(&DeadlineExtendedEvent { new_deadline });
    }

    /// Full state snapshot for the frontend.
    pub fn get_state(env: Env) -> (CampaignConfig, Vec<Milestone>) {
        let config = read_config(&env);
        let milestones = read_milestones(&env);
        (config, milestones)
    }

    pub fn get_contribution(env: Env, contributor: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Contribution(contributor))
            .unwrap_or(0i128)
    }

    pub fn get_refunded(env: Env, contributor: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::RefundClaimed(contributor))
            .unwrap_or(0i128)
    }
}

#[cfg(test)]
mod test;
