use serde::{Deserialize, Serialize};

use cosmwasm_std::{HumanAddr, ReadonlyStorage, StdResult, Storage};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};

pub static PREFIX_CONFIG: &[u8] = b"config";
pub static CONFIG_KEY: &[u8] = b"config";

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq)]
pub struct Config {
    pub symbol: String,
    pub admin: HumanAddr,
    pub token_contract: HumanAddr,
    pub token_contract_hash: String,
    pub gov_token: HumanAddr,
    pub gov_token_hash: String,
    pub voting_admin: HumanAddr,
    pub unbonding_time: u64,
    pub viewing_key: String,
    pub kill_switch: u8,
    pub dev_address: HumanAddr,
    pub dev_fee: u64, // 10^-3 percent. 1 = 0.001%
    pub shared_withdrawals: u8,
}

pub fn set_config<S: Storage>(storage: &mut S, config: &Config) {
    let config_bytes: Vec<u8> = bincode2::serialize(&config).unwrap();

    let mut config_store = PrefixedStorage::new(PREFIX_CONFIG, storage);
    config_store.set(CONFIG_KEY, &config_bytes);
}

pub fn read_config<S: Storage>(store: &S) -> StdResult<Config> {
    let config_store = ReadonlyPrefixedStorage::new(PREFIX_CONFIG, store);
    let consts_bytes = config_store.get(CONFIG_KEY).unwrap();

    let consts: Config = bincode2::deserialize(&consts_bytes).unwrap();

    Ok(consts)
}
