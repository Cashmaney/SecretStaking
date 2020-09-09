use cosmwasm_std::{log, Api, BankMsg, Binary, Coin, CosmosMsg, Env, Extern, HandleResponse, HumanAddr, InitResponse, MigrateResponse, Querier, StdResult, Storage, Uint128, StdError};
use cosmwasm_storage::PrefixedStorage;

use crate::admin::admin_commands;
use crate::deposit::try_deposit;
use crate::liquidity_pool::update_balances_message;
use crate::msg::{HandleMsg, InitMsg, MigrateMsg, QueryMsg};
use crate::queries::{query_exchange_rate, query_interest_rate};
use crate::staking::{stake, undelegate};
use crate::state::{
    add_token_balance, deposit, get_exchange_rate, get_fee, get_validator_address,
    liquidity_pool_balance, remove_balance, set_fee, set_liquidity_ratio, set_validator_address,
    update_cached_liquidity_balance, withdraw, Constants, EXCHANGE_RATE_RESOLUTION,
    KEY_TOTAL_BALANCE, KEY_TOTAL_TOKENS,
};
use crate::validator_set::{set_validator_set, ValidatorSet};
use crate::withdraw::try_withdraw;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::ops::Mul;

pub const PREFIX_CONFIG: &[u8] = b"config";
pub const PREFIX_BALANCES: &[u8] = b"balances";
pub const PREFIX_ALLOWANCES: &[u8] = b"allowances";

pub const KEY_CONSTANTS: &[u8] = b"constants";

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    // ensure the validator is registered
    let vals = deps.querier.query_validators()?;
    let human_addr_wrap = HumanAddr(msg.validator.clone());

    if !vals.iter().any(|v| v.address == human_addr_wrap) {
        return Err(StdError::generic_err(format!(
            "{} is not in the current validator set",
            msg.validator
        )));
    }

    let total_token_supply: u128 = 0;
    let total_scrt_balance: u128 = 0;
    // Check name, symbol, decimals
    if !is_valid_name(&msg.name) {
        return Err(StdError::generic_err(
            "Name is not in the expected format (3-30 UTF-8 bytes)",
        ));
    }
    if !is_valid_symbol(&msg.symbol) {
        return Err(StdError::generic_err(
            "Ticker symbol is not in expected format [A-Z]{3,6}",
        ));
    }
    if msg.decimals > 18 {
        return Err(StdError::generic_err("Decimals must not exceed 18"));
    }
    set_fee(&mut deps.storage, msg.fee_pips)?;
    set_liquidity_ratio(&mut deps.storage, u128::from(msg.target_staking_ratio))?;
    update_cached_liquidity_balance(&mut deps.storage, total_scrt_balance);
    let mut config_store = PrefixedStorage::new(PREFIX_CONFIG, &mut deps.storage);
    let constants = bincode2::serialize(&Constants {
        admin: env.message.sender,
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
    })
    .unwrap();

    config_store.set(KEY_CONSTANTS, &constants);
    config_store.set(KEY_TOTAL_TOKENS, &total_token_supply.to_be_bytes());
    config_store.set(KEY_TOTAL_BALANCE, &total_scrt_balance.to_be_bytes());

    let mut valset = ValidatorSet::default();

    valset.add((&msg.validator).clone());

    set_validator_set(&mut deps.storage, &valset)?;

    //set_validator_address(&mut deps.storage, &msg.validator)?;

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::Withdraw { amount } => try_withdraw(deps, env, amount),
        HandleMsg::Deposit {} => try_deposit(deps, env),
        HandleMsg::Balance {} => crate::tokens::try_balance(deps, env),
        HandleMsg::Transfer { recipient, amount } => {
            crate::transfer::try_transfer(deps, env, &recipient, &amount)
        }
        _ => admin_commands(deps, env, msg),
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::ExchangeRate {} => query_exchange_rate(&deps.storage),
        QueryMsg::InterestRate {} => query_interest_rate(&deps.storage),
    }
}

fn is_valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.len() < 3 || bytes.len() > 30 {
        return false;
    }
    true
}

fn is_valid_symbol(symbol: &str) -> bool {
    let bytes = symbol.as_bytes();
    if bytes.len() < 3 || bytes.len() > 6 {
        return false;
    }
    for byte in bytes.iter() {
        if *byte < 65 || *byte > 90 {
            return false;
        }
    }
    true
}

// pub(crate) fn to_display_token(amount: u128, symbol: &String, decimals: u8) -> String {
//     let base: u32 = 10;
//
//     let amnt: Decimal = Decimal::from_ratio(amount, (base.pow(decimals.into())) as u64);
//
//     format!("{} {}", amnt, symbol)
// }

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: MigrateMsg,
) -> StdResult<MigrateResponse> {
    Ok(MigrateResponse::default())
}
