//use crate::types::pending_withdraws::PendingWithdraws;
use crate::types::user_withdraws::{all_waiting_withdraws_for_user, get_withdraw_for_user};
use crate::types::user_withdraws::{
    get_active_withdraw_window, set_active_withdraw_window, UserWithdrawManager,
};
use crate::types::window_manager::get_window_manager;
use crate::types::withdraw_window::get_claim_time;
use cosmwasm_std::{
    debug_print, log, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse, HumanAddr,
    Querier, StdResult, Storage,
};

pub fn claim<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let (sum_withdraws, messages) = _claim_withdraws_for_sender(deps, &env)?;

    let res = HandleResponse {
        messages,
        log: vec![
            log("action", "claim"),
            log("account", env.message.sender.as_str()),
            log("amount", format!("{:?}", sum_withdraws)),
        ],
        data: None,
    };

    Ok(res)
}

pub fn claim_multiple<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    amount: u32,
) -> StdResult<HandleResponse> {
    let (sum_withdraws, messages) = _claim_multiple_withdraws(deps, env, amount)?;

    debug_print(format!(
        "Claiming multiple withdraws: {} for a total of {}",
        messages.len(),
        sum_withdraws
    ));

    let res = HandleResponse {
        messages,
        log: vec![
            log("action", "claim_all"),
            log("account", env.message.sender.as_str()),
            log("amount", format!("{:?}", sum_withdraws)),
        ],
        data: None,
    };

    Ok(res)
}

fn _claim_withdraws_for_sender<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
) -> StdResult<(u128, Vec<CosmosMsg>)> {
    let withdraws = all_waiting_withdraws_for_user(&deps.storage, &env.message.sender);

    let manager = get_window_manager(&deps.storage)?;

    let active_window = manager.current_active_window;

    let mut todo_withdraws = vec![];
    for withdraw in withdraws.0 {
        if withdraw.id < active_window {
            todo_withdraws.push(withdraw.id);
        }
    }

    if todo_withdraws.is_empty() {
        return Ok((0, vec![]));
    }

    let mut sum_withdraws = 0;

    let mut messages: Vec<CosmosMsg> = vec![];

    // todo: loop over all windows (a newer window may be available for claiming)
    for window in todo_withdraws {
        let active_time = get_claim_time(&deps.storage, window);
        if let Some(time) = active_time {
            // nothing to do, skip
            // todo: make this return an Option
            if time > env.block.time {
                continue;
            }
        } else {
            continue;
        }

        let mut withdraw_manager = UserWithdrawManager::new(window);

        let found = withdraw_manager.remove_address(&mut deps.storage, &env.message.sender)?;

        if !found {
            debug_print(format!("****** user not found in this window *********",));
            continue;
        }

        debug_print(format!(
            "****** found user {} in window {} *********",
            &env.message.sender, &window
        ));

        let (withdraws, temp_messages) = _do_claim(deps, &env, window, &env.message.sender)?;
        sum_withdraws += withdraws;
        messages.extend(temp_messages);
    }

    let contract_balance = &deps.querier.query_balance(&env.contract.address, "uscrt")?;
    debug_print(format!(
        "sum of withdraws: {}. Current balance: {}",
        sum_withdraws, contract_balance.amount
    ));

    Ok((sum_withdraws, messages))
}

fn _do_claim<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    window: u64,
    address: &HumanAddr,
) -> StdResult<(u128, Vec<CosmosMsg>)> {
    let mut messages = vec![];

    let coin_to_withdraw = get_withdraw_for_user(&mut deps.storage, address, window)?;

    return if let Some(coin) = coin_to_withdraw {
        debug_print(format!(
            "Withdrawing {} for window {} for user {}",
            coin.amount.clone(),
            window,
            address
        ));

        messages.push(CosmosMsg::Bank(BankMsg::Send {
            from_address: env.contract.address.clone(),
            to_address: address.clone(),
            amount: vec![Coin {
                denom: "uscrt".to_string(),
                amount: coin.amount.clone(),
            }],
        }));

        Ok((coin.amount.u128(), messages))
    } else {
        debug_print(format!(
            "No withdraw for window {} for user {}",
            window, address
        ));
        Ok((0, vec![]))
    };
}

fn _claim_multiple_withdraws<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    amount: u32,
) -> StdResult<(u128, Vec<CosmosMsg>)> {
    //let mut pending_withdraws = PendingWithdraws::load(&mut deps.storage)?;
    let mut sum_withdraws = 0;

    let mut messages: Vec<CosmosMsg> = vec![];

    let window = get_active_withdraw_window(&deps.storage)?;
    debug_print(format!(
        "****** active withdraw window: {} *********",
        window
    ));

    let maybe_activation_time = get_claim_time(&deps.storage, window);

    if let Some(activation_time) = maybe_activation_time {
        // nothing to do, skip
        // todo: make this return an Option
        debug_print(format!(
            "****** window: {}, activation time: {}, current time: {} *********",
            window, activation_time, env.block.time
        ));

        if activation_time > env.block.time {
            return Ok((0, vec![]));
        }
    } else {
        // if time is undefined, don't do anything either (we might be ahead)
        return Ok((0, vec![]));
    }

    let withdraw_manager = UserWithdrawManager::new(window);

    let users = withdraw_manager.get_many(&mut deps.storage, amount)?;
    debug_print(format!(
        "****** got {} users from manager: {:?} *********",
        users.len(),
        &users
    ));

    for user in users {
        let (withdraws, temp_messages) = _do_claim(deps, &env, window, &user)?;
        sum_withdraws += withdraws;
        messages.extend(temp_messages);
    }

    debug_print(format!(
        "****** withdraw_manager length: {} *********",
        withdraw_manager.len(&mut deps.storage)
    ));

    if withdraw_manager.len(&mut deps.storage) == 0 {
        debug_print(format!(
            "****** setting active window as: {} *********",
            &(window + 1)
        ));

        set_active_withdraw_window(&mut deps.storage, &(window + 1))?;
    }

    // let pending_withdraws: Vec<PendingWithdraws> =
    //     PendingWithdraws::get_multiple(&mut deps.storage, amount)?;
    //
    // debug_print(format!(
    //     "Loaded multiple withdraws: {}",
    //     pending_withdraws.len()
    // ));
    //
    // if pending_withdraws.is_empty() {
    //     return Ok((0u128, vec![]));
    // }

    // debug
    // if pending_withdraws.len() > 1 {
    //     let r1 = pending_withdraws[0].pending()[0].clone().receiver;
    //     let r2 = pending_withdraws[1].pending()[0].clone().receiver;
    //     debug_print(format!("multiple withdraw for {} and {}", r1, r2));
    // }

    // for pending in pending_withdraws {
    //     if pending.len() != 0 {
    //         debug_print(format!("withdrawing for {}", pending.pending()[0].receiver));
    //         let (withdraws, temp_messages) = _do_claim(deps, env, pending)?;
    //
    //         sum_withdraws += withdraws;
    //         messages.extend(temp_messages);
    //     }
    // }

    let contract_balance = &deps.querier.query_balance(&env.contract.address, "uscrt")?;
    debug_print(format!(
        "remove multiple: sum of withdraws: {}. Current balance: {}",
        sum_withdraws, contract_balance.amount
    ));

    Ok((sum_withdraws, messages))
}

// to claim:

//
