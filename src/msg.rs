use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{HumanAddr, Uint128};

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct InitialBalance {
    pub address: HumanAddr,
    pub amount: Uint128,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct InitMsg {
    pub token_contract: HumanAddr,
    pub token_contract_hash: String,
    pub validator: String,
    pub target_staking_ratio: u8,
    pub fee_pips: u32,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Deposit {},
    Receive {
        amount: Uint128,
        sender: HumanAddr,
    },
    // admin commands
    RegisterReceive {
        address: HumanAddr,
        token_contract_hash: String,
    },
    UpdateExchangeRate {},
    QueryBalances {},
    WithdrawToLiquidityPool {},
    UpdateDailyLiquidity {},
    HandleRewards {},
    UpdateValidatorWhitelist {},
    WithdrawLiquidity {
        address: HumanAddr,
        amount: Uint128,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    InterestRate {},
    ExchangeRate {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct BalanceResponse {
    pub balance: Uint128,
}
