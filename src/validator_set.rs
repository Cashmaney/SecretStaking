use bincode2;
use serde::{Deserialize, Serialize};

use crate::state::{CONFIG_KEY, VALIDATOR_ADDRESS_KEY};
use cosmwasm_std::{StdError, ReadonlyStorage, StdResult, Storage};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};
use std::cmp::Ordering;
use std::collections::VecDeque;

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct Validator {
    address: String,
    staked: u64,
    //weight: u8
}

impl PartialOrd for Validator {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Validator {
    fn cmp(&self, other: &Self) -> Ordering {
        self.staked.cmp(&other.staked)
    }
}

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, Default)]
pub struct ValidatorSet {
    validators: VecDeque<Validator>,
}

impl ValidatorSet {
    pub fn remove(&mut self, address: &String) -> StdResult<()> {
        let val = self.validators.back().unwrap();

        if &val.address != address {
            return Err(StdError::generic_err(format!(
                "Failed to remove validator: {}, you need to remove {} first",
                address, val.address
            )));
        }

        if val.staked != 0 {
            return Err(StdError::generic_err(format!(
                "Failed to remove validator: {}, you need to undelegate {} first",
                address, val.staked
            )));
        }

        self.validators.pop_back();
        Ok(())
    }

    pub fn add(&mut self, address: String) {
        self.validators.push_back(Validator { address, staked: 0 })
    }

    pub fn unbond(&mut self, to_stake: u64) -> StdResult<String> {
        if self.validators.is_empty() {
            return Err(StdError::generic_err(
                "Failed to get validator to unbond - validator set is empty",
            ));
        }

        let val = self.validators.front_mut().unwrap();
        val.staked -= to_stake;
        Ok(val.address.clone())
    }

    pub fn stake(&mut self, to_stake: u64) -> StdResult<String> {
        if self.validators.is_empty() {
            return Err(StdError::generic_err(
                "Failed to get validator to stake - validator set is empty",
            ));
        }

        let val = self.validators.back_mut().unwrap();
        val.staked += to_stake;
        Ok(val.address.clone())
    }

    // call this after every stake or unbond call
    pub fn rebalance(&mut self) {
        if self.validators.len() < 2 {
            return;
        }

        self.validators.make_contiguous().sort_by(|a, b| a.cmp(b));
    }
}

/// todo: validator address is a String till we test with HumanAddr and see that secretval addresses are working
pub fn get_validator_set<S: Storage>(store: &S) -> StdResult<ValidatorSet> {
    let config_store = ReadonlyPrefixedStorage::new(CONFIG_KEY, store);
    let x = config_store.get(VALIDATOR_ADDRESS_KEY).unwrap();
    let record: ValidatorSet =
        bincode2::deserialize(&x).map_err(|_| StdError::generic_err("Error unpacking validator set"))?;
    Ok(record)
}

pub fn set_validator_set<S: Storage>(
    store: &mut S,
    validator_address: &ValidatorSet,
) -> StdResult<()> {
    let mut config_store = PrefixedStorage::new(CONFIG_KEY, store);
    let as_bytes = bincode2::serialize(validator_address)
        .map_err(|_| StdError::generic_err("Error packing validator set"))?;

    config_store.set(VALIDATOR_ADDRESS_KEY, &as_bytes);

    Ok(())
}
