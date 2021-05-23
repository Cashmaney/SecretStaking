use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::staking::{undelegate_msg, withdraw_to_self};
use crate::types::config::PREFIX_CONFIG;
use cosmwasm_std::{CosmosMsg, ReadonlyStorage, StdError, StdResult, Storage, Uint128};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};
use std::cmp::Ordering;
use std::collections::VecDeque;

pub const DEFAULT_WEIGHT: u8 = 10;

pub const KEY_VALIDATOR_SET: &[u8] = b"KEY_VALIDATOR_SET";

#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct ValidatorResponse {
    pub(crate) address: String,
    pub(crate) staked: Uint128,
    pub(crate) weight: u8,
    //weight: u8
}

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct Validator {
    pub(crate) address: String,
    pub(crate) staked: u128,
    pub(crate) weight: u8,
    //weight: u8
}

impl PartialOrd for Validator {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Validator {
    fn cmp(&self, other: &Self) -> Ordering {
        //
        (self.staked.saturating_mul(other.weight as u128))
            .cmp(&(other.staked.saturating_mul(self.weight as u128)))
    }
}

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, Default, JsonSchema)]
pub struct ValidatorSet {
    validators: VecDeque<Validator>,
}

impl ValidatorSet {
    pub fn to_query_response(&self) -> Vec<ValidatorResponse> {
        self.validators
            .clone()
            .into_iter()
            .map(|v| ValidatorResponse {
                address: v.address,
                staked: Uint128(v.staked),
                weight: v.weight,
            })
            .collect()
    }

    pub fn next_to_unbond(&self) -> Option<&Validator> {
        if self.validators.is_empty() {
            return None;
        }
        self.validators.front()
    }

    pub fn remove(&mut self, address: &str, force: bool) -> StdResult<Option<Validator>> {
        let pos = self.exists(address);
        if pos.is_none() {
            return Err(StdError::generic_err(format!(
                "Failed to remove validator: {}, doesn't exist",
                address
            )));
        }

        let val = self.validators.get(pos.unwrap()).ok_or_else(|| {
            StdError::generic_err(format!(
                "Failed to remove validator: {}, failed to get from validator list",
                address
            ))
        })?;

        if !force && val.staked != 0 {
            return Err(StdError::generic_err(format!(
                "Failed to remove validator: {}, you need to undelegate {}uscrt first or set the flag force=true",
                address, val.staked
            )));
        }

        Ok(self.validators.remove(pos.unwrap()))
    }

    pub fn add(&mut self, address: String, weight: Option<u8>) {
        if self.exists(&address).is_none() {
            self.validators.push_back(Validator {
                address,
                staked: 0,
                weight: weight.unwrap_or(DEFAULT_WEIGHT),
            })
        }
    }

    pub fn change_weight(&mut self, address: &str, weight: Option<u8>) -> StdResult<()> {
        let pos = self.exists(address);
        if pos.is_none() {
            return Err(StdError::generic_err(format!(
                "Failed to remove validator: {}, doesn't exist",
                address
            )));
        }

        let val = self.validators.get_mut(pos.unwrap()).ok_or_else(|| {
            StdError::generic_err(format!(
                "Failed to remove validator: {}, failed to get from validator list",
                address
            ))
        })?;

        val.weight = weight.unwrap_or(DEFAULT_WEIGHT);

        Ok(())
    }

    pub fn unbond(&mut self, to_unbond: u128) -> StdResult<String> {
        if self.validators.is_empty() {
            return Err(StdError::generic_err(
                "Failed to get validator to unbond - validator set is empty",
            ));
        }

        let val = self.validators.front_mut().unwrap();
        val.staked = val.staked.saturating_sub(to_unbond);
        Ok(val.address.clone())
    }

    pub fn stake(&mut self, to_stake: u128) -> StdResult<String> {
        if self.validators.is_empty() {
            return Err(StdError::generic_err(
                "Failed to get validator to stake - validator set is empty",
            ));
        }

        let val = self.validators.back_mut().unwrap();
        val.staked += to_stake;
        Ok(val.address.clone())
    }

    pub fn stake_at(&mut self, address: &str, to_stake: u128) -> StdResult<()> {
        if self.validators.is_empty() {
            return Err(StdError::generic_err(
                "Failed to get validator to stake - validator set is empty",
            ));
        }

        for val in self.validators.iter_mut() {
            if val.address == address {
                val.staked += to_stake;
                return Ok(());
            }
        }

        Err(StdError::generic_err(
            "Failed to get validator to stake - validator not found",
        ))
    }

    pub fn exists(&self, address: &str) -> Option<usize> {
        self.validators.iter().position(|v| v.address == address)
    }

    // call this after every stake or unbond call
    pub fn rebalance(&mut self) {
        if self.validators.len() < 2 {
            return;
        }

        self.validators.make_contiguous().sort_by(|a, b| b.cmp(a));
    }

    pub fn withdraw_rewards_messages(&self, addresses: Option<Vec<String>>) -> Vec<CosmosMsg> {
        if let Some(validators) = addresses {
            self.validators
                .iter()
                .filter(|&val| validators.contains(&val.address) && val.staked > 0)
                .map(|val| withdraw_to_self(&val.address))
                .collect()
        } else {
            self.validators
                .iter()
                .filter(|&val| val.staked > 0)
                .map(|val| withdraw_to_self(&val.address))
                .collect()
        }
    }

    pub fn unbond_all(&self) -> Vec<CosmosMsg> {
        self.validators
            .iter()
            .filter(|&val| val.staked > 0)
            .map(|val| undelegate_msg(&val.address, val.staked))
            .collect()
    }

    pub fn zero(&mut self) {
        if self.validators.is_empty() {
            return;
        }

        for val in self.validators.iter_mut() {
            val.staked = 0;
        }
    }
}

/// todo: validator address is a String till we test with HumanAddr and see that secretval addresses are working
pub fn get_validator_set<S: Storage>(store: &S) -> StdResult<ValidatorSet> {
    let config_store = ReadonlyPrefixedStorage::new(PREFIX_CONFIG, store);
    let x = config_store.get(KEY_VALIDATOR_SET).unwrap();
    let record: ValidatorSet = bincode2::deserialize(&x)
        .map_err(|_| StdError::generic_err("Error unpacking validator set"))?;
    Ok(record)
}

pub fn set_validator_set<S: Storage>(
    store: &mut S,
    validator_address: &ValidatorSet,
) -> StdResult<()> {
    let mut config_store = PrefixedStorage::new(PREFIX_CONFIG, store);
    let as_bytes = bincode2::serialize(validator_address)
        .map_err(|_| StdError::generic_err("Error packing validator set"))?;

    config_store.set(KEY_VALIDATOR_SET, &as_bytes);

    Ok(())
}
