use crate::state::SingleVote;
use cosmwasm_std::{HumanAddr, Uint128, VoteOption};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct InitMsg {
    pub staking_contract: HumanAddr,
    pub staking_contract_hash: String,

    pub gov_token: HumanAddr,
    pub gov_token_hash: String,

    pub voting_time: Option<u64>,

    pub admin: Option<HumanAddr>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Vote {
        proposal: u64,
        vote: SingleVote,
    },

    // Admin commands
    ChangeVotingTime {
        new_time: u64,
    },
    SetStakingContract {
        staking_contract: HumanAddr,
        staking_contract_hash: Option<String>,
    },
    SetGovToken {
        gov_token: HumanAddr,
        gov_token_hash: Option<String>,
    },
    Tally {
        proposal: u64,
    },
    ChangeOwner {
        new_owner: HumanAddr,
    },
    NotifyBalanceChange {
        changes: Vec<VoteChange>,
    },
    CreateSnapshot {
        proposal: u64,
    },
    InitVote {
        proposal: u64,
        voting_time: Option<u64>,
    },
    SetPassword {
        password: String,
    },
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
pub struct VoteChange {
    pub voting_power: u64,
    pub address: HumanAddr,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum HandleAnswer {
    Vote {
        proposal: u64,
        vote: VoteOption,
        voting_power: u64,
        status: ResponseStatus,
    },

    Tally {
        status: ResponseStatus,
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Success,
    Failure,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    ActiveProposals,
    InactiveProposals,
    VoteState {
        proposal: u64,
    },
    QueryVote {
        address: HumanAddr,
        proposal: u64,
        password: String,
    },
}

#[derive(Serialize, Deserialize, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub enum QueryAnswer {
    ActiveProposals {
        proposals: Vec<u64>,
    },
    InActiveProposals {
        proposals: Vec<u64>,
    },
    VoteState {
        proposal: u64,
        yes: Uint128,
        no: Uint128,
        no_with_veto: Uint128,
        abstain: Uint128,
        end_time: u64,
        active: bool,
        result: Option<VoteOption>,
    },
    QueryVote {
        address: HumanAddr,
        proposal: u64,
        vote: Option<VoteOption>,
        voting_power: Uint128,
    },
}
