use cosmwasm_std::{
    generic_err, log, Api, CosmosMsg, Env, Extern, HandleResponse, HumanAddr, Querier, StdResult,
    Storage, Uint128,
};

use crate::liquidity_pool::{current_staked_ratio, update_balances_message};
use crate::staking::stake;
use crate::state::{
    add_token_balance, deposit, get_exchange_rate, get_staked_ratio, get_validator_address,
};
use crate::validator_set::{get_validator_set, set_validator_set};

pub fn try_deposit<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let mut amount_raw: Uint128 = Uint128::default();

    let contract_addr = deps.api.human_address(&env.contract.address)?;
    let code_hash = env.contract_code_hash;

    let mut validator_set = get_validator_set(&deps.storage)?;

    for coin in &env.message.sent_funds {
        if coin.denom == "uscrt" {
            amount_raw = coin.amount
        }
    }

    if amount_raw == Uint128::default() {
        return Err(generic_err(format!("Lol send some funds dude")));
    }

    let amount = amount_raw.u128();

    if amount < 1000 {
        return Err(generic_err("Can only deposit a minimum of 1000 uscrt"));
    }

    let sender_address_raw = &env.message.sender;

    let exch_rate = get_exchange_rate(&deps.storage)?;
    let token_amount = deposit(&mut deps.storage, amount_raw, exch_rate)?;

    let staked_amount =
        amount_to_stake_from_deposit(&deps.querier, &deps.storage, amount, &contract_addr)?;

    add_token_balance(&mut deps.storage, sender_address_raw, token_amount)?;

    let mut messages: Vec<CosmosMsg> = vec![];

    let validator = validator_set.stake(staked_amount as u64)?;
    validator_set.rebalance();

    if staked_amount > 0 {
        messages.push(stake(&validator, staked_amount));
    }

    messages.push(update_balances_message(&contract_addr, &code_hash));

    set_validator_set(&mut deps.storage, &validator_set)?;

    let res = HandleResponse {
        messages,
        log: vec![
            log("action", "deposit"),
            log(
                "account",
                deps.api.human_address(&env.message.sender)?.as_str(),
            ),
            log("amount", &token_amount.to_string()),
        ],
        data: None,
    };

    Ok(res)
}

/// calculate amount that goes to the staking pool and the amount that should stay in the liquidity pool
/// naive all or nothing - might be okay to keep things simple at first
pub fn amount_to_stake_from_deposit<S: Storage, Q: Querier>(
    querier: &Q,
    store: &S,
    deposit_amount: u128,
    contract: &HumanAddr,
) -> StdResult<u128> {
    let target_ratio = get_staked_ratio(store)?;

    let current_ratio = current_staked_ratio(querier, store, contract)?;

    // if target ratio is greater than the current ratio it means that we need to stake more (increase stake-liquidity ratio)
    return Ok(if target_ratio > current_ratio {
        deposit_amount
    } else {
        0
    });
}
