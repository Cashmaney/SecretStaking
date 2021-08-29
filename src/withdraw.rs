use std::cmp::min;
use std::convert::TryFrom;

use cosmwasm_std::{
    debug_print, from_binary, log, Api, BankMsg, Binary, Coin, CosmosMsg, Env, Extern,
    HandleResponse, HumanAddr, Querier, StdError, StdResult, Storage, Uint128,
};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use secret_toolkit::snip20;

use crate::msg::WithdrawRequest;
use crate::staking::{exchange_rate, get_balance, undelegate_msg};
use crate::state::get_frozen_exchange_rate;
use crate::types::config::read_config;
use crate::types::killswitch::KillSwitch;
use crate::types::user_withdraws::UserWithdrawManager;
use crate::types::validator_set::{get_validator_set, set_validator_set};
use crate::types::window_manager::{get_window_manager, set_window_manager, WindowManager};
use crate::types::withdraw_window::set_claim_time;
use crate::utils::perform_helper_claims;

const MINIMUM_WITHDRAW: u128 = 1_000_000; // 1 scrt

pub fn try_withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    amount: Uint128,
    sender: HumanAddr,
    msg: Option<Binary>,
) -> StdResult<HandleResponse> {
    let constants = read_config(&deps.storage)?;
    let mut messages: Vec<CosmosMsg> = vec![];

    if let Some(_msg) = msg {
        let _: WithdrawRequest = from_binary(&_msg).unwrap();
    } else {
        return Err(StdError::generic_err(
            "Withdraw must contain a valid withdraw message",
        ));
    }

    let kill_switch = KillSwitch::try_from(constants.kill_switch)?;

    if kill_switch == KillSwitch::Unbonding {
        return Err(StdError::generic_err(
                "Contract has been frozen. You must wait till unbonding has finished, then you will be able to withdraw your funds",
            ));
    }

    if kill_switch == KillSwitch::Open {
        return release_tokens(deps, &env, amount, sender);
    }

    let exch_rate = exchange_rate(&deps.storage, &deps.querier)?;

    // if amount.u128() < EXCHANGE_RATE_RESOLUTION as u128 {
    //     return Err(StdError::generic_err(
    //         "Can only withdraw a minimum of 1 uscrt",
    //     ));
    // }

    let unbond_amount = calc_withdraw(amount, exch_rate)?;
    debug_print(format!(
        "\x1B[34m ********* calculated withdraw as {} ****** \x1B[0m",
        unbond_amount,
    ));

    if unbond_amount < MINIMUM_WITHDRAW {
        return Err(StdError::generic_err(format!(
            "Amount withdrawn below minimum of {:?}uscrt",
            MINIMUM_WITHDRAW
        )));
    }

    perform_helper_claims(deps, &env, &constants, &mut messages)?;

    // let rewards = get_rewards_limited(
    //     &deps.querier,
    //     &env.contract.address,
    //     AMOUNT_OF_REWARDS_TO_HANDLE,
    // )?;

    // let rewards_amount = rewards
    //     .total
    //     .first()
    //     .unwrap_or(&Coin {
    //         denom: "".to_string(),
    //         amount: Default::default(),
    //     })
    //     .amount
    //     .u128();
    //
    // debug_print(format!(
    //     "\x1B[34m ********* calculated rewards as {} ****** \x1B[0m",
    //     &rewards_amount,
    // ));

    // check if we have to unbond, or do the available rewards cover this withdraw?
    // let mut unbond_amount = scrt_amount.saturating_sub(rewards_amount);
    // debug_print(format!(
    //     "\x1B[34m ********* unbond amount as {} ****** \x1B[0m",
    //     unbond_amount,
    // ));
    let scrt_coin = Coin {
        denom: "uscrt".to_string(),
        amount: Uint128::from(unbond_amount),
    };
    // if unbond_amount == 0 {
    //     let top_5_validators = rewards
    //         .rewards
    //         .iter()
    //         .map(|v| v.validator_address.0.clone())
    //         .collect();
    //
    //     messages.extend((&validator_set).withdraw_rewards_messages(Some(top_5_validators)));
    // }

    let mut window_manager = get_window_manager(&deps.storage)?;

    window_manager.withdraw(&mut deps.storage, &sender, Uint128::from(unbond_amount))?;

    let user_manager = UserWithdrawManager::new(window_manager.current_active_window);

    debug_print(format!(
        "appending user {} for a withdraw in window {}",
        &sender, &window_manager.current_active_window
    ));
    user_manager.append(&mut deps.storage, &sender)?;

    //perform_unbonding(&mut validator_set, &mut messages, unbond_amount);

    //
    // PendingWithdraws::append_withdraw(
    //     &mut deps.storage,
    //     &PendingWithdraw {
    //         available_time: env.block.time + constants.unbonding_time,
    //         receiver: sender.clone(),
    //         coins: scrt_coin.clone(),
    //     },
    //     &sender,
    // )?;

    // burn tokens

    messages.push(snip20::burn_msg(
        amount,
        None,
        256,
        constants.token_contract_hash.clone(),
        constants.token_contract.clone(),
    )?);

    if check_window_advance(&env, &window_manager) {
        // todo: decide if we should add incentive for this
        debug_print(format!(
            "**** advancing window {} ****",
            window_manager.current_active_window
        ));
        set_claim_time(
            &mut deps.storage,
            window_manager.current_active_window,
            &env.block.time + &constants.unbonding_time,
        )?;
        perform_window_unbond(deps, &env, &mut window_manager, &mut messages)?;
    }

    set_window_manager(&mut deps.storage, &window_manager)?;

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "withdraw"),
            log("account", env.message.sender.as_str()),
            log("amount", format!("{:?}", scrt_coin)),
        ],
        data: None,
    })
}

