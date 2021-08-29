use std::cmp::min;

use cosmwasm_std::{Coin, HumanAddr, ReadonlyStorage, StdError, StdResult, Storage};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};
use schemars::JsonSchema;
use secret_toolkit::storage::{AppendStore, AppendStoreMut};
use serde::{Deserialize, Serialize};

use cargo_common::cashmap::{CashMap, ReadOnlyCashMap};

use crate::utils::{address_to_bytes, u64_to_bytes};

pub const USER_WITHDRAWS: &[u8] = b"WITHDRAWERS";

pub const WITHDRAWERS_FOR_WINDOW: &[u8] = b"WFORW";

pub const WITHDRAW_WINDOW: &[u8] = b"WWIN";

#[derive(Serialize, Deserialize, Clone, Debug, Default, JsonSchema)]
pub struct WaitingWithdraw {
    pub id: u64,
    pub coins: Coin,
}

// #[derive(Serialize, Deserialize, Clone, Debug, Default)]
// pub struct AllWindows(pub Vec<WithdrawWindow>);

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct UserWithdraws(pub Vec<WaitingWithdraw>);

impl UserWithdraws {
    pub fn position(&self, window_id: &u64) -> Option<usize> {
        self.0.iter().position(|item| &item.id == window_id)
    }
}

/// appends a new withdrawer to the current window

// pub fn get_next_users_for_window<S: Storage>(
//     store: &mut S,
//     window: u64,
//     amount: u32,
// ) -> StdResult<Vec<HumanAddr>> {
//     let mut store =
//         PrefixedStorage::multilevel(&[WITHDRAWERS_FOR_WINDOW, u64_to_bytes(&window)], store);
//     let mut store = AppendStoreMut::attach_or_create(&mut store)?;
//
//     let mut users: Vec<HumanAddr> = vec![];
//
//     if store.len() == 0 {
//         return Ok(users);
//     }
//
//     for i in 1..amount {
//         let user = store.pop()?;
//     }
//
//     return Ok(Some(user));
// }

pub struct UserWithdrawManager {
    pub window: u64,
}

impl UserWithdrawManager {
    pub fn new(window: u64) -> Self {
        return Self { window };
    }

    pub fn len<S: Storage>(&self, store: &S) -> u32 {
        let store = ReadonlyPrefixedStorage::multilevel(
            &[WITHDRAWERS_FOR_WINDOW, &u64_to_bytes(&self.window)],
            store,
        );
        let store: Option<StdResult<AppendStore<HumanAddr, ReadonlyPrefixedStorage<S>>>> =
            AppendStore::attach(&store);

        if let Some(store_result) = store {
            if store_result.is_err() {
                return 0;
            }
            return store_result.unwrap().len();
        }
        return 0;
    }

    pub fn append<S: Storage>(&self, store: &mut S, user: &HumanAddr) -> StdResult<()> {
        let mut store = PrefixedStorage::multilevel(
            &[WITHDRAWERS_FOR_WINDOW, &u64_to_bytes(&self.window)],
            store,
        );
        let mut store = AppendStoreMut::attach_or_create(&mut store)?;

        let mut iter = store.iter();
        if iter.position(|p| &p.unwrap_or_default() == user).is_none() {
            return store.push(user);
        }
        Ok(())
    }

    pub fn get_many<S: Storage>(&self, storage: &mut S, amount: u32) -> StdResult<Vec<HumanAddr>> {
        let mut address_store = PrefixedStorage::multilevel(
            &[WITHDRAWERS_FOR_WINDOW, &u64_to_bytes(&self.window)],
            storage,
        );
        let mut address_store = AppendStoreMut::attach_or_create(&mut address_store)?;

        let mut users: Vec<HumanAddr> = vec![];

        let max_users = address_store.len();

        if max_users == 0 || amount == 0 {
            return Ok(users);
        }

        let returned_addresses = min(max_users, amount);

        for _i in 0..returned_addresses {
            let user = address_store.pop()?;
            users.push(user);
        }

        return Ok(users);
    }

