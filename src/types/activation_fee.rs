use serde::{Deserialize, Serialize};

use cosmwasm_std::{StdResult, Storage};
use cosmwasm_storage::{ReadonlySingleton, Singleton};

pub static KEY_ACTIVATION_FEE_CONFIG: &[u8] = b"activation_fee_config";
pub static KEY_ACTIVATION_FEE: &[u8] = b"activation_fee";

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq)]
pub struct ActivationFeeConfig {
    pub fee: u64,
    pub max: u64,
}

pub fn set_activation_fee_config<S: Storage>(
    storage: &mut S,
    data: &ActivationFeeConfig,
) -> StdResult<()> {
    Singleton::new(storage, KEY_ACTIVATION_FEE_CONFIG).save(data)
}
pub fn read_activation_fee_config<S: Storage>(storage: &S) -> StdResult<ActivationFeeConfig> {
    ReadonlySingleton::new(storage, KEY_ACTIVATION_FEE_CONFIG).load()
}

pub fn set_activation_fee<S: Storage>(storage: &mut S, data: &u64) -> StdResult<()> {
    Singleton::new(storage, KEY_ACTIVATION_FEE).save(data)
}
pub fn read_activation_fee<S: Storage>(storage: &S) -> StdResult<u64> {
    ReadonlySingleton::new(storage, KEY_ACTIVATION_FEE).load()
}
