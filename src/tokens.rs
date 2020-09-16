use cosmwasm_std::{
    log, Api, Binary, CosmosMsg, Env, Extern, HandleResponse, HumanAddr, Querier, StdResult,
    Storage, Uint128, WasmMsg,
};

use crate::staking::{get_bonded, get_rewards};
use crate::state::{get_exchange_rate, read_constants, read_token_balance};

pub fn mint<S: Storage>(store: &S, amount: Uint128, account: HumanAddr) -> StdResult<CosmosMsg> {
    let constants = read_constants(store)?;

    return Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: constants.token_contract,
        callback_code_hash: constants.token_contract_hash,
        msg: Binary(
            format!(
                r#"{{"mint": {{"account":{}, "amount": {} }} }}"#,
                account.to_string(),
                amount.to_string()
            )
            .as_bytes()
            .to_vec(),
        ),
        send: vec![],
    }));
}

pub fn burn<S: Storage>(store: &S, amount: Uint128) -> StdResult<CosmosMsg> {
    let constants = read_constants(store)?;

    return Ok(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: constants.token_contract,
        callback_code_hash: constants.token_contract_hash,
        msg: Binary(
            format!(r#"{{"burn": {{"amount": {} }} }}"#, amount.to_string())
                .as_bytes()
                .to_vec(),
        ),
        send: vec![],
    }));
}
