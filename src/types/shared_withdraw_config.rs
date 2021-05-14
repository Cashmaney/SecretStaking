use serde::{Deserialize, Serialize};

use cosmwasm_std::StdError;
use std::convert::TryFrom;

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq)]
pub enum SharedWithdrawConfig {
    None,
    Withdraws,
    Deposits,
    All,
}

impl TryFrom<u8> for SharedWithdrawConfig {
    type Error = StdError;

    fn try_from(other: u8) -> Result<Self, Self::Error> {
        match other {
            0 => Ok(Self::None),
            1 => Ok(Self::Withdraws),
            2 => Ok(Self::Deposits),
            3 => Ok(Self::All),
            _ => Err(StdError::generic_err(
                "Failed to convert SharedWithdrawConfig enum",
            )),
        }
    }
}

impl Into<u8> for SharedWithdrawConfig {
    fn into(self) -> u8 {
        match self {
            Self::None => 0u8,
            Self::Withdraws => 1u8,
            Self::Deposits => 2u8,
            Self::All => 3u8,
        }
    }
}
