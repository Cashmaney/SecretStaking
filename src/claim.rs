use crate::state::PendingWithdraws;
use cosmwasm_std::{
    log, Api, BankMsg, CosmosMsg, Env, Extern, HandleResponse, Querier, StdResult, Storage,
};

pub fn claim<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let (sum_withdraws, messages) = _claim_withdraws(deps, &env, false);

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

pub fn claim_all<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let (sum_withdraws, messages) = _claim_withdraws(deps, &env, true);

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
    all: bool,
) -> (u128, Vec<CosmosMsg>) {
    let mut pending_withdraws = PendingWithdraws::load(&deps.storage);

    let expired = if all {
        pending_withdraws.remove_expired(env.block.time)
    } else {
        pending_withdraws.remove_expired_by_sender(env.block.time, &env.message.sender)
    };

    let mut sum_withdraws = 0;

    let mut messages: Vec<CosmosMsg> = vec![];

    if !expired.is_empty() {
        for withdraw in expired {
            sum_withdraws += withdraw.coins.amount.u128();
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                from_address: env.contract.address.clone(),
                to_address: withdraw.receiver,
                amount: vec![withdraw.coins],
            }));
        }

        pending_withdraws.save(&mut deps.storage);
    }
    (sum_withdraws, messages)
}
