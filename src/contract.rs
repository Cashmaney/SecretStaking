use cosmwasm_std::{
    generic_err, log, Api, BankMsg, Binary, Coin, CosmosMsg, Decimal, Env, Extern, HandleResponse,
    HumanAddr, InitResponse, MigrateResponse, Querier, StdResult, Storage, Uint128,
};
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
    let admin = deps.api.human_address(&env.message.sender)?;

    if !vals.iter().any(|v| v.address == human_addr_wrap) {
        return Err(generic_err(format!(
            "{} is not in the current validator set",
            msg.validator
        )));
    }

    let total_token_supply: u128 = 0;
    let total_scrt_balance: u128 = 0;
    // Check name, symbol, decimals
    if !is_valid_name(&msg.name) {
        return Err(generic_err(
            "Name is not in the expected format (3-30 UTF-8 bytes)",
        ));
    }
    if !is_valid_symbol(&msg.symbol) {
        return Err(generic_err(
            "Ticker symbol is not in expected format [A-Z]{3,6}",
        ));
    }
    if msg.decimals > 18 {
        return Err(generic_err("Decimals must not exceed 18"));
    }
    set_fee(&mut deps.storage, msg.fee_pips)?;
    set_liquidity_ratio(&mut deps.storage, u128::from(msg.target_staking_ratio))?;
    update_cached_liquidity_balance(&mut deps.storage, total_scrt_balance);
    let mut config_store = PrefixedStorage::new(PREFIX_CONFIG, &mut deps.storage);
    let constants = bincode2::serialize(&Constants {
        admin,
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
    })
    .unwrap();

    config_store.set(KEY_CONSTANTS, &constants);
    config_store.set(KEY_TOTAL_TOKENS, &total_token_supply.to_be_bytes());
    config_store.set(KEY_TOTAL_BALANCE, &total_scrt_balance.to_be_bytes());
    set_validator_address(&mut deps.storage, &msg.validator)?;

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

fn try_withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    let owner_address_raw = &env.message.sender;
    let code_hash = env.contract_code_hash;
    let validator = get_validator_address(&deps.storage)?;
    let contract_addr = deps.api.human_address(&env.contract.address)?;
    let withdrawal_address = deps.api.human_address(&env.message.sender)?;
    let current_liquidity = liquidity_pool_balance(&deps.storage);
    let rate = get_exchange_rate(&deps.storage)?;

    if amount.u128() < EXCHANGE_RATE_RESOLUTION as u128 {
        return Err(generic_err("Can only withdraw a minimum of 1000 uscrt"));
    }

    // todo: set this limit in some other way
    if current_liquidity < rate * (amount.u128() / (EXCHANGE_RATE_RESOLUTION as u128)) {
        return Err(generic_err(format!(
            "Cannot withdraw this amount at this time. You can only withdraw a limit of {:?} uscrt",
            current_liquidity
        )));
    }

    remove_balance(&mut deps.storage, owner_address_raw, amount.u128())?;

    let exch_rate = get_exchange_rate(&deps.storage)?;
    let fee = get_fee(&deps.storage)?;
    let scrt_amount = withdraw(&mut deps.storage, amount.u128(), exch_rate, fee)?;

    let scrt = Coin {
        denom: "uscrt".to_string(),
        amount,
    };

    let res = HandleResponse {
        messages: vec![
            CosmosMsg::Bank(BankMsg::Send {
                from_address: contract_addr.clone(),
                to_address: withdrawal_address,
                amount: vec![scrt.clone()],
            }),
            undelegate(&validator, scrt_amount),
            update_balances_message(&contract_addr, &code_hash),
        ],
        log: vec![
            log("action", "withdraw"),
            log(
                "account",
                deps.api.human_address(&env.message.sender)?.as_str(),
            ),
            log("amount", format!("{:?}", scrt)),
        ],
        data: None,
    };

    Ok(res)
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

pub(crate) fn to_display_token(amount: u128, symbol: &String, decimals: u8) -> String {
    let base: u32 = 10;

    let amnt: Decimal = Decimal::from_ratio(amount, (base.pow(decimals.into())) as u64);

    format!("{} {}", amnt, symbol)
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: MigrateMsg,
) -> StdResult<MigrateResponse> {
    Ok(MigrateResponse::default())
}
