use cosmwasm_std::{Coin, ReadonlyStorage, StdResult, Storage};
use secret_toolkit::storage::{TypedStore, TypedStoreMut};
use serde::{Deserialize, Serialize};

use crate::utils::u64_to_bytes;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct WithdrawWindow {
    //pub id: u64,
    //pub available_time: u64,
    //pub withdraw_started: bool,
    pub coins: Coin,
}

pub fn set_claim_time<S: Storage>(store: &mut S, window: u64, time: u64) -> StdResult<()> {
    let mut typed_store = TypedStoreMut::attach(store);
    typed_store.store(&u64_to_bytes(&window), &time)
}

pub fn get_claim_time<S: ReadonlyStorage>(store: &S, window: u64) -> Option<u64> {
    let typed_store = TypedStore::attach(store);
    let result = typed_store.may_load(&u64_to_bytes(&window));

    if result.is_err() {
        return None;
    }

    result.unwrap()
}

//pub fn get_window_manager<S: ReadonlyStorage>(store: &S) -> StdResult<WindowManager> {
//     let config_store = ReadonlyPrefixedStorage::new(PREFIX_WINDOW_MANANGER, store);
//     let x = config_store.get(&PREFIX_WINDOW_MANANGER).unwrap();
//     let record: WindowManager = bincode2::deserialize(&x)
//         .map_err(|_| StdError::generic_err("Error unpacking validator set"))?;
//     Ok(record)
// }
//
// pub fn set_window_manager<S: Storage>(store: &mut S) -> StdResult<()> {
//     let mut config_store = PrefixedStorage::new(PREFIX_WINDOW_MANANGER, store);
//     let as_bytes = bincode2::serialize(validator_address)
//         .map_err(|_| StdError::generic_err("Error packing validator set"))?;
//
//     config_store.set(PREFIX_WINDOW_MANANGER, &as_bytes);
//
//     Ok(())
// }
