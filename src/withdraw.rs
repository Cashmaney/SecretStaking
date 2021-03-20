use secret_toolkit::snip20;

//use crate::liquidity_pool::update_exchange_rate_message;
use crate::staking::{exchange_rate, get_balance, get_rewards, stake_msg, undelegate_msg};
use crate::state::{
    get_frozen_exchange_rate, read_config, KillSwitch, PendingWithdraw, PendingWithdraws,
};
use crate::validator_set::{get_validator_set, set_validator_set};
use cosmwasm_std::{
    log, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse, HumanAddr, Querier, StdError,
    StdResult, Storage, Uint128,
};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::cmp::min;

pub fn try_withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    amount: Uint128,
    sender: HumanAddr,
) -> StdResult<HandleResponse> {
    let mut validator_set = get_validator_set(&deps.storage)?;
    let constants = read_config(&deps.storage)?;
    let mut messages: Vec<CosmosMsg> = vec![];

    if constants.kill_switch == KillSwitch::Unbonding {
        return Err(StdError::generic_err(
                "Contract has been frozen. You must wait till unbonding has finished, then you will be able to withdraw your funds",
            ));
    }

    if constants.kill_switch == KillSwitch::Open {
        let xrate = get_frozen_exchange_rate(&deps.storage)?;

        let scrt_amount = calc_withdraw(amount, xrate)?;

        let my_balance = get_balance(&deps.querier, &env.contract.address)?;

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

    let exch_rate = exchange_rate(&deps.storage, &deps.querier)?;

    // if amount.u128() < EXCHANGE_RATE_RESOLUTION as u128 {
    //     return Err(StdError::generic_err(
    //         "Can only withdraw a minimum of 1 uscrt",
    //     ));
    // }

    let scrt_amount = calc_withdraw(amount, exch_rate)?;

    let rewards = get_rewards(&deps.querier, &env.contract.address)?.u128();

    messages.append(&mut validator_set.withdraw_rewards_messages());

    // check if we have to unbond, or do the available rewards cover this withdraw?
    let mut unbond_amount = scrt_amount.saturating_sub(rewards);

    let scrt_coin = Coin {
        denom: "uscrt".to_string(),
        amount: Uint128::from(scrt_amount),
    };

    if unbond_amount == 0 {
        // restake the difference
        let amount_to_stake = rewards.saturating_sub(scrt_amount);
        let validator = validator_set.stake(amount_to_stake)?;
        validator_set.rebalance();
        messages.push(stake_msg(&validator, amount_to_stake));

        // and just send the funds immediately (no need to wait for unbonding in this case)
        messages.push(CosmosMsg::Bank(BankMsg::Send {
            from_address: env.contract.address.clone(),
            to_address: sender,
            amount: vec![scrt_coin.clone()],
        }));
    } else {
        // we might have to unbond from multiple validators
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

                unbond_amount = unbond_amount.saturating_sub(to_unbond);
            }
        }

        let mut pending_withdraws = PendingWithdraws::load(&deps.storage);
        pending_withdraws.append(PendingWithdraw {
            available_time: env.block.time + constants.unbonding_time,
            receiver: sender,
            coins: scrt_coin.clone(),
        });
        pending_withdraws.save(&mut deps.storage);
    }

    // burn tokens

    messages.push(snip20::burn_msg(
        amount,
        None,
        256,
        constants.token_contract_hash,
        constants.token_contract,
    )?);

    set_validator_set(&mut deps.storage, &validator_set)?;

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

/// Calculates how much your withdrawn tokens are worth in SCRT
/// Removes the balance from the total supply and balance
/// Returns amount of SCRT your tokens earned
pub fn calc_withdraw(amount: Uint128, exchange_rate: Decimal) -> StdResult<u128> {
    let raw_amount = Decimal::from(amount.u128() as u64) / exchange_rate;

    let coins_to_withdraw = raw_amount.to_u128().unwrap();

    Ok(coins_to_withdraw)
}
