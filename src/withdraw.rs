use rust_decimal::Decimal;

use crate::liquidity_pool::update_balances_message;
use crate::staking::undelegate;
use crate::state::{
    get_exchange_rate, get_fee, liquidity_pool_balance, remove_balance, withdraw,
    EXCHANGE_RATE_RESOLUTION,
};
use crate::validator_set::{get_validator_set, set_validator_set};
use cosmwasm_std::{
    generic_err, log, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse, Querier,
    StdResult, Storage, Uint128,
};
use rust_decimal::prelude::ToPrimitive;

pub fn try_withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    let owner_address_raw = &env.message.sender;
    let code_hash = env.contract_code_hash;
    //let validator = get_validator_address(&deps.storage)?;

    let mut validator_set = get_validator_set(&deps.storage)?;

    let contract_addr = deps.api.human_address(&env.contract.address)?;
    let withdrawal_address = deps.api.human_address(&env.message.sender)?;
    let current_liquidity = liquidity_pool_balance(&deps.storage);
    let exch_rate = get_exchange_rate(&deps.storage)?;

    if amount.u128() < EXCHANGE_RATE_RESOLUTION as u128 {
        return Err(generic_err("Can only withdraw a minimum of 1000 uscrt"));
    }

    // todo: set this limit in some other way
    if current_liquidity
        < (exch_rate.checked_mul(Decimal::from(amount.u128() as u64)))
            .unwrap()
            .to_u128()
            .unwrap()
    {
        return Err(generic_err(format!(
            "Cannot withdraw this amount at this time. You can only withdraw a limit of {:?} uscrt",
            current_liquidity
        )));
    }

    remove_balance(&mut deps.storage, owner_address_raw, amount.u128())?;

    let fee = get_fee(&deps.storage)?;
    let scrt_amount = withdraw(&mut deps.storage, amount, exch_rate, fee)?;

    let validator = validator_set.unbond(scrt_amount as u64)?;
    validator_set.rebalance();
    set_validator_set(&mut deps.storage, &validator_set)?;

    let scrt = Coin {
        denom: "uscrt".to_string(),
        amount: Uint128::from(scrt_amount),
    };

    let res = HandleResponse {
        messages: vec![
            CosmosMsg::Bank(BankMsg::Send {
                from_address: contract_addr.clone(),
                to_address: withdrawal_address,
                amount: vec![scrt.clone()],
            }),
            undelegate(&validator, scrt_amount),
            update_balances_message(&contract_addr, &code_hash),
        ],
        log: vec![
            log("action", "withdraw"),
            log(
                "account",
                deps.api.human_address(&env.message.sender)?.as_str(),
            ),
            log("amount", format!("{:?}", scrt)),
        ],
        data: None,
    };

    Ok(res)
}
