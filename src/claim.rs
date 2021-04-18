use crate::state::PendingWithdraws;
use cosmwasm_std::{
    debug_print, log, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse, Querier,
    StdResult, Storage, Uint128,
};

pub fn claim<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let (sum_withdraws, messages) = _claim_withdraws(deps, &env, false)?;

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
    let (sum_withdraws, messages) = _claim_withdraws(deps, &env, true)?;

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
) -> StdResult<(u128, Vec<CosmosMsg>)> {
    //let mut pending_withdraws = PendingWithdraws::load(&deps.storage)?;
    let mut sum_withdraws = 0;

    let mut messages: Vec<CosmosMsg> = vec![];

    let contract_balance = &deps.querier.query_balance(&env.contract.address, "uscrt")?;

    let expired = if all {
        pending_withdraws.remove_expired(env.block.time)
    } else {
        let mut pending_withdraws =
            PendingWithdraws::load_by_address(&deps.storage, &env.message.sender)?;
        pending_withdraws.remove_expired_by_sender(env.block.time, &env.message.sender)
    };

    if all {
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
    } else {
        if !expired.is_empty() {
            for withdraw in &expired {
                sum_withdraws += withdraw.coins.amount.u128();
            }
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                from_address: env.contract.address.clone(),
                to_address: expired.first().unwrap().receiver.clone(),
                amount: vec![Coin {
                    denom: "uscrt".to_string(),
                    amount: Uint128(sum_withdraws),
                }],
            }));
            //pending_withdraws.save(&mut deps.storage);
        }
    }

    debug_print(format!(
        "sum of withdraws: {}. Current balance: {}",
        sum_withdraws, contract_balance.amount
    ));

    Ok((sum_withdraws, messages))
}
