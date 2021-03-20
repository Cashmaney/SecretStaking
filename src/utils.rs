use cosmwasm_std::{StdError, StdResult};
use std::str::FromStr;

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
pub fn dec_to_uint(dec: String) -> StdResult<u128> {
    let tokens: Vec<&str> = dec.split('.').collect();

    if tokens.len() < 2 {
        return u128::from_str(&dec).map_err(|_| StdError::generic_err("failed to parse number"));
    }

    u128::from_str(&dec).map_err(|_| StdError::generic_err("failed to parse number"))
}
