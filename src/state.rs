use cosmwasm_std::{Coin, HumanAddr, ReadonlyStorage, StdError, StdResult, Storage, VoteOption};

use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cargo_common::balances::Balances;
use rust_decimal::Decimal;
use secret_toolkit::storage::{AppendStore, AppendStoreMut, TypedStore, TypedStoreMut};
use std::convert::TryFrom;
use test::test::parse_opts;

pub const INDEXES: &[u8] = b"indexes";

pub const PREFIX_CONFIG: &[u8] = b"config";

pub const KEY_VALIDATOR_SET: &[u8] = b"validator_address";

pub static CONFIG_KEY: &[u8] = b"config";
pub const CONTRACT_ADDRESS: &[u8] = b"contract_address";
pub const FROZEN_EXCHANGE_RATE: &[u8] = b"FROZEN_EXCHANGE_RATE";
pub const PENDING_WITHDRAW: &[u8] = b"PENDING_WITHDRAW";
pub const VOTES: &[u8] = b"VOTES";

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

#[derive(Default)]
pub struct VoteTotals {
    pub yes: u128,
    pub no: u128,
    pub abstain: u128,
    pub no_with_veto: u128,
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
}

impl Votes {
    pub fn tally<S: Storage>(
        store: &S,
        proposal_id: u64,
        balances: &Balances,
    ) -> StdResult<VoteOption> {
        let mut vote_totals = VoteTotals::default();

        for address in &balances.0 {
            let vote = Votes::get(store, proposal_id, &address.account)?;
            match u32_to_vote_option(vote.vote) {
                VoteOption::Yes => vote_totals.yes += address.amount,
                VoteOption::No => vote_totals.no += address.amount,
                VoteOption::Abstain => vote_totals.abstain += address.amount,
                VoteOption::NoWithVeto => vote_totals.no_with_veto += address.amount,
            }
        }

        Ok(vote_totals.winner())
    }

    pub fn get_voters<S: Storage>(store: &S, proposal_id: u64) -> StdResult<Vec<HumanAddr>> {
        let store = ReadonlyPrefixedStorage::multilevel(
            &[VOTES, &proposal_id.to_be_bytes(), INDEXES],
            store,
        );
        let store = if let Some(result) = AppendStore::<HumanAddr, _>::attach(&store) {
            result?
        } else {
            return Ok(vec![]);
        };

        let mut voters = vec![];

        for addr in store.iter().flatten() {
            voters.push(addr);
        }

        Ok(voters)
    }

    pub fn set<S: Storage>(storage: &mut S, proposal_id: u64, vote: SingleVote) -> StdResult<()> {
        let mut store =
            PrefixedStorage::multilevel(&[VOTES, &proposal_id.to_be_bytes(), INDEXES], storage);
        let mut proposal_store = AppendStoreMut::attach_or_create(&mut store)?;
        proposal_store.push(&vote.address)?;

        let mut mut_store =
            PrefixedStorage::multilevel(&[VOTES, &proposal_id.to_be_bytes()], storage);
        let mut owner_store =
            TypedStoreMut::<SingleVote, PrefixedStorage<S>>::attach(&mut mut_store);
        owner_store.store(vote.address.0.as_bytes(), &vote)
        //Ok(())
    }

