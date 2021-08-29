use cosmwasm_std::{to_binary, Binary, HumanAddr, Querier, StdResult, Storage, Uint128};
use rust_decimal::prelude::{One, Zero};
use rust_decimal::Decimal;

use crate::msg::{PendingClaimsResponse, QueryResponse};
use crate::staking::{exchange_rate, get_total_onchain_balance};
use crate::state::get_address;
use crate::types::activation_fee::read_activation_fee;
use crate::types::config::read_config;
use crate::types::pending_withdraw::PendingWithdraw;
use crate::types::user_withdraws::all_waiting_withdraws_for_user;
use crate::types::validator_set::get_validator_set;
use crate::types::window_manager::get_window_manager;
use crate::types::withdraw_window::get_claim_time;

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
    let pending_withdraws = all_waiting_withdraws_for_user(store, &address);
    let window_manager = get_window_manager(store)?;

    let mut responses: Vec<PendingClaimsResponse> = vec![];

    for w in pending_withdraws.0 {
        let mut withdraw = PendingWithdraw {
            available_time: 0,
            receiver: address.clone(),
            coins: w.coins,
        };
        let in_current_window: bool = w.id == window_manager.current_active_window;
        let mut matured: Option<bool> = None;

        if !in_current_window {
            let claimable_time = get_claim_time(store, w.id).unwrap_or_default();
            if let Some(time) = current_time {
                matured = Some(time > claimable_time && claimable_time != 0)
            };

            withdraw.available_time = claimable_time;
        }

        let response = PendingClaimsResponse {
            withdraw,
            ready_for_claim: matured,
            in_current_window,
        };

        responses.push(response)
    }

    to_binary(&QueryResponse::PendingClaims { pending: responses })
}

pub fn query_activation_fee<S: Storage>(store: &S, time: u64) -> StdResult<Binary> {
    let fee = read_activation_fee(store)?;

    let window_manager = get_window_manager(store)?;
    let is_available = window_manager.time_to_close_window < time;
    to_binary(&QueryResponse::ActivationFee { fee, is_available })
}

pub fn query_current_window<S: Storage>(store: &S) -> StdResult<Binary> {
    let manager = get_window_manager(store)?;

    to_binary(&QueryResponse::Window {
        id: manager.current_active_window,
        time_to_close: manager.time_to_close_window,
    })
}
