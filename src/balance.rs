use crate::staking::{get_bonded, get_rewards};
use crate::state::{get_ratio, read_balance, read_constants, update_stored_balance};
use cosmwasm_std::{log, Api, Env, Extern, HandleResponse, Querier, StdResult, Storage};

pub fn try_balance<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let sender_address_raw = &env.message.sender;
    let account_balance = read_balance(&deps.storage, sender_address_raw);

    let consts = read_constants(&deps.storage)?;

    // this is here to return the same message if there is a 0 balance to not leak information
    if let Err(_e) = account_balance {
        Ok(HandleResponse {
            messages: vec![],
            log: vec![
                log("action", "balance"),
                log(
                    "account",
                    deps.api.human_address(&env.message.sender)?.as_str(),
                ),
                log("amount", "0"),
            ],
            data: None,
        })
    } else {
        let printable_token = crate::contract::to_display_token(
            account_balance.unwrap(),
            &consts.symbol,
            consts.decimals,
        );

        Ok(HandleResponse {
            messages: vec![],
            log: vec![
                log("action", "balance"),
                log(
                    "account",
                    deps.api.human_address(&env.message.sender)?.as_str(),
                ),
                log("amount", printable_token),
            ],
            data: None,
        })
    }
}

pub fn refresh_balances<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let contract = deps.api.human_address(&env.contract.address)?;

    let balance = get_bonded(&deps.querier, &contract)?;
    let rewards_balance = get_rewards(&deps.querier, &contract)?;

    update_stored_balance(&mut deps.storage, balance.u128() + rewards_balance.u128());

    let ratio = get_ratio(&deps.storage)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("ratio", format!("{:?}", ratio))],
        data: None,
    })
}
