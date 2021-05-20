use std::convert::TryFrom;

use cosmwasm_std::{
    log, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse, Querier, StdError, StdResult,
    Storage, Uint128,
};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use secret_toolkit::snip20;

use crate::claim::claim_multiple;
use crate::constants::{AMOUNT_OF_REWARDS_TO_HANDLE, AMOUNT_OF_SHARED_WITHDRAWS};
use crate::staking::{exchange_rate, get_rewards_limited, stake_msg};
use crate::types::config::read_config;
use crate::types::killswitch::KillSwitch;
use crate::types::shared_withdraw_config::SharedWithdrawConfig;
use crate::types::validator_set::{get_validator_set, set_validator_set};

const FEE_RESOLUTION: u128 = 100_000;

pub fn try_deposit<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let mut amount_raw: Uint128 = Uint128::default();
    let config = read_config(&deps.storage)?;
    let mut validator_set = get_validator_set(&deps.storage)?;
    let mut messages: Vec<CosmosMsg> = vec![];

    let kill_switch = KillSwitch::try_from(config.kill_switch)?;
    let withdraw_config = SharedWithdrawConfig::try_from(config.shared_withdrawals)?;

    if kill_switch == KillSwitch::Unbonding || kill_switch == KillSwitch::Open {
        return Err(StdError::generic_err(
            "Contract has been frozen. New deposits are not currently possible",
        ));
    }

    for coin in &env.message.sent_funds {
        if coin.denom == "uscrt" {
            amount_raw = coin.amount
        }
    }

    if amount_raw == Uint128::default() {
        return Err(StdError::generic_err(
            "Can only deposit a minimum of 1000000 uscrt, or 1 scrt".to_string(),
        ));
    }

    if amount_raw.u128() < 1_000_000 {
        return Err(StdError::generic_err(
            "Can only deposit a minimum of 1000000 uscrt, or 1 scrt",
        ));
    }

    if withdraw_config == SharedWithdrawConfig::Deposits
        || withdraw_config == SharedWithdrawConfig::All
    {
        messages.extend(claim_multiple(deps, &env, AMOUNT_OF_SHARED_WITHDRAWS)?.messages);
    }

    let exch_rate = exchange_rate(&deps.storage, &deps.querier)?;

    let fee = calc_fee(amount_raw, config.dev_fee);

    messages.push(CosmosMsg::Bank(BankMsg::Send {
        from_address: env.contract.address.clone(),
        to_address: config.dev_address,
        amount: vec![Coin {
            denom: "uscrt".to_string(),
            amount: Uint128::from(fee * 99 / 100), // leave a tiny amount in the contract for round error purposes
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

    // get rewards from 5 validators with the most rewards
    let top_5_rewards = get_rewards_limited(
        &deps.querier,
        &env.contract.address,
        AMOUNT_OF_REWARDS_TO_HANDLE,
    )?;

    // deposit = outstanding rewards + deposited amount
    // let deposit_amount = get_rewards(&deps.querier, &env.contract.address)
    //     .unwrap_or_default()
    //     .u128()
    //     + amount_raw.u128();
    let deposit_amount = top_5_rewards
        .total
        .first()
        .unwrap_or(&Coin {
            denom: "".to_string(),
            amount: Default::default(),
        })
        .amount
        .u128()
        + amount_raw.u128();

    let top_5_validators = top_5_rewards
        .rewards
        .iter()
        .map(|v| v.validator_address.0.clone())
        .collect();
    messages.append(&mut validator_set.withdraw_rewards_messages(Some(top_5_validators)));

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
