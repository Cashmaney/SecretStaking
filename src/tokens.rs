use std::convert::TryFrom;

use cosmwasm_std::{
    to_binary, Binary, HumanAddr, Querier, QueryRequest, StdError, StdResult, Uint128, WasmQuery,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cargo_common::balances::Balances;
use cargo_common::tokens::InitHook;

/// TokenContract InitMsg
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Balance {
    pub amount: Uint128,
    pub address: HumanAddr,
}

/// TokenContract InitMsg
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct TokenInitMsg {
    pub name: String,
    pub admin: Option<HumanAddr>,
    pub symbol: String,
    pub decimals: u8,
    pub initial_balances: Option<Vec<Balance>>,
    pub prng_seed: Binary,
    pub init_hook: Option<InitHook>,
    pub config: Option<InitConfig>,
}

#[derive(Serialize, Deserialize, JsonSchema, Clone, Default, Debug)]
#[serde(rename_all = "snake_case")]
pub struct InitConfig {
    /// Indicates whether the total supply is public or should be kept secret.
    /// default: False
    pub public_total_supply: Option<bool>,
}

impl TokenInitMsg {
    pub fn new(
        name: String,
        admin: HumanAddr,
        symbol: String,
        decimals: u8,
        prng_seed: Binary,
        init_hook: InitHook,
    ) -> Self {
        Self {
            name,
            admin: Some(admin),
            symbol,
            decimals,
            initial_balances: None,
            prng_seed,
            init_hook: Some(init_hook),
            config: Some(InitConfig {
                public_total_supply: Some(true),
            }),
        }
    }
    pub fn validate(&self) -> StdResult<()> {
        // Check name, symbol, decimals
        if !is_valid_name(&self.name) {
            return Err(StdError::generic_err(
                "Name is not in the expected format (3-50 UTF-8 bytes)",
            ));
        }
        if !is_valid_symbol(&self.symbol) {
            return Err(StdError::generic_err(
                "Ticker symbol is not in expected format [a-zA-Z\\-]{3,12}",
            ));
        }
        if self.decimals > 18 {
            return Err(StdError::generic_err("Decimals must not exceed 18"));
        }
        Ok(())
    }
}

fn is_valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.len() < 3 || bytes.len() > 50 {
        return false;
    }
    true
}

fn is_valid_symbol(symbol: &str) -> bool {
    let bytes = symbol.as_bytes();
    if bytes.len() < 3 || bytes.len() > 12 {
        return false;
    }
    for byte in bytes.iter() {
        if (*byte != 45) && (*byte < 65 || *byte > 90) && (*byte < 97 || *byte > 122) {
            return false;
        }
    }
    true
}

pub fn query_balances<Q: Querier>(
    querier: &Q,
    token_contract: &HumanAddr,
    token_contract_hash: &String,
    address: &HumanAddr,
    key: &String,
    voters: Vec<HumanAddr>,
) -> StdResult<Balances> {
    let query = QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: token_contract.clone(),
        callback_code_hash: token_contract_hash.clone(),
        msg: to_binary(&TokenQuery::MultipleBalances {
            address: address.clone(),
            key: key.clone(),
            addresses: voters,
        })?,
    });

    if let TokenQueryResponse::MultipleBalances { balances } = querier.query(&query.into())? {
        let deserialized = Balances::try_from(balances)?;
        return Ok(deserialized);
    } else {
        return Err(StdError::generic_err("Failed to get balances"));
    }
}

pub fn query_total_supply<Q: Querier>(
    querier: &Q,
    token_contract: &HumanAddr,
    token_contract_hash: &String,
) -> Uint128 {
    let token_info = secret_toolkit::snip20::token_info_query(
        querier,
        256,
        token_contract_hash.clone(),
        token_contract.clone(),
    )
    .unwrap();

    return token_info.total_supply.unwrap_or_default();
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TokenQuery {
    MultipleBalances {
        address: HumanAddr,
        key: String,
        addresses: Vec<HumanAddr>,
    },
    TokenInfo {},
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TokenQueryResponse {
    MultipleBalances {
        balances: Binary,
    },
    TokenInfo {
        name: String,
        symbol: String,
        decimals: u8,
        total_supply: Option<Uint128>,
    },
}
