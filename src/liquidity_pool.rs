use crate::staking::{get_bonded, get_rewards, get_total_onchain_balance};
use crate::state::{get_exchange_rate, update_cached_liquidity_balance, update_total_balance};
use cosmwasm_std::{log, Api, Binary, CosmosMsg, Env, Extern, HandleResponse, HumanAddr, Querier, StdResult, Storage, Uint128, WasmMsg, StdError};

// get_bonded returns the total amount of delegations from contract
// it ensures they are all the same denom
// Simon I'm trusting you that this works don't let me down bro
pub fn liquidity_pool_from_chain<Q: Querier>(
    querier: &Q,
    contract: &HumanAddr,
) -> StdResult<Uint128> {
    let balances = querier.query_all_balances(contract)?;
    if balances.is_empty() {
        return Ok(Uint128(0));
    }
    let denom = balances[0].denom.as_str();
    balances.iter().fold(Ok(Uint128(0)), |racc, d| {
        let acc = racc?;
        if d.denom.as_str() != denom {
            Err(StdError::generic_err(format!(
                "different denoms in bonds: '{}' vs '{}'",
                denom, &d.denom
            )))
        } else {
            Ok(acc + d.amount)
        }
    })
}

pub fn current_staked_ratio<Q: Querier, S: Storage>(
    querier: &Q,
    store: &S,
    contract: &HumanAddr,
) -> StdResult<u128> {
    let pool_size = crate::state::liquidity_pool_balance(store);
    let staked_size = crate::staking::get_bonded(querier, contract)?.u128();

    // This should only happen on 1st deposit, but if everything is empty lets just stake it all
    if staked_size == 0 && pool_size == 0 {
        return Ok(0);
    }

    // if the pool is empty just return a huge number
    if pool_size == 0 {
        return Ok(1000000);
    }

    Ok(staked_size / pool_size)
}

pub fn update_exchange_rate<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {

    let rewards_balance = get_rewards(&deps.querier, &env.contract.address)?;
    let total_on_chain = get_total_onchain_balance(&deps.querier, &env.contract.address)?;

    // update liquidity pool
    let pool = liquidity_pool_from_chain(&deps.querier, &env.contract.address)?.u128();
    update_cached_liquidity_balance(&mut deps.storage, pool);

    // update total balance
    update_total_balance(
        &mut deps.storage,
        &total_on_chain.clone() + &rewards_balance.u128(),
    );

    // calculate new exchange rate
    let ratio = get_exchange_rate(&deps.storage)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("ratio", format!("{:?}", ratio)),
            log("debug_total_on_chain", format!("{:?}", total_on_chain)),
            log(
                "debug_rewards_balance",
                format!("{:?}", rewards_balance.u128()),
            ),
        ],
        data: None,
    })
}

pub fn update_balances_message(contract_address: &HumanAddr, code_hash: &String) -> CosmosMsg {
    CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: contract_address.clone(),
        callback_code_hash: code_hash.clone(),
        msg: Binary("{\"update_exchange_rate\":{}}".as_bytes().to_vec()),
        send: vec![],
    })
}
