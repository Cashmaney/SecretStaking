use std::convert::TryFrom;

use cosmwasm_std::{
    to_binary, HumanAddr, Querier, QueryRequest, StdError, StdResult, Uint128, WasmQuery,
};

use cargo_common::balances::Balances;
use cargo_common::tokens::{TokenQuery, TokenQueryResponse};

pub fn query_balances<Q: Querier>(
    querier: &Q,
    token_contract: &HumanAddr,
    token_contract_hash: &str,
    address: &HumanAddr,
    key: &str,
    voters: Vec<HumanAddr>,
) -> StdResult<Balances> {
    let query = QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: token_contract.clone(),
        callback_code_hash: token_contract_hash.to_string(),
        msg: to_binary(&TokenQuery::MultipleBalances {
            address: address.clone(),
            key: key.to_string(),
            addresses: voters,
        })?,
    });

    if let TokenQueryResponse::MultipleBalances { balances } = querier.query(&query)? {
        let deserialized = Balances::try_from(balances)?;
        Ok(deserialized)
    } else {
        Err(StdError::generic_err("Failed to get balances"))
    }
}

pub fn query_total_supply<Q: Querier>(
    querier: &Q,
    token_contract: &HumanAddr,
    token_contract_hash: &str,
) -> Uint128 {
    let token_info = secret_toolkit::snip20::token_info_query(
        querier,
        256,
        token_contract_hash.to_string(),
        token_contract.clone(),
    )
    .unwrap();

    token_info.total_supply.unwrap_or_default()
}
