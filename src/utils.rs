use std::convert::TryFrom;
use std::str::FromStr;

use cosmwasm_std::{Api, CosmosMsg, Env, Extern, HumanAddr, Querier, StdError, StdResult, Storage};

use crate::claim::claim_multiple;
use crate::constants::AMOUNT_OF_SHARED_WITHDRAWS;
use crate::types::config::Config;
use crate::types::shared_withdraw_config::SharedWithdrawConfig;

// Converts 16 bytes value into u128
// Errors if data found that is not 16 bytes
// pub fn bytes_to_u128(data: &[u8]) -> StdResult<u128> {
//     match data[0..16].try_into() {
//         Ok(bytes) => Ok(u128::from_be_bytes(bytes)),
//         Err(_) => Err(StdError::generic_err(
//             "Corrupted data found. 16 byte expected.",
//         )),
//     }
// }

// Converts 4 bytes value into u32
// Errors if data found that is not 4 bytes
// pub fn bytes_to_u32(data: &[u8]) -> StdResult<u32> {
//     match data[0..4].try_into() {
//         Ok(bytes) => Ok(u32::from_be_bytes(bytes)),
//         Err(_) => Err(StdError::generic_err(
//             "Corrupted data found. 4 byte expected.",
//         )),
//     }
// }

/// Inflation rate, and other fun things are in the form 0.xxxxx. To use we remove the leading '0.'
/// and cut all but the the first 4 digits
#[allow(dead_code)]
pub fn dec_to_uint(dec: String) -> StdResult<u128> {
    let tokens: Vec<&str> = dec.split('.').collect();

    if tokens.len() < 2 {
        return u128::from_str(&dec).map_err(|_| StdError::generic_err("failed to parse number"));
    }

    u128::from_str(&dec).map_err(|_| StdError::generic_err("failed to parse number"))
}

pub fn address_to_bytes(address: &HumanAddr) -> &[u8] {
    &address.0.as_bytes()
}

pub fn u64_to_bytes(number: &u64) -> [u8; 8] {
    number.to_be_bytes()
}

pub fn perform_helper_claims<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    constants: &Config,
    messages: &mut Vec<CosmosMsg>,
) -> StdResult<()> {
    let withdraw_config = SharedWithdrawConfig::try_from(constants.shared_withdrawals)?;
    if withdraw_config == SharedWithdrawConfig::Withdraws
        || withdraw_config == SharedWithdrawConfig::All
    {
        messages.extend(claim_multiple(deps, &env, AMOUNT_OF_SHARED_WITHDRAWS)?.messages);
    }

    Ok(())
}
