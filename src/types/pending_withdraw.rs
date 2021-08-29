use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Coin, HumanAddr};

//pub const PENDING_WITHDRAW: &[u8] = b"PENDING_WITHDRAW";

#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct PendingWithdraw {
    pub available_time: u64,
    pub receiver: HumanAddr,
    pub coins: Coin,
}

// #[derive(Serialize, Deserialize, Clone, Debug, Default)]
// pub struct PendingWithdraws(Vec<PendingWithdraw>);
//
// impl PendingWithdraws {
//     pub(crate) fn append(&mut self, withdraw: PendingWithdraw) {
//         self.0.push(withdraw)
//     }
//
//     /// same as get_expired, but also modifies itself - this is used in handles
//     pub(crate) fn remove_expired(&mut self, current_time: u64) -> Vec<PendingWithdraw> {
//         self.0
//             .drain_filter(|item| item.available_time <= current_time)
//             .collect::<Vec<_>>()
//     }
//
//     pub fn len(&self) -> usize {
//         self.0.len()
//     }
//
//     /// get all expired (matured) withdraws. Can be used in queries since it does not modify the inner
//     /// structure
//     // pub fn get_expired(&self, current_time: u64) -> Vec<PendingWithdraw> {
//     //     self.0
//     //         .clone()
//     //         .drain_filter(|item| item.available_time <= current_time)
//     //         .collect::<Vec<_>>()
//     // }
//
//     pub(crate) fn pending(&self) -> Vec<PendingWithdraw> {
//         let pending: Vec<PendingWithdraw> = self.0.clone();
//
//         pending
//     }
//
//     pub(crate) fn save<S: Storage>(self, storage: &mut S, address: &HumanAddr) -> StdResult<()> {
//         let mut cashmap = CashMap::init(&PENDING_WITHDRAW, storage);
//
//         if self.0.is_empty() {
//             cashmap.remove(&address.0.as_bytes())
//         } else {
//             cashmap.insert(&address.0.as_bytes(), self)
//         }
//     }
//
//     pub(crate) fn load<S: Storage>(storage: &S, address: &HumanAddr) -> Self {
//         let cashmap = ReadOnlyCashMap::init(&PENDING_WITHDRAW, storage);
//
//         let withdraws = cashmap.get(&address.0.as_bytes());
//
//         withdraws.unwrap_or_default()
//     }
//
//     pub(crate) fn get_multiple<S: Storage>(storage: &mut S, amount: u32) -> StdResult<Vec<Self>> {
//         let cashmap = CashMap::<PendingWithdraws, _>::init(&PENDING_WITHDRAW, storage);
//
//         let values = cashmap.paging(0, amount)?;
//
//         Ok(values)
//     }
//
//     pub(crate) fn append_withdraw<S: Storage>(
//         storage: &mut S,
//         withdraw: &PendingWithdraw,
//         address: &HumanAddr,
//     ) -> StdResult<()> {
//         let mut cashmap: CashMap<PendingWithdraws, S> = CashMap::init(&PENDING_WITHDRAW, storage);
//
//         if let Some(mut withdraws) = cashmap.get(&address.0.as_bytes()) {
//             if withdraws.len() >= MAX_WITHDRAW_AMOUNT as usize {
//                 return Err(StdError::generic_err(format!(
//                     "Cannot have more than {} pending withdraws",
//                     MAX_WITHDRAW_AMOUNT
//                 )));
//             }
//
//             withdraws.append(withdraw.clone());
//             cashmap.insert(&address.0.as_bytes(), withdraws)?;
//         } else {
//             let mut new_withdraws = PendingWithdraws::default();
//             new_withdraws.append(withdraw.clone());
//             cashmap.insert(&address.0.as_bytes(), new_withdraws)?;
//         }
//
//         Ok(())
//     }
// }