    pub fn remove_address<S: Storage>(
        &mut self,
        storage: &mut S,
        address: &HumanAddr,
    ) -> StdResult<bool> {
        let mut address_store = PrefixedStorage::multilevel(
            &[WITHDRAWERS_FOR_WINDOW, &u64_to_bytes(&self.window)],
            storage,
        );
        let mut address_store: AppendStoreMut<HumanAddr, PrefixedStorage<S>> =
            AppendStoreMut::attach_or_create(&mut address_store)?;

        let found = address_store
            .iter()
            .position(|p| &p.unwrap_or_default() == address);

        return if let Some(pos) = found {
            let last = address_store.pop()?;

            if &last != address {
                address_store.set_at(pos as u32, &last)?;
            }
            Ok(true)
        } else {
            Ok(false)
        };
    }
}

//
pub fn get_active_withdraw_window<S: ReadonlyStorage>(store: &S) -> StdResult<u64> {
    let config_store = ReadonlyPrefixedStorage::new(WITHDRAW_WINDOW, store);
    let x = config_store.get(WITHDRAW_WINDOW).unwrap();
    let record: u64 = bincode2::deserialize(&x)
        .map_err(|_| StdError::generic_err("Error unpacking validator set"))?;
    Ok(record)
}

pub fn set_active_withdraw_window<S: Storage>(store: &mut S, window: &u64) -> StdResult<()> {
    let mut config_store = PrefixedStorage::new(WITHDRAW_WINDOW, store);
    let as_bytes = bincode2::serialize(window)
        .map_err(|_| StdError::generic_err("Error packing validator set"))?;

    config_store.set(WITHDRAW_WINDOW, &as_bytes);

    Ok(())
}

//               let item = windows.0.get_mut(active_window).unwrap();
//                item.coins.amount

// terms:
//
// pending withdraw = waiting for withdraw window
// bonded withdraw = waiting for 21 days
// claimable withdraw = can claim

/* withdraw ->
  withdraw window amount += deposit amount

  we need 2 structs: withdrawers (vec of addresses with active withdraws)
                     hashmap of withdraws (address -> withdraws)
           withdraws is a vec (max N items) of the current pending withdraws, organized by window

  check if address has an open pending withdraw.
  if yes: add the amount to his current pending withdraw and exit

  if no: create a new pending withdraw for this withdraw window and append name to withdrawers

  deposit -> stays the same

*/

pub fn get_withdraw_for_user<S: Storage>(
    storage: &mut S,
    address: &HumanAddr,
    window: u64,
) -> StdResult<Option<Coin>> {
    let mut cashmap: CashMap<UserWithdraws, S> = CashMap::init(USER_WITHDRAWS, storage);

    let mut windows = cashmap.get(address_to_bytes(address)).unwrap_or_default();

    //
    let withdraw = windows
        .0
        .drain_filter(|p| p.id == window)
        .collect::<Vec<_>>();

    // user has no withdraws
    if withdraw.len() == 0 {
        return Ok(None);
    }

    // todo: remove this check after testing
    if withdraw.len() > 1 {
        return Err(StdError::generic_err(
            "User has more than 1 withdraw for this window (should never happen)",
        ));
    }

    cashmap.insert(address_to_bytes(address), windows)?;

    Ok(Some(withdraw[0].coins.clone()))
}

pub fn all_waiting_withdraws_for_user<S: ReadonlyStorage>(
    storage: &S,
    address: &HumanAddr,
) -> UserWithdraws {
    let cashmap: ReadOnlyCashMap<UserWithdraws, S> = ReadOnlyCashMap::init(USER_WITHDRAWS, storage);

    let windows = cashmap.get(address_to_bytes(address)).unwrap_or_default();

    windows
}