fn unbond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    messages: &mut Vec<CosmosMsg>,
    mut unbond_amount: u128,
) -> StdResult<()> {
    let mut validator_set = get_validator_set(&deps.storage)?;

    while unbond_amount > 0 {
        if let Some(validator) = validator_set.clone().next_to_unbond() {
            if validator.staked == 0 {
                // we can't unbond any more, no validator has any stake left!
                break;
            }

            let to_unbond = min(validator.staked as u128, unbond_amount);
            validator_set.unbond(to_unbond)?;
            validator_set.rebalance();
            messages.push(undelegate_msg(&validator.address, to_unbond));
            debug_print(format!(
                "\x1B[34m ********* undelegating {} from {} ****** \x1B[0m",
                to_unbond, &validator.address
            ));
            unbond_amount = unbond_amount.saturating_sub(to_unbond);
        }
    }

    set_validator_set(&mut deps.storage, &validator_set)
}

// optimization to immediately send if there's enough rewards in the pool - probably best to disable
// since this can complicate the UX for marginal gain
// if unbond_amount == 0 {
//     // restake the difference
//     let amount_to_stake = rewards - scrt_amount;
//
//     // if by some crazy chance this is exactly the same amount, we need to manually trigger a withdraw
//     if amount_to_stake == 0 {
//         messages.extend(&validator_set.withdraw_rewards_messages());
//     } else {
//         let validator = validator_set.stake(amount_to_stake)?;
//         validator_set.rebalance();
//         messages.push(stake_msg(&validator, amount_to_stake));
//     }
//
//     // and just send the funds immediately (no need to wait for unbonding in this case)
//     messages.push(CosmosMsg::Bank(BankMsg::Send {
//         from_address: env.contract.address.clone(),
//         to_address: sender,
//         amount: vec![scrt_coin.clone()],
//     }));
// } else {
// we might have to unbond from multiple validators

fn release_tokens<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    amount: Uint128,
    sender: HumanAddr,
) -> StdResult<HandleResponse> {
    let mut messages: Vec<CosmosMsg> = vec![];
    let constants = read_config(&deps.storage)?;

    debug_print(format!("** tokens withdrawn: {}", amount));
    let xrate = get_frozen_exchange_rate(&deps.storage)?;
    debug_print(format!("** Frozen exchange rate: {}", xrate.to_string()));
    let scrt_amount = calc_withdraw(amount, xrate)?;
    debug_print(format!("** SCRT amount withdrawn: {}", scrt_amount));
    let my_balance = get_balance(&deps.querier, &env.contract.address)?;
    debug_print(format!("** contract balance: {}", my_balance));

    let scrt_coin = Coin {
        denom: "uscrt".to_string(),
        amount: min(my_balance, Uint128::from(scrt_amount)),
    };

    messages.push(snip20::burn_msg(
        amount,
        None,
        256,
        constants.token_contract_hash,
        constants.token_contract,
    )?);

    messages.push(CosmosMsg::Bank(BankMsg::Send {
        from_address: env.contract.address.clone(),
        to_address: sender,
        amount: vec![scrt_coin.clone()],
    }));

    return Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "withdraw"),
            log("account", env.message.sender.as_str()),
            log("amount", format!("{:?}", scrt_coin)),
        ],
        data: None,
    });
}

/// Calculates how much your withdrawn tokens are worth in SCRT
/// Removes the balance from the total supply and balance
/// Returns amount of SCRT your tokens earned
pub fn calc_withdraw(amount: Uint128, exchange_rate: Decimal) -> StdResult<u128> {
    // do this to withdraw slightly less than actually worth - this will cover exchange_rate calculation errors
    let normalized_amount = amount.u128(); // / 10000 * 9999

    let raw_amount = Decimal::from(normalized_amount as u64) / exchange_rate;

    let coins_to_withdraw = raw_amount.to_u128().unwrap();

    Ok(coins_to_withdraw)
}

pub fn check_window_advance(env: &Env, window_manager: &WindowManager) -> bool {
    return window_manager.time_to_close_window <= env.block.time;
}

// perform withdraw
pub fn perform_window_unbond<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    window_manager: &mut WindowManager,
    messages: &mut Vec<CosmosMsg>,
) -> StdResult<()> {
    let withdraw_amount = window_manager.advance_window(env.block.time)?;

    unbond(deps, messages, withdraw_amount.amount.u128())
}
