use cosmwasm_std::{
    log, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse, Querier, StdError, StdResult,
    Storage, Uint128,
};

use secret_toolkit::snip20;

use crate::staking::{exchange_rate, get_rewards, stake_msg};
use crate::state::read_config;
use crate::validator_set::{get_validator_set, set_validator_set};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

const FEE_RESOLUTION: u128 = 100_000;

pub fn try_deposit<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let mut amount_raw: Uint128 = Uint128::default();
    let config = read_config(&deps.storage)?;
    let mut validator_set = get_validator_set(&deps.storage)?;
    let mut messages: Vec<CosmosMsg> = vec![];

    for coin in &env.message.sent_funds {
        if coin.denom == "uscrt" {
            amount_raw = coin.amount
        }
    }

    if amount_raw == Uint128::default() {
        return Err(StdError::generic_err(
            "Lol send some funds dude".to_string(),
        ));
    }

    if amount_raw.u128() < 1_000_000 {
        return Err(StdError::generic_err(
            "Can only deposit a minimum of 1000000 uscrt, or 1 scrt",
        ));
    }

    let exch_rate = exchange_rate(&deps.storage, &deps.querier)?;

    let fee = calc_fee(amount_raw, config.dev_fee);

    messages.push(CosmosMsg::Bank(BankMsg::Send {
        from_address: env.contract.address.clone(),
        to_address: config.dev_address,
        amount: vec![Coin {
            denom: "uscrt".to_string(),
            amount: Uint128::from(fee * 99 / 100),
        }],
    }));

    amount_raw = Uint128::from(amount_raw.u128().saturating_sub(fee as u128));

    let token_amount = calc_deposit(amount_raw, exch_rate)?;

    let constants = read_config(&deps.storage)?;
    messages.push(snip20::mint_msg(
        env.message.sender.clone(),
        token_amount.into(),
        None,
        256,
        constants.token_contract_hash,
        constants.token_contract,
    )?);

    // deposit = outstanding rewards + deposited amount
    let deposit_amount = get_rewards(&deps.querier, &env.contract.address)
        .unwrap_or_default()
        .u128()
        + amount_raw.u128();
    messages.append(&mut validator_set.withdraw_rewards_messages());

    // add the amount to our stake tracker
    let validator = validator_set.stake(deposit_amount)?;
    validator_set.rebalance();

    // send the stake message
    messages.push(stake_msg(&validator, deposit_amount));

    set_validator_set(&mut deps.storage, &validator_set)?;

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "deposit"),
            log("account", env.message.sender.as_str()),
            log("amount", &token_amount.to_string()),
        ],
        data: None,
    })
}

/// Calculates how much your deposited SCRT is worth in tokens
/// Adds the balance from the total supply and balance
/// Returns amount of tokens you get
pub fn calc_deposit(amount: Uint128, exchange_rate: Decimal) -> StdResult<u128> {
    let tokens_to_mint = exchange_rate
        .checked_mul(Decimal::from(amount.u128() as u64))
        .unwrap()
        .to_u128()
        .unwrap();

    Ok(tokens_to_mint)
}

pub fn calc_fee(amount: Uint128, fee: u64) -> u128 {
    amount
        .u128()
        .saturating_mul(fee as u128)
        .checked_div(FEE_RESOLUTION)
        .unwrap_or(0)
}
