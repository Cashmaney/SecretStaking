use crate::types::pending_withdraws::PendingWithdraws;
use cosmwasm_std::{
    debug_print, log, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse, Querier,
    StdError, StdResult, Storage, Uint128,
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

fn _claim_withdraws<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
) -> StdResult<(u128, Vec<CosmosMsg>)> {
    let pending_withdraws = PendingWithdraws::load(&deps.storage, &env.message.sender);

    let withdraws_before = pending_withdraws.len();
    debug_print(format!(
        "Got pending withdraws of length {}",
        withdraws_before
    ));

    let (sum_withdraws, messages) = _do_claim(deps, env, pending_withdraws)?;

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
    mut pending_withdraws: PendingWithdraws,
) -> StdResult<(u128, Vec<CosmosMsg>)> {
    let withdraws_before = pending_withdraws.len();
    let mut messages = vec![];
    let mut sum_withdraws = 0;

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
            to_address: receiver.clone(),
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
            "Saving modified pending withdraws of length: {} for address {}",
            pending_withdraws.len(),
            receiver
        ));
        pending_withdraws.save(&mut deps.storage, &receiver)?;
    }

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

    debug_print(format!(
        "Loaded multiple withdraws: {}",
        pending_withdraws.len()
    ));

    if pending_withdraws.is_empty() {
        return Ok((0u128, vec![]));
    }

    // debug
    if pending_withdraws.len() > 1 {
        let r1 = pending_withdraws[0].pending()[0].clone().receiver;
        let r2 = pending_withdraws[1].pending()[0].clone().receiver;
        debug_print(format!("multiple withdraw for {} and {}", r1, r2));
    }

    for pending in pending_withdraws {
        if pending.len() != 0 {
            debug_print(format!("withdrawing for {}", pending.pending()[0].receiver));
            let (withdraws, temp_messages) = _do_claim(deps, env, pending)?;

            sum_withdraws += withdraws;
            messages.extend(temp_messages);
        }
    }

    // let contract_balance = &deps.querier.query_balance(&env.contract.address, "uscrt")?;
    // debug_print(format!(
    //     "remove multiple: sum of withdraws: {}. Current balance: {}",
    //     sum_withdraws, contract_balance.amount
    // ));

    Ok((sum_withdraws, messages))
}
