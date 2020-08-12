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
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub validator: HumanAddr,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Withdraw {
        amount: Uint128,
    },
    Deposit {},
    // Approve {
    //     spender: HumanAddr,
    //     amount: Uint128,
    // },
    Transfer {
        recipient: HumanAddr,
        amount: Uint128,
    },
    // TransferFrom {
    //     owner: HumanAddr,
    //     recipient: HumanAddr,
    //     amount: Uint128,
    // },
    Balance {},
    // Allowance {
    //     spender: HumanAddr,
    // },
    UpdateBalances {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    InterestRate {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrateMsg {}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct BalanceResponse {
    pub balance: Uint128,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema)]
pub struct AllowanceResponse {
    pub allowance: Uint128,
}
