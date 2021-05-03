use cosmwasm_std::{Coin, HumanAddr, ReadonlyStorage, StdError, StdResult, Storage, VoteOption};

use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::cashmap::{CashMap, ReadOnlyCashMap};
use cargo_common::balances::Balances;
use rust_decimal::Decimal;
//use secret_toolkit::storage::{AppendStore, AppendStoreMut, TypedStore, TypedStoreMut};
use std::convert::TryFrom;

pub const MAX_WITHDRAW_AMOUNT: u32 = 10;

pub const INDEXES: &[u8] = b"indexes";
pub const PREFIX_CONFIG: &[u8] = b"config";
pub const KEY_VALIDATOR_SET: &[u8] = b"validator_address";

pub static CONFIG_KEY: &[u8] = b"config";
pub const CONTRACT_ADDRESS: &[u8] = b"contract_address";
pub const FROZEN_EXCHANGE_RATE: &[u8] = b"FROZEN_EXCHANGE_RATE";
pub const PENDING_WITHDRAW: &[u8] = b"PENDING_WITHDRAW";
pub const VOTES: &[u8] = b"VOTES";
pub const VOTE_TOTALS: &[u8] = b"VOTE_TOTALS";

pub fn u32_to_vote_option(num: u32) -> VoteOption {
    match num {
        0 => VoteOption::Abstain,
        1 => VoteOption::NoWithVeto,
        2 => VoteOption::No,
        3 => VoteOption::Yes,
        _ => panic!(),
    }
}

