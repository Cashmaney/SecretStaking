use cosmwasm_std::{
    generic_err, log, Api, Env, Extern, HandleResponse, HumanAddr, Querier, StdResult, Storage,
    Uint128,
};

use crate::liquidity_pool::{current_staked_ratio, update_balances_message};
use crate::staking::stake;
use crate::state::{add_balance, deposit, get_staked_ratio, get_validator_address};

pub fn try_deposit<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let mut amount_raw: Uint128 = Uint128::default();

    let contract_addr = deps.api.human_address(&env.contract.address)?;
    let code_hash = env.contract_code_hash;
    let validator = get_validator_address(&deps.storage)?;

    for coin in &env.message.sent_funds {
        if coin.denom == "uscrt" {
            amount_raw = coin.amount
        }
    }

    if amount_raw == Uint128::default() {
        return Err(generic_err(format!("Lol send some funds dude")));
    }

    let amount = amount_raw.u128();

    let sender_address_raw = &env.message.sender;

    let token_amount = deposit(&mut deps.storage, amount)?;

    let staked_amount =
        amount_to_stake_from_deposit(&deps.querier, &deps.storage, amount, &contract_addr)?;

    add_balance(&mut deps.storage, sender_address_raw, token_amount)?;

    let res = HandleResponse {
        messages: vec![
            stake(&validator, staked_amount),
            update_balances_message(&contract_addr, &code_hash),
        ],
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
fn amount_to_stake_from_deposit<S: Storage, Q: Querier>(
    querier: &Q,
    store: &S,
    deposit_amount: u128,
    contract: &HumanAddr,
) -> StdResult<u128> {
    let target_ratio = get_staked_ratio(store)?;

    let current_ratio = current_staked_ratio(querier, store, contract)?;

    return Ok(if current_ratio > target_ratio {
        deposit_amount
    } else {
        0
    });
}
