use crate::types::activation_fee::{read_activation_fee, set_activation_fee};
use crate::types::config::read_config;
use crate::types::window_manager::{get_window_manager, set_window_manager};
use crate::types::withdraw_window::set_claim_time;
use crate::withdraw::{check_window_advance, perform_window_unbond};
use cosmwasm_std::{
    log, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse, Querier, StdError, StdResult,
    Storage, Uint128,
};
use rust_decimal::prelude::Zero;

pub fn advance_window<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let mut messages = vec![];
    let constants = read_config(&deps.storage)?;
    let mut window_manager = get_window_manager(&deps.storage)?;
    let fee_for_activation;
    if check_window_advance(&env, &window_manager) {
        set_claim_time(
            &mut deps.storage,
            window_manager.current_active_window,
            &env.block.time + &constants.unbonding_time,
        )?;
        perform_window_unbond(deps, &env, &mut window_manager, &mut messages)?;

        fee_for_activation = read_activation_fee(&deps.storage)?;

        if fee_for_activation > 0 {
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                from_address: env.contract.address,
                to_address: env.message.sender.clone(),
                amount: vec![Coin {
                    denom: "uscrt".to_string(),
                    amount: Uint128::from(fee_for_activation),
                }],
            }));

            set_activation_fee(&mut deps.storage, &u64::zero())?;
        }

        set_window_manager(&mut deps.storage, &window_manager)?;
    } else {
        return Err(StdError::generic_err("Advance window not available yet"));
    }

    Ok(HandleResponse {
        messages,
        log: vec![
            log("action", "advance_window"),
            log("account", env.message.sender.as_str()),
            log("amount", format!("{:?}", fee_for_activation)),
        ],
        data: None,
    })
}
