use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::PendingWithdraw;
use cosmwasm_std::{Binary, HumanAddr, Uint128, VoteOption};

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct InitMsg {
    pub token_code_id: u64,
    pub token_code_hash: String,
    pub validator: String,
    pub symbol: String,
    pub label: String,
    pub prng_seed: Binary,
    pub dev_fee: Option<u64>,
    pub dev_address: Option<HumanAddr>,
    pub code_id: Option<u64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    /// user interactions
    Deposit {},
    Claim {},

    /// token interaction
    Receive {
        amount: Uint128,
        sender: HumanAddr,
        msg: Option<Binary>,
    },

    /// callback init
    PostInitialize {},

    /// voting
    Vote {
        proposal: u64,
        vote: VoteOption,
    },

    /********** admin commands **********/
    /// global "claim" for all expired withdraws
    ClaimMaturedWithdraws {},

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
    Tally {
        proposal: u64,
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
        to: HumanAddr,
    },
    ChangeOwner {
        new_owner: HumanAddr,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    InterestRate {},
    ExchangeRate {},
    PendingClaims {
        address: HumanAddr,
        current_time: Option<u64>,
    },
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
    PendingClaims { pending: Vec<PendingClaimsResponse> },
    ExchangeRate { rate: Uint128, denom: String },
    InterestRate { rate: Uint128, denom: String },
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawRequest {
    Withdraw {},
}
