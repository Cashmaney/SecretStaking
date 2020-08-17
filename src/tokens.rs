use cosmwasm_std::{
    log, Api, Binary, CosmosMsg, Env, Extern, HandleResponse, HumanAddr, Querier, StdResult,
    Storage, WasmMsg,
};

use crate::staking::{get_bonded, get_rewards};
use crate::state::{get_exchange_rate, read_constants, read_token_balance};

pub fn try_balance<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let sender_address_raw = &env.message.sender;
    let account_balance = read_token_balance(&deps.storage, sender_address_raw);

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
