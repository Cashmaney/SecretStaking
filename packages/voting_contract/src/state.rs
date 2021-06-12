use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cargo_common::cashmap::{CashMap, ReadOnlyCashMap};
use cosmwasm_std::{HumanAddr, ReadonlyStorage, StdResult, Storage, VoteOption};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};

pub const PREFIX_CONFIG: &[u8] = b"config";

pub static CONFIG_KEY: &[u8] = b"config";
pub static ACTIVE_PROPOSALS_KEY: &[u8] = b"active_proposals";
pub static INACTIVE_PROPOSALS_KEY: &[u8] = b"inactive_proposals";

pub static PROPOSALS: &[u8] = b"proposals";

pub const VOTES: &[u8] = b"VOTES";
pub const VOTE_TOTALS: &[u8] = b"VOTE_TOTALS";

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

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq)]
pub struct Config {
    pub admin: HumanAddr,
    pub gov_token: HumanAddr,
    pub gov_token_hash: String,
    pub staking_contract: HumanAddr,
    pub staking_contract_hash: String,
    pub voting_time: u64,
    pub password: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct Proposal {
    pub proposal_id: u64,
    pub start_time: u64,
    pub end_time: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Proposals {
    pub proposals: Vec<Proposal>,
}

impl Proposals {
    pub fn contains(&self, proposal_id: &u64) -> bool {
        for item in self.proposals.iter() {
            if &item.proposal_id == proposal_id {
                return true;
            }
        }
        false
    }
}

pub fn set_active_proposals<S: Storage>(storage: &mut S, proposals: &Proposals) {
    let config_bytes: Vec<u8> = bincode2::serialize(&proposals).unwrap();

    let mut config_store = PrefixedStorage::new(PROPOSALS, storage);
    config_store.set(ACTIVE_PROPOSALS_KEY, &config_bytes);
}

pub fn get_active_proposals<S: Storage>(store: &S) -> Proposals {
    let config_store = ReadonlyPrefixedStorage::new(PROPOSALS, store);
    let consts_bytes = config_store.get(ACTIVE_PROPOSALS_KEY).unwrap_or_default();

    let consts: Proposals = bincode2::deserialize(&consts_bytes).unwrap_or_default();

    consts
}

pub fn set_inactive_proposals<S: Storage>(storage: &mut S, proposals: &Proposals) {
    let config_bytes: Vec<u8> = bincode2::serialize(&proposals).unwrap();

    let mut config_store = PrefixedStorage::new(PROPOSALS, storage);
    config_store.set(INACTIVE_PROPOSALS_KEY, &config_bytes);
}

pub fn get_inactive_proposals<S: Storage>(store: &S) -> Proposals {
    let config_store = ReadonlyPrefixedStorage::new(PROPOSALS, store);
    let consts_bytes = config_store.get(INACTIVE_PROPOSALS_KEY).unwrap_or_default();

    let consts: Proposals = bincode2::deserialize(&consts_bytes).unwrap_or_default();

    consts
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug)]
pub struct SingleVote {
    pub address: HumanAddr,
    pub vote: u32,
    pub voting_power: u64,
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

    pub fn change(&mut self, vote: VoteOption, change: u128, add_votes: bool) {
        match vote {
            VoteOption::Yes => {
                self.yes = if add_votes {
                    self.yes.saturating_add(change)
                } else {
                    self.yes.saturating_sub(change)
                }
            }
            VoteOption::No => {
                self.no = if add_votes {
                    self.no.saturating_add(change)
                } else {
                    self.no.saturating_sub(change)
                }
            }
            VoteOption::Abstain => {
                self.abstain = if add_votes {
                    self.abstain.saturating_add(change)
                } else {
                    self.abstain.saturating_sub(change)
                }
            }
            VoteOption::NoWithVeto => {
                self.no_with_veto = if add_votes {
                    self.no_with_veto.saturating_add(change)
                } else {
                    self.no_with_veto.saturating_sub(change)
                }
            }
        }
    }

    pub fn load<S: Storage>(store: &S, proposal_id: u64) -> Self {
        let cashmap = ReadOnlyCashMap::init(VOTE_TOTALS, store);

        cashmap.get(&proposal_id.to_be_bytes()).unwrap_or_default()
    }

    pub fn store<S: Storage>(self, store: &mut S, proposal_id: u64) -> StdResult<()> {
        let mut cashmap = CashMap::init(VOTE_TOTALS, store);

        cashmap.insert(&proposal_id.to_be_bytes(), self)
    }

    // pub fn done(&self) -> bool {
    //     self.counted_votes >= self.threshold
    // }
}

impl Votes {
    // pub fn tally<S: Storage>(
    //     store: &mut S,
    //     proposal_id: u64,
    //     balances: &Balances,
    // ) -> StdResult<Option<VoteOption>> {
    //     let mut vote_totals = VoteTotals::load(store, proposal_id);
    //     let cashmap = CashMap::init(&[VOTES, &proposal_id.to_be_bytes()].concat(), store);
    //
    //     if vote_totals.counted_votes == 0 {
    //         vote_totals.threshold = cashmap.len()
    //     }
    //
    //     for address in &balances.0 {
    //         //let vote = Votes::get(store, proposal_id, &address.account)?;
    //         let vote: Option<SingleVote> = cashmap.get(address.account.0.as_bytes());
    //
    //         if let Some(_vote) = vote {
    //             match u32_to_vote_option(_vote.vote) {
    //                 VoteOption::Yes => vote_totals.yes += address.amount,
    //                 VoteOption::No => vote_totals.no += address.amount,
    //                 VoteOption::Abstain => vote_totals.abstain += address.amount,
    //                 VoteOption::NoWithVeto => vote_totals.no_with_veto += address.amount,
    //             }
    //             vote_totals.counted_votes += 1;
    //         }
    //     }
    //
    //     let result = if vote_totals.done() {
    //         Ok(Some(vote_totals.winner()))
    //     } else {
    //         Ok(None)
    //     };
    //
    //     vote_totals.store(store, proposal_id)?;
    //
    //     return result;
    // }
    //
    // pub fn get_voters<S: Storage>(
    //     store: &S,
    //     proposal_id: u64,
    //     page: u32,
    //     page_size: u32,
    // ) -> StdResult<Vec<HumanAddr>> {
    //     let cashmap = ReadOnlyCashMap::init(&[VOTES, &proposal_id.to_be_bytes()].concat(), store);
    //
    //     let voters: Vec<SingleVote> = cashmap.paging(page, page_size)?;
    //
    //     Ok(voters.iter().map(|vote| vote.address.clone()).collect())
    // }

    pub fn set<S: Storage>(storage: &mut S, proposal_id: u64, vote: SingleVote) -> StdResult<()> {
        let mut cashmap = CashMap::init(&[VOTES, &proposal_id.to_be_bytes()].concat(), storage);
        cashmap.insert(vote.address.0.as_bytes(), vote.clone())
    }

    pub fn get<S: Storage>(store: &S, proposal_id: u64, address: &HumanAddr) -> Option<SingleVote> {
        let cashmap = ReadOnlyCashMap::init(&[VOTES, &proposal_id.to_be_bytes()].concat(), store);
        cashmap.get(&address.0.as_bytes())
    }

    // pub fn len<S: Storage>(store: &S, proposal_id: u64) -> u32 {
    //     let cashmap = ReadOnlyCashMap::<SingleVote, S>::init(
    //         &[VOTES, &proposal_id.to_be_bytes()].concat(),
    //         store,
    //     );
    //     cashmap.len()
    // }
}
