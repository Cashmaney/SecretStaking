use cosmwasm_std::{HumanAddr, Querier, StdResult, Uint128};

pub fn query_total_supply<Q: Querier>(
    querier: &Q,
    token_contract: &HumanAddr,
    token_contract_hash: &str,
) -> StdResult<Uint128> {
    let token_info = secret_toolkit::snip20::token_info_query(
        querier,
        256,
        token_contract_hash.to_string(),
        token_contract.clone(),
    )?;

    Ok(token_info.total_supply.unwrap_or_default())
}
