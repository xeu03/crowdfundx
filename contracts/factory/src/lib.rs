//! CrowdfundX Factory — deploys and tracks campaign contracts on-chain.
//!
//! Inter-contract communication:
//!   - `create_campaign` deploys a fresh `Campaign` contract via the Soroban
//!     `Deployer` (`deploy_v2`) and passes constructor args (factory address,
//!     token, goal, deadline, milestone schedule).
//!   - Deployed campaigns call back into `record_contribution`, which only
//!     accepts callers that are registered campaigns — a contract-level
//!     authorization check, no wallet signature involved.
//!   - A creation fee is charged in the campaign's token and accumulates in
//!     the factory vault (platform economics).
//!
//! Events (streamable from the RPC `getEvents` endpoint):
//!   `factory/initialized`, `factory/config_updated`, `factory/campaign_created`,
//!   `factory/contribution_tracked`

#![no_std]

use soroban_sdk::{
    contract, contractclient, contracterror, contractevent, contractimpl, contracttype,
    panic_with_error, Address, BytesN, Env, Vec,
};

const DAY_IN_LEDGERS: u32 = 17_280;
const PERSISTENT_BUMP: u32 = 60 * DAY_IN_LEDGERS;
const PERSISTENT_THRESHOLD: u32 = 45 * DAY_IN_LEDGERS;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Clone)]
#[contracttype]
pub struct CampaignInfo {
    pub address: Address,
    pub creator: Address,
    pub token: Address,
    pub goal: i128,
    pub deadline: u64,
    pub milestones: Vec<i128>,
    pub created_at: u64,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,
    CampaignWasm,
    /// Ordered list of deployed campaign addresses.
    CampaignList,
    /// Per-campaign metadata.
    CampaignInfo(Address),
    /// Total number of campaigns deployed (drives deployment salts).
    Count,
    /// Platform-wide contributions across all campaigns.
    TotalRaised,
    /// Flat creation fee charged in the campaign's token (0 = disabled).
    CreationFee,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotAuthorized = 2,
    NegativeAmount = 3,
    InvalidMilestones = 4,
    CampaignNotFound = 5,
    UnauthorizedCaller = 6,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[contractevent(topics = ["factory", "initialized"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactoryInitializedEvent {
    pub admin: Address,
    pub campaign_wasm: BytesN<32>,
    pub creation_fee: i128,
}

#[contractevent(topics = ["factory", "config_updated"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigUpdatedEvent {
    pub campaign_wasm: BytesN<32>,
    pub creation_fee: i128,
}

#[contractevent(topics = ["factory", "campaign_created"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CampaignCreatedEvent {
    #[topic]
    pub campaign: Address,
    pub creator: Address,
    pub token: Address,
    pub goal: i128,
    pub deadline: u64,
    pub milestones: Vec<i128>,
    pub created_at: u64,
}

#[contractevent(topics = ["factory", "contribution_tracked"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContributionTrackedEvent {
    #[topic]
    pub campaign: Address,
    pub amount: i128,
    pub total_raised: i128,
}

// ---------------------------------------------------------------------------
// Cross-contract clients
// ---------------------------------------------------------------------------

#[contractclient(name = "TokenClient")]
pub trait TokenTrait {
    fn transfer(env: &Env, from: &Address, to: &Address, amount: &i128);
    fn balance(env: &Env, owner: &Address) -> i128;
    /// Only used by tests and the demo scripts (admin-minted faucet).
    fn mint(env: &Env, to: &Address, amount: &i128);
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------

#[contract]
pub struct Factory;

fn bump(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_THRESHOLD, PERSISTENT_BUMP);
}

#[contractimpl]
impl Factory {
    pub fn __constructor(env: Env, admin: Address, campaign_wasm: BytesN<32>, creation_fee: i128) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(env, Error::AlreadyInitialized);
        }
        if creation_fee < 0 {
            panic_with_error!(env, Error::NegativeAmount);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::CampaignWasm, &campaign_wasm);
        env.storage()
            .instance()
            .set(&DataKey::CreationFee, &creation_fee);
        env.storage().instance().set(&DataKey::Count, &0u32);
        env.storage()
            .persistent()
            .set(&DataKey::CampaignList, &Vec::<Address>::new(&env));
        env.storage().persistent().set(&DataKey::TotalRaised, &0i128);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_THRESHOLD, PERSISTENT_BUMP);
        env.events().publish_event(&FactoryInitializedEvent {
            admin,
            campaign_wasm,
            creation_fee,
        });
    }

    // ------------------------------------------------------------------
    // Admin surface
    // ------------------------------------------------------------------

    pub fn set_campaign_wasm(env: Env, wasm: BytesN<32>) {
        Self::require_admin(&env);
        env.storage()
            .instance()
            .set(&DataKey::CampaignWasm, &wasm);
        let fee: i128 = env.storage().instance().get(&DataKey::CreationFee).unwrap();
        env.events().publish_event(&ConfigUpdatedEvent {
            campaign_wasm: wasm,
            creation_fee: fee,
        });
    }

    pub fn set_creation_fee(env: Env, fee: i128) {
        Self::require_admin(&env);
        if fee < 0 {
            panic_with_error!(env, Error::NegativeAmount);
        }
        env.storage().instance().set(&DataKey::CreationFee, &fee);
        let wasm: BytesN<32> = env.storage().instance().get(&DataKey::CampaignWasm).unwrap();
        env.events().publish_event(&ConfigUpdatedEvent {
            campaign_wasm: wasm,
            creation_fee: fee,
        });
    }

    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
    }

    // ------------------------------------------------------------------
    // Campaign deployment (inter-contract deploy)
    // ------------------------------------------------------------------

    /// Deploys a new campaign contract and registers it with the platform.
    /// Returns the new campaign's contract address.
    pub fn create_campaign(
        env: Env,
        creator: Address,
        token: Address,
        goal: i128,
        deadline: u64,
        milestones: Vec<i128>,
    ) -> Address {
        creator.require_auth();
        if goal <= 0 {
            panic_with_error!(env, Error::NegativeAmount);
        }
        let mut sum: i128 = 0;
        for amount in milestones.iter() {
            if amount <= 0 {
                panic_with_error!(env, Error::InvalidMilestones);
            }
            sum += amount;
        }
        if sum != goal {
            panic_with_error!(env, Error::InvalidMilestones);
        }

        // Platform fee: charged in the campaign's token, held by the factory.
        let fee: i128 = env.storage().instance().get(&DataKey::CreationFee).unwrap();
        if fee > 0 {
            TokenClient::new(&env, &token).transfer(
                &creator,
                &env.current_contract_address(),
                &fee,
            );
        }

        // Deploy the campaign. The salt is the global campaign counter, so
        // addresses are deterministic per factory and never collide.
        let count: u32 = env.storage().instance().get(&DataKey::Count).unwrap();
        let mut salt_raw = [0u8; 32];
        salt_raw[28..32].copy_from_slice(&count.to_be_bytes());
        let wasm: BytesN<32> = env.storage().instance().get(&DataKey::CampaignWasm).unwrap();

        let campaign_addr = env
            .deployer()
            .with_current_contract(salt_raw)
            .deploy_v2(
                wasm,
                (
                    creator.clone(),
                    token.clone(),
                    env.current_contract_address(),
                    goal,
                    deadline,
                    milestones.clone(),
                ),
            );

        // Register the campaign.
        let info = CampaignInfo {
            address: campaign_addr.clone(),
            creator,
            token,
            goal,
            deadline,
            milestones,
            created_at: env.ledger().timestamp(),
        };
        let info_key = DataKey::CampaignInfo(campaign_addr.clone());
        env.storage().persistent().set(&info_key, &info);
        bump(&env, &info_key);

        let mut list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::CampaignList)
            .unwrap();
        list.push_back(campaign_addr.clone());
        env.storage().persistent().set(&DataKey::CampaignList, &list);
        bump(&env, &DataKey::CampaignList);
        env.storage().instance().set(&DataKey::Count, &(count + 1));

        env.events().publish_event(&CampaignCreatedEvent {
            campaign: campaign_addr.clone(),
            creator: info.creator.clone(),
            token: info.token.clone(),
            goal: info.goal,
            deadline: info.deadline,
            milestones: info.milestones.clone(),
            created_at: info.created_at,
        });
        campaign_addr
    }

    // ------------------------------------------------------------------
    // Stats (inter-contract callback)
    // ------------------------------------------------------------------

    /// Called by deployed campaigns after each contribution. Only registered
    /// campaign contracts may call this: `require_auth` on the contract
    /// address passes only for the invoking contract itself (invoker-contract
    /// authorization), and the registry check adds defense in depth.
    pub fn record_contribution(env: Env, campaign: Address, amount: i128) {
        campaign.require_auth();
        if !env
            .storage()
            .persistent()
            .has(&DataKey::CampaignInfo(campaign.clone()))
        {
            panic_with_error!(env, Error::CampaignNotFound);
        }
        if amount <= 0 {
            panic_with_error!(env, Error::NegativeAmount);
        }
        let total: i128 = env.storage().persistent().get(&DataKey::TotalRaised).unwrap();
        env.storage()
            .persistent()
            .set(&DataKey::TotalRaised, &(total + amount));
        env.events().publish_event(&ContributionTrackedEvent {
            campaign,
            amount,
            total_raised: total + amount,
        });
    }

    // ------------------------------------------------------------------
    // Getters
    // ------------------------------------------------------------------

    pub fn get_campaigns(env: Env) -> Vec<CampaignInfo> {
        let list: Vec<Address> = env
            .storage()
            .persistent()
            .get(&DataKey::CampaignList)
            .unwrap();
        let mut infos = Vec::new(&env);
        for address in list.iter() {
            if let Some(info) = env
                .storage()
                .persistent()
                .get(&DataKey::CampaignInfo(address))
            {
                infos.push_back(info);
            }
        }
        infos
    }

    pub fn get_campaign_info(env: Env, campaign: Address) -> CampaignInfo {
        match env
            .storage()
            .persistent()
            .get(&DataKey::CampaignInfo(campaign.clone()))
        {
            Some(info) => info,
            None => panic_with_error!(env, Error::CampaignNotFound),
        }
    }

    /// (campaign_count, total_raised, creation_fee)
    pub fn get_stats(env: Env) -> (u32, i128, i128) {
        let count: u32 = env.storage().instance().get(&DataKey::Count).unwrap();
        let total: i128 = env.storage().persistent().get(&DataKey::TotalRaised).unwrap();
        let fee: i128 = env.storage().instance().get(&DataKey::CreationFee).unwrap();
        (count, total, fee)
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    pub fn get_campaign_wasm(env: Env) -> BytesN<32> {
        env.storage().instance().get(&DataKey::CampaignWasm).unwrap()
    }
}

#[cfg(test)]
mod test;
