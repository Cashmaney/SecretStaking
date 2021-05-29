use cosmwasm_std::{to_binary, Binary, HumanAddr, Querier, StdResult, Storage, Uint128};
use rust_decimal::prelude::{One, Zero};
use rust_decimal::Decimal;

use crate::msg::{PendingClaimsResponse, QueryResponse};
use crate::staking::{exchange_rate, get_total_onchain_balance, interest_rate};
use crate::state::get_address;
use crate::types::config::read_config;
use crate::types::pending_withdraws::PendingWithdraws;
use crate::types::validator_set::get_validator_set;

// todo: implement interest rate query
pub fn query_interest_rate<Q: Querier>(querier: &Q) -> StdResult<Binary> {
    let rate = interest_rate(querier)?;

    to_binary(&QueryResponse::InterestRate {
        rate: Uint128(rate),
        denom: "uscrt".to_string(),
    })
}

pub fn query_info<S: Storage, Q: Querier>(store: &S, querier: &Q) -> StdResult<Binary> {
    let config = read_config(store)?;
    let validator_set = get_validator_set(store)?;
    let contract_address = get_address(store)?;
    let total_on_chain = get_total_onchain_balance(querier, store, &contract_address)?;

    to_binary(&QueryResponse::Info {
        token_address: config.token_contract,
        validators: validator_set.to_query_response(),
        admin: config.admin,
        total_staked: Uint128(total_on_chain),
        voting_admin: Some(config.voting_admin),
    })
}

pub fn query_dev_fee<S: Storage>(store: &S) -> StdResult<Binary> {
    let config = read_config(store)?;

    to_binary(&QueryResponse::DevFee {
        fee: config.dev_fee,
        address: config.dev_address,
    })
}

pub fn query_exchange_rate<S: Storage, Q: Querier>(store: &S, querier: &Q) -> StdResult<Binary> {
    let ratio = exchange_rate(store, querier)?;

    let rate = if ratio.is_zero() {
        "1".to_string()
    } else {
        (Decimal::one() / (ratio)).to_string()
    };

    to_binary(&QueryResponse::ExchangeRate {
        rate,
        denom: "uscrt".to_string(),
    })
}

pub fn query_pending_claims<S: Storage>(
    store: &S,
    address: HumanAddr,
    current_time: Option<u64>,
) -> StdResult<Binary> {
    let pending_withdraws = PendingWithdraws::load(store, &address);

    let withdraws = pending_withdraws.pending();

    let mut responses: Vec<PendingClaimsResponse> = vec![];

    for w in withdraws {
        let mut matured: Option<bool> = None;
        if let Some(time) = current_time {
            matured = Some(time > w.available_time)
        };

        let response = PendingClaimsResponse {
            withdraw: w,
            matured,
        };

        responses.push(response)
    }

    to_binary(&QueryResponse::PendingClaims { pending: responses })
}
