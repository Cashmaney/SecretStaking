use crate::types::pending_withdraws::PendingWithdraws;
use cosmwasm_std::{
    debug_print, log, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse, HumanAddr,
    Querier, StdError, StdResult, Storage, Uint128,
};

pub fn claim<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let (sum_withdraws, messages) = _claim_withdraws(deps, &env)?;

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

fn _claim_withdraws<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
) -> StdResult<(u128, Vec<CosmosMsg>)> {
    //let mut pending_withdraws = PendingWithdraws::load(&mut deps.storage)?;
    let mut sum_withdraws = 0;

    let mut messages: Vec<CosmosMsg> = vec![];

    let contract_balance = &deps.querier.query_balance(&env.contract.address, "uscrt")?;

    let mut pending_withdraws = PendingWithdraws::load(&mut deps.storage, &env.message.sender);

    let withdraws_before = pending_withdraws.len();

    let expired = pending_withdraws.remove_expired(env.block.time);
    debug_print(format!("Claiming {} matured withdraws", expired.len()));

    if !expired.is_empty() {
        for withdraw in &expired {
            sum_withdraws += withdraw.coins.amount.u128();
        }

        // todo: allow withdraw to different account? If not, just make this msg.sender
        let receiver = expired.first().unwrap().receiver.clone();

        messages.push(CosmosMsg::Bank(BankMsg::Send {
            from_address: env.contract.address.clone(),
            to_address: receiver,
            amount: vec![Coin {
                denom: "uscrt".to_string(),
                amount: Uint128(sum_withdraws),
            }],
        }));

        if (expired.len() + pending_withdraws.len()) != withdraws_before {
            return Err(StdError::generic_err(
                "Withdraw length invariant difference",
            ));
        }

        debug_print(format!(
            "Saving modified pending withdraws of length: {}",
            pending_withdraws.len()
        ));
        pending_withdraws.save(&mut deps.storage, &env.message.sender)?;
    }

    //if !expired.is_empty() {
    //             for withdraw in expired {
    //                 sum_withdraws += withdraw.coins.amount.u128();
    //                 messages.push(CosmosMsg::Bank(BankMsg::Send {
    //                     from_address: env.contract.address.clone(),
    //                     to_address: withdraw.receiver,
    //                     amount: vec![withdraw.coins],
    //                 }));
    //             }
    //             pending_withdraws.save(&mut deps.storage);
    //         }
    // if all {
    //
    // } else {

    //}

    debug_print(format!(
        "sum of withdraws: {}. Current balance: {}",
        sum_withdraws, contract_balance.amount
    ));

    Ok((sum_withdraws, messages))
}

fn _claim_multiple_withdraws<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    amount: u32,
) -> StdResult<(u128, Vec<CosmosMsg>)> {
    //let mut pending_withdraws = PendingWithdraws::load(&mut deps.storage)?;
    let mut sum_withdraws = 0;

    let mut messages: Vec<CosmosMsg> = vec![];

    let pending_withdraws: Vec<PendingWithdraws> =
        PendingWithdraws::get_multiple(&mut deps.storage, amount)?;

    if pending_withdraws.is_empty() {
        return Ok((0u128, vec![]));
    }

    let contract_balance = &deps.querier.query_balance(&env.contract.address, "uscrt")?;

    for mut pending in pending_withdraws {
        let expired = pending.remove_expired(env.block.time);
        if !expired.is_empty() {
            let mut receiver: HumanAddr = HumanAddr::default();
            for withdraw in expired {
                receiver = withdraw.receiver.clone();
                sum_withdraws += withdraw.coins.amount.u128();
                messages.push(CosmosMsg::Bank(BankMsg::Send {
                    from_address: env.contract.address.clone(),
                    to_address: withdraw.receiver,
                    amount: vec![withdraw.coins],
                }));
            }
            pending.save(&mut deps.storage, &receiver)?;
        }
    }

    debug_print(format!(
        "remove multiple: sum of withdraws: {}. Current balance: {}",
        sum_withdraws, contract_balance.amount
    ));

    Ok((sum_withdraws, messages))
}
