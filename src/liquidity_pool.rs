use crate::staking::{get_bonded, get_rewards, get_total_onchain_balance};
use crate::state::{get_ratio, update_cached_liquidity_balance, update_total_balance};
use cosmwasm_std::{
    generic_err, log, Api, Binary, CosmosMsg, Env, Extern, HandleResponse, HumanAddr, Querier,
    StdResult, Storage, Uint128, WasmMsg,
};

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
            Err(generic_err(format!(
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

    Ok(staked_size / pool_size)
}

pub fn update_exchange_rate<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let contract = deps.api.human_address(&env.contract.address)?;

    let rewards_balance = get_rewards(&deps.querier, &contract)?;
    let total_on_chain = get_total_onchain_balance(&deps.querier, &contract)?;

    update_total_balance(&mut deps.storage, total_on_chain + rewards_balance.u128());

    let ratio = get_ratio(&deps.storage)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("ratio", format!("{:?}", ratio))],
        data: None,
    })
}

pub fn update_balances_message(contract_address: &HumanAddr, code_hash: &String) -> CosmosMsg {
    CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: contract_address.clone(),
        callback_code_hash: code_hash.clone(),
        msg: Binary("{\"update_balances\":{}}".as_bytes().to_vec()),
        send: vec![],
    })
}
