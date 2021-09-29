use cosmwasm_std::{
    debug_print, Coin, HumanAddr, ReadonlyStorage, StdError, StdResult, Storage, Uint128,
};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};

use serde::{Deserialize, Serialize};

use cargo_common::cashmap::CashMap;

use crate::constants::{NATIVE_TOKEN_DENOM, WINDOW_TIME};
use crate::types::user_withdraws::{UserWithdraws, WaitingWithdraw, USER_WITHDRAWS};
use crate::types::withdraw_window::WithdrawWindow;
use crate::utils::address_to_bytes;

pub const PREFIX_WINDOW_MANANGER: &[u8] = b"WINDOW_MANAGER";

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct WindowManager {
    //pub window_cycle: u64, // cycle?
    pub current_active_window: u64,
    pub time_to_close_window: u64,
    pub window: WithdrawWindow,
}

impl WindowManager {
    // pub fn has_waiting_withdraw<S: Storage>(&self, storage: &mut S, address: &HumanAddr) -> bool {
    //     let mut cashmap: CashMap<UserWithdraws, S> = CashMap::init(USER_WITHDRAWS, storage);
    //
    //     let x = cashmap.get(address_to_bytes(address));
    //
    //     if let Some(mut windows) = x {
    //         return windows.position(&self.current_active_window).is_some();
    //     }
    //
    //     false
    // }

    pub fn advance_window(&mut self, current_time: u64) -> StdResult<Coin> {
        self.current_active_window += 1;

        self.time_to_close_window = current_time + WINDOW_TIME;

        let amount_to_withdraw = self.window.coins.clone();

        self.window.coins.amount = Uint128::zero();

        // set_window_manager(storage, &self)?;

        return Ok(amount_to_withdraw);
    }

    pub fn withdraw<S: Storage>(
        &mut self,
        storage: &mut S,
        address: &HumanAddr,
        amount: Uint128,
    ) -> StdResult<()> {
        self.window.coins.amount += amount;

        let mut cashmap: CashMap<UserWithdraws, S> = CashMap::init(USER_WITHDRAWS, storage);

        let user_key = address_to_bytes(address);

        let x = cashmap.get(user_key);

        if let Some(mut user_withdraws) = x {
            if let Some(active_window) = user_withdraws.position(&self.current_active_window) {
                // user already has active withdraws for this window
                let item = user_withdraws.0.get_mut(active_window).unwrap();
                item.coins.amount += amount;
                debug_print(format!(
                    "Adding amount to current withdraw: {}",
                    item.coins.amount,
                ));
                cashmap.insert(user_key, user_withdraws)?;
            } else {
                // user has withdraws, but no active one
                debug_print(format!("No active withdraw found for this window"));
                return self._append_new_withdraw_to_user(
                    amount,
                    &mut cashmap,
                    user_key,
                    user_withdraws,
                );
            }
        } else {
            // user has no withdraws
            let user_withdraws = UserWithdraws::default();
            return self._append_new_withdraw_to_user(
                amount,
                &mut cashmap,
                user_key,
                user_withdraws,
            );
        }

        Ok(())
    }

    fn _append_new_withdraw_to_user<S: Storage>(
        &self,
        amount: Uint128,
        cashmap: &mut CashMap<UserWithdraws, S>,
        user_key: &[u8],
        mut user_withdraws: UserWithdraws,
    ) -> StdResult<()> {
        let new_withdraw = WaitingWithdraw {
            id: self.current_active_window,
            coins: Coin {
                denom: NATIVE_TOKEN_DENOM.to_string(),
                amount,
            },
        };
        user_withdraws.0.push(new_withdraw);
        cashmap.insert(user_key, user_withdraws)
    }
}

pub fn get_window_manager<S: ReadonlyStorage>(store: &S) -> StdResult<WindowManager> {
    let config_store = ReadonlyPrefixedStorage::new(PREFIX_WINDOW_MANANGER, store);
    let x = config_store.get(&PREFIX_WINDOW_MANANGER).unwrap();
    let record: WindowManager = bincode2::deserialize(&x)
        .map_err(|_| StdError::generic_err("Error getting window manager"))?;
    Ok(record)
}

pub fn set_window_manager<S: Storage>(store: &mut S, manager: &WindowManager) -> StdResult<()> {
    let mut config_store = PrefixedStorage::new(PREFIX_WINDOW_MANANGER, store);
    let as_bytes = bincode2::serialize(manager)
        .map_err(|_| StdError::generic_err("Error setting window manager"))?;

    config_store.set(PREFIX_WINDOW_MANANGER, &as_bytes);

    Ok(())
}