    pub fn get<S: Storage>(
        store: &S,
        proposal_id: u64,
        address: &HumanAddr,
    ) -> StdResult<SingleVote> {
        let ro_store =
            ReadonlyPrefixedStorage::multilevel(&[VOTES, &proposal_id.to_be_bytes()], store);
        let owner_store = TypedStore::<SingleVote, ReadonlyPrefixedStorage<S>>::attach(&ro_store);
        owner_store.load(address.0.as_bytes())
        // owner_store.may_load(address.clone().0.as_bytes())
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

    pub(crate) fn remove_expired(&mut self, current_time: u64) -> Vec<PendingWithdraw> {
        self.0
            .drain_filter(|item| item.available_time <= current_time)
            .collect::<Vec<_>>()
    }

    // pub(crate) fn amount_reserved_for_claims(&self, time: u64) -> u128 {
    //     let expired = self.get_expired(time);
    //     expired
    //         .iter()
    //         .map(|withdraw| withdraw.coins.amount.u128())
    //         .sum()
    // }

    pub(crate) fn remove_expired_by_sender(
        &mut self,
        current_time: u64,
        sender: &HumanAddr,
    ) -> Vec<PendingWithdraw> {
        self.0
            .drain_filter(|item| (item.available_time <= current_time && &item.receiver == sender))
            .collect::<Vec<_>>()
    }

    pub fn get_expired(&self, current_time: u64) -> Vec<PendingWithdraw> {
        self.0
            .clone()
            .drain_filter(|item| item.available_time <= current_time)
            .collect::<Vec<_>>()
    }

    // pub(crate) fn get_expired_by_sender(
    //     &self,
    //     current_time: u64,
    //     sender: &HumanAddr,
    // ) -> Vec<PendingWithdraw> {
    //     self.0
    //         .clone()
    //         .drain_filter(|item| item.available_time <= current_time && &item.receiver == sender)
    //         .collect::<Vec<_>>()
    // }

    pub(crate) fn get_pending_by_address(&self, sender: &HumanAddr) -> Vec<PendingWithdraw> {
        let mut pending: Vec<PendingWithdraw> = self.0.clone();

        // return all withdrawals that have been executed on-chain

        pending.retain(|item| &item.receiver == sender);

        pending
    }

    pub(crate) fn save<S: Storage>(self, storage: &mut S) {
        let bytes: Vec<u8> = bincode2::serialize(&self).unwrap();

        storage.set(&PENDING_WITHDRAW, &bytes);
    }

    pub(crate) fn load_by_address<S: Storage>(storage: &S, address: &HumanAddr) -> StdResult<Self> {
        let store = ReadonlyPrefixedStorage::multilevel(
            &[PENDING_WITHDRAW, &address.0.as_bytes()],
            storage,
        )?;

        let store = if let Some(result) = AppendStore::<PendingWithdraw, _>::attach(&store) {
            result?
        } else {
            return Ok(Self::default());
        };

        let mut withdraws = vec![];

        for addr in store.iter().flatten() {
            withdraws.push(addr);
        }

        Ok(PendingWithdraws(withdraws))
    }

    pub(crate) fn load<S: Storage>(storage: &S) -> StdResult<Self> {
        let store = ReadonlyPrefixedStorage::new(&PENDING_WITHDRAW, storage);

        let store = if let Some(result) = AppendStore::<PendingWithdraw, _>::attach(&store) {
            result?
        } else {
            return Ok(Self::default());
        };

        let mut voters = vec![];

        for addr in store.iter().flatten() {
            voters.push(addr);
        }

        Ok(PendingWithdraws(voters))
    }

    pub(crate) fn append_withdraw<S: Storage>(
        storage: &mut S,
        withdraw: &PendingWithdraw,
    ) -> StdResult<()> {
        let mut store = PrefixedStorage::new(&PENDING_WITHDRAW, storage);
        let mut store = AppendStoreMut::attach_or_create(&mut store)?;
        store.push(withdraw)
    }

    pub(crate) fn append_withdraw_by_address<S: Storage>(
        storage: &mut S,
        withdraw: &PendingWithdraw,
    ) -> StdResult<()> {
        let mut store = PrefixedStorage::multilevel(&[PENDING_WITHDRAW, INDEXES], storage);
        let mut proposal_store = AppendStoreMut::attach_or_create(&mut store)?;
        proposal_store.push(&withdraw.receiver)?;

        let mut mut_store = PrefixedStorage::multilevel(
            &[PENDING_WITHDRAW, &withdraw.receiver.0.as_bytes()],
            storage,
        )?;
        let mut owner_store =
            AppendStoreMut::<PendingWithdraw, PrefixedStorage<S>>::attach_or_create(
                &mut mut_store,
            )?;
        owner_store.push(withdraw)
    }

    pub(crate) fn save_by_address<S: Storage>(self, storage: &mut S, address: HumanAddr) -> StdResult<()> {
        let mut store: PrefixedStorage<PendingWithdraw> = PrefixedStorage::multilevel(
            &[PENDING_WITHDRAW, INDEXES],
            storage,
        )?;

        if (store)

        let store = AppendStoreMut::<PendingWithdraw, _>::attach_or_create(&mut store)?;

        //let mut withdraws = vec![];

        for addr in self.0.iter().flatten() {
            withdraws.push(addr);
        }

        Ok(PendingWithdraws(withdraws))
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
