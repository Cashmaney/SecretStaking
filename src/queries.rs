use crate::state::get_exchange_rate;
use cosmwasm_std::{Binary, StdResult, Storage};

// todo: implement interest rate query
pub fn query_interest_rate<S: Storage>(_store: &S) -> StdResult<Binary> {
    Ok(Binary::default())
}

pub fn query_exchange_rate<S: Storage>(store: &S) -> StdResult<Binary> {
    let ratio = get_exchange_rate(store)?;

    let result = format!(
        "The current exchange rate is {:?} uscrt = 1000 tokens",
        ratio
    );

    Ok(Binary(result.as_bytes().to_vec()))
}