pub fn vote_option_to_u32(option: VoteOption) -> u32 {
    match option {
        VoteOption::Abstain => 0,
        VoteOption::NoWithVeto => 1,
        VoteOption::No => 2,
        VoteOption::Yes => 3,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SingleVote {
    pub address: HumanAddr,
    pub vote: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Votes {
    pub proposal_id: u64,
    pub votes: Vec<HumanAddr>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct VoteTotals {
    pub yes: u128,
    pub no: u128,
    pub abstain: u128,
    pub no_with_veto: u128,
    pub counted_votes: u32,
    pub threshold: u32,
}

impl VoteTotals {
    pub fn winner(&self) -> VoteOption {
        if self.yes >= self.no && self.yes >= self.abstain && self.yes >= self.no_with_veto {
            return VoteOption::Yes;
        }

        if self.abstain >= self.no && self.abstain >= self.yes && self.abstain >= self.no_with_veto
        {
            return VoteOption::Abstain;
        }

        if self.no >= self.yes && self.no >= self.abstain && self.no >= self.no_with_veto {
            return VoteOption::No;
        }

        VoteOption::NoWithVeto
    }

    pub fn load<S: Storage>(store: &S, proposal_id: u64) -> Self {
        let cashmap = ReadOnlyCashMap::init(VOTE_TOTALS, store);

        cashmap.get(&proposal_id.to_be_bytes()).unwrap_or_default()
    }

    pub fn store<S: Storage>(self, store: &mut S, proposal_id: u64) -> StdResult<()> {
        let mut cashmap = CashMap::init(VOTE_TOTALS, store);

        cashmap.insert(&proposal_id.to_be_bytes(), self)
    }

    pub fn done(&self) -> bool {
        self.counted_votes >= self.threshold
    }
}

impl Votes {
    pub fn tally<S: Storage>(
        store: &mut S,
        proposal_id: u64,
        balances: &Balances,
    ) -> StdResult<Option<VoteOption>> {
        let mut vote_totals = VoteTotals::load(store, proposal_id);
        let cashmap = CashMap::init(&[VOTES, &proposal_id.to_be_bytes()].concat(), store);

        if vote_totals.counted_votes == 0 {
            vote_totals.threshold = cashmap.len()
        }

        for address in &balances.0 {
            //let vote = Votes::get(store, proposal_id, &address.account)?;
            let vote: Option<SingleVote> = cashmap.get(address.account.0.as_bytes());

            if let Some(_vote) = vote {
                match u32_to_vote_option(_vote.vote) {
                    VoteOption::Yes => vote_totals.yes += address.amount,
                    VoteOption::No => vote_totals.no += address.amount,
                    VoteOption::Abstain => vote_totals.abstain += address.amount,
                    VoteOption::NoWithVeto => vote_totals.no_with_veto += address.amount,
                }
                vote_totals.counted_votes += 1;
            }
        }

        let result = if vote_totals.done() {
            Ok(Some(vote_totals.winner()))
        } else {
            Ok(None)
        };

        vote_totals.store(store, proposal_id)?;

        return result;
    }

    pub fn get_voters<S: Storage>(
        store: &S,
        proposal_id: u64,
        page: u32,
        page_size: u32,
    ) -> StdResult<Vec<HumanAddr>> {
        let cashmap = ReadOnlyCashMap::init(&[VOTES, &proposal_id.to_be_bytes()].concat(), store);

        let voters: Vec<SingleVote> = cashmap.paging(page, page_size)?;

        Ok(voters.iter().map(|vote| vote.address.clone()).collect())
    }

    pub fn set<S: Storage>(storage: &mut S, proposal_id: u64, vote: SingleVote) -> StdResult<()> {
        let mut cashmap = CashMap::init(&[VOTES, &proposal_id.to_be_bytes()].concat(), storage);
        cashmap.insert(vote.address.0.as_bytes(), vote.clone())
    }

    pub fn get<S: Storage>(store: &S, proposal_id: u64, address: &HumanAddr) -> Option<SingleVote> {
        let cashmap = ReadOnlyCashMap::init(&[VOTES, &proposal_id.to_be_bytes()].concat(), store);
        cashmap.get(&address.0.as_bytes())
    }

    pub fn len<S: Storage>(store: &S, proposal_id: u64) -> u32 {
        let cashmap = ReadOnlyCashMap::<SingleVote, S>::init(
            &[VOTES, &proposal_id.to_be_bytes()].concat(),
            store,
        );
        cashmap.len()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct PendingWithdraw {
    pub available_time: u64,
    pub receiver: HumanAddr,
    pub coins: Coin,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PendingWithdraws(Vec<PendingWithdraw>);

impl PendingWithdraws {
    pub(crate) fn append(&mut self, withdraw: PendingWithdraw) {
        self.0.push(withdraw)
    }

    /// same as get_expired, but also modifies itself - this is used in handles
    pub(crate) fn remove_expired(&mut self, current_time: u64) -> Vec<PendingWithdraw> {
        self.0
            .drain_filter(|item| item.available_time <= current_time)
            .collect::<Vec<_>>()
    }

    pub fn len(&self) -> usize {
        return self.0.len();
    }

    /// get all expired (matured) withdraws. Can be used in queries since it does not modify the inner
    /// structure
    pub fn get_expired(&self, current_time: u64) -> Vec<PendingWithdraw> {
        self.0
            .clone()
            .drain_filter(|item| item.available_time <= current_time)
            .collect::<Vec<_>>()
    }

    pub(crate) fn pending(&self) -> Vec<PendingWithdraw> {
        let pending: Vec<PendingWithdraw> = self.0.clone();

        pending
    }

    pub(crate) fn save<S: Storage>(self, storage: &mut S, address: &HumanAddr) -> StdResult<()> {
        let mut cashmap = CashMap::init(&PENDING_WITHDRAW, storage);

        if self.0.len() == 0 {
            cashmap.remove(&address.0.as_bytes())
        } else {
            cashmap.insert(&address.0.as_bytes(), self)
        }
    }

    pub(crate) fn load<S: Storage>(storage: &S, address: &HumanAddr) -> Self {
        let cashmap = ReadOnlyCashMap::init(&PENDING_WITHDRAW, storage);

        let withdraws = cashmap.get(&address.0.as_bytes());

        withdraws.unwrap_or_default()
    }

    pub(crate) fn get_multiple<S: Storage>(storage: &mut S, amount: u32) -> StdResult<Vec<Self>> {
        let cashmap = CashMap::<PendingWithdraws, _>::init(&PENDING_WITHDRAW, storage);

        let mut withdraws: Vec<Self> = vec![];

        let values = cashmap.paging(0, amount)?;

        for value in values.iter() {
            withdraws.push(value.clone());
        }

        Ok(withdraws)
    }

    pub(crate) fn append_withdraw<S: Storage>(
        storage: &mut S,
        withdraw: &PendingWithdraw,
        address: &HumanAddr,
    ) -> StdResult<()> {
        let mut cashmap = CashMap::init(&PENDING_WITHDRAW, storage);

        let withdraws = cashmap.get(&address.0.as_bytes());

        if withdraws.is_some() {
            let mut new_withdraws: PendingWithdraws = withdraws.unwrap();

            if new_withdraws.len() >= MAX_WITHDRAW_AMOUNT as usize {
                return Err(StdError::generic_err(format!(
                    "Cannot have more than {} pending withdraws",
                    MAX_WITHDRAW_AMOUNT
                )));
            }

            new_withdraws.append(withdraw.clone());
            cashmap.insert(&address.0.as_bytes(), new_withdraws)?;
        } else {
            let mut new_withdraws = PendingWithdraws::default();
            new_withdraws.append(withdraw.clone());
            cashmap.insert(&address.0.as_bytes(), new_withdraws)?;
        }

        Ok(())
    }
}

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq)]
pub enum KillSwitch {
    Closed,
    Unbonding,
    Open,
}

impl TryFrom<u8> for KillSwitch {
    type Error = StdError;

    fn try_from(other: u8) -> Result<Self, Self::Error> {
        match other {
            0 => Ok(Self::Closed),
            1 => Ok(Self::Unbonding),
            2 => Ok(Self::Open),
            _ => Err(StdError::generic_err("Failed to convert killswitch enum")),
        }
    }
}

impl Into<u8> for KillSwitch {
    fn into(self) -> u8 {
        match self {
            Self::Closed => 0u8,
            Self::Unbonding => 1u8,
            Self::Open => 2u8,
        }
    }
}

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq)]
pub struct Config {
    pub symbol: String,
    pub admin: HumanAddr,
    pub token_contract: HumanAddr,
    pub token_contract_hash: String,
    pub gov_token: HumanAddr,
    pub gov_token_hash: String,
    pub unbonding_time: u64,
    pub viewing_key: String,
    pub kill_switch: u8,
    pub dev_address: HumanAddr,
    pub dev_fee: u64, // 10^-3 percent. 1 = 0.001%
}

pub fn store_address<S: Storage>(storage: &mut S, address: &HumanAddr) {
    let address_bytes: Vec<u8> = bincode2::serialize(&address).unwrap();

    storage.set(&CONTRACT_ADDRESS, &address_bytes);
}

pub fn get_address<S: Storage>(storage: &S) -> StdResult<HumanAddr> {
    if let Some(address_bytes) = storage.get(&CONTRACT_ADDRESS) {
        let record: HumanAddr = bincode2::deserialize(&address_bytes).unwrap();
        Ok(record)
    } else {
        Err(StdError::GenericErr {
            msg: "Privacy token not available for this token".to_string(),
            backtrace: None,
        })
    }
}

pub fn store_frozen_exchange_rate<S: Storage>(storage: &mut S, xrate: &Decimal) {
    let address_bytes: Vec<u8> = bincode2::serialize(&xrate).unwrap();

    storage.set(&FROZEN_EXCHANGE_RATE, &address_bytes);
}

pub fn get_frozen_exchange_rate<S: Storage>(storage: &S) -> StdResult<Decimal> {
    if let Some(address_bytes) = storage.get(&FROZEN_EXCHANGE_RATE) {
        let record: Decimal = bincode2::deserialize(&address_bytes).unwrap();
        Ok(record)
    } else {
        Err(StdError::GenericErr {
            msg: "Privacy token not available for this token".to_string(),
            backtrace: None,
        })
    }
}

pub fn set_config<S: Storage>(storage: &mut S, config: &Config) {
    let config_bytes: Vec<u8> = bincode2::serialize(&config).unwrap();

    let mut config_store = PrefixedStorage::new(PREFIX_CONFIG, storage);
    config_store.set(CONFIG_KEY, &config_bytes);
}

pub fn read_config<S: Storage>(store: &S) -> StdResult<Config> {
    let config_store = ReadonlyPrefixedStorage::new(PREFIX_CONFIG, store);
    let consts_bytes = config_store.get(CONFIG_KEY).unwrap();

    let consts: Config = bincode2::deserialize(&consts_bytes).unwrap();

    Ok(consts)
}
