use cosmwasm_std::{Binary, HumanAddr, Uint128, VoteOption};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cargo_common::contract::Contract;

use crate::types::pending_withdraws::PendingWithdraw;
use crate::types::validator_set::ValidatorResponse;

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct InitMsg {
    pub token_code_id: u64,
    pub token_code_hash: String,
    pub validator: String,
    pub label: String,
    pub prng_seed: Binary,
    pub dev_fee: Option<u64>,
    pub dev_address: Option<HumanAddr>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    /// user interactions
    Stake {},
    Claim {},

    /// token interaction
    Receive {
        amount: Uint128,
        sender: HumanAddr,
        msg: Option<Binary>,
    },

    /// callback init
    PostInitialize {},

    /********** admin commands **********/
    /// global "claim" for all expired withdraws
    /// amount is the number of addresses we want to claim - this allows us to use "paging"
    /// to only claim a certain amount to avoid large txs or computations
    ClaimMaturedWithdraws {
        amount: u32,
    },

    /// voting
    VoteOnChain {
        proposal: u64,
        vote: VoteOption,
    },

    /// remove validator from set - redelegates all bonds to next available validator
    RemoveValidator {
        address: String,
        redelegate: Option<bool>,
    },

    /// add a new validator to the set
    AddValidator {
        address: String,
        weight: Option<u8>,
    },

    /// add a new validator to the set
    ChangeWeight {
        address: String,
        weight: Option<u8>,
    },

    /// add a new validator to the set
    Redelegate {
        from: String,
        to: String,
    },
    /// Unbond everything
    KillSwitchUnbond {},

    /// open the floodgates
    KillSwitchOpenWithdraws {},

    ChangeUnbondingTime {
        new_time: u64,
    },

    SetGovToken {
        gov_token: HumanAddr,
        gov_token_hash: Option<String>,
    },

    SetMintingGov {
        minting: bool,
    },

    /// setting voting admin but not voting contract will disable the voting on the token
    /// don't set both voting admin and voting contract
    /// gov_token = true -> vote with gov token
    /// gov_token = false -> vote with staking token
    SetVotingContract {
        voting_admin: Option<HumanAddr>,
        voting_contract: Option<Contract>,
        gov_token: Option<bool>,
    },

    /// recover token may be useful to recover lost tokens, gov tokens, or something else
    RecoverToken {
        token: HumanAddr,
        token_hash: String,
        amount: Uint128,
        to: HumanAddr,
        snip20_send_msg: Option<Binary>,
    },
    /// recover SCRT can help us do something with SCRT that is left in the available balance
    RecoverScrt {
        amount: Uint128,
        denom: String,
        to: HumanAddr,
    },
    ChangeOwner {
        new_owner: HumanAddr,
    },
    ChangeDevFee {
        dev_fee: Option<u64>,
        dev_address: Option<HumanAddr>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    ExchangeRate {},
    Claims {
        address: HumanAddr,
        current_time: Option<u64>,
    },
    QueryDevFee {},
    Info {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrateMsg {}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct PendingClaimsResponses {
    pub withdrawals: Vec<PendingClaimsResponse>,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct PendingClaimsResponse {
    pub withdraw: PendingWithdraw,
    pub matured: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryResponse {
    PendingClaims {
        pending: Vec<PendingClaimsResponse>,
    },
    ExchangeRate {
        rate: String,
        denom: String,
    },
    DevFee {
        fee: u64,
        address: HumanAddr,
    },
    Info {
        token_address: HumanAddr,
        validators: Vec<ValidatorResponse>,
        admin: HumanAddr,
        total_staked: Uint128,
        voting_admin: Option<HumanAddr>,
    },
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawRequest {
    Withdraw {},
}
