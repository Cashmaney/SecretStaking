use std::convert::TryInto;

use cosmwasm_std::{generic_err, Binary, CosmosMsg, HumanAddr, StdResult, WasmMsg};

// Converts 16 bytes value into u128
// Errors if data found that is not 16 bytes
pub fn bytes_to_u128(data: &[u8]) -> StdResult<u128> {
    match data[0..16].try_into() {
        Ok(bytes) => Ok(u128::from_be_bytes(bytes)),
        Err(_) => Err(generic_err("Corrupted data found. 16 byte expected.")),
    }
}

pub fn callback_update_balances(contract_address: &HumanAddr, code_hash: &String) -> CosmosMsg {
    CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: contract_address.clone(),
        callback_code_hash: code_hash.clone(),
        msg: Binary("{\"update_balances\":{}}".as_bytes().to_vec()),
        send: vec![],
    })
}
