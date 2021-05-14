use cosmwasm_std::{HumanAddr, StdError, StdResult, Storage};

use rust_decimal::Decimal;

pub const MAX_WITHDRAW_AMOUNT: u32 = 10;

pub const CONTRACT_ADDRESS: &[u8] = b"contract_address";
pub const FROZEN_EXCHANGE_RATE: &[u8] = b"FROZEN_EXCHANGE_RATE";

pub fn store_address<S: Storage>(storage: &mut S, address: &HumanAddr) {
    let address_bytes: Vec<u8> = bincode2::serialize(&address).unwrap();

    storage.set(&CONTRACT_ADDRESS, &address_bytes);
}

pub fn get_address<S: Storage>(storage: &S) -> StdResult<HumanAddr> {
    if let Some(address_bytes) = storage.get(&CONTRACT_ADDRESS) {
        let record: HumanAddr = bincode2::deserialize(&address_bytes).unwrap();
        Ok(record)
    } else {
        Err(StdError::GenericErr {
            msg: "Privacy token not available for this token".to_string(),
            backtrace: None,
        })
    }
}

pub fn store_frozen_exchange_rate<S: Storage>(storage: &mut S, xrate: &Decimal) {
    let address_bytes: Vec<u8> = bincode2::serialize(&xrate).unwrap();

    storage.set(&FROZEN_EXCHANGE_RATE, &address_bytes);
}

pub fn get_frozen_exchange_rate<S: Storage>(storage: &S) -> StdResult<Decimal> {
    if let Some(address_bytes) = storage.get(&FROZEN_EXCHANGE_RATE) {
        let record: Decimal = bincode2::deserialize(&address_bytes).unwrap();
        Ok(record)
    } else {
        Err(StdError::GenericErr {
            msg: "Privacy token not available for this token".to_string(),
            backtrace: None,
        })
    }
}
