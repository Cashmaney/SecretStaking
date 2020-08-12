use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::msg::{AllowanceResponse, BalanceResponse, HandleMsg, InitMsg, MigrateMsg, QueryMsg};
use crate::staking::{get_bonded, get_onchain_balance, stake, undelegate};
use crate::state::{
    add_balance, deposit, get_ratio, get_validator_address, read_balance, read_constants,
    remove_balance, update_stored_balance, withdraw, Constants, KEY_TOTAL_BALANCE,
};
use crate::transfer::{perform_transfer, store_transfer};
use crate::utils::callback_update_balances;
use cosmwasm_std::{
    generic_err, log, to_binary, to_vec, Api, BankMsg, Binary, CanonicalAddr, Coin, CosmosMsg,
    Decimal, Env, Extern, HandleResponse, HumanAddr, InitResponse, MigrateResponse, Querier,
    ReadonlyStorage, StakingMsg, StdResult, Storage, Uint128, WasmMsg,
};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};

pub const PREFIX_CONFIG: &[u8] = b"config";
pub const PREFIX_BALANCES: &[u8] = b"balances";
pub const PREFIX_ALLOWANCES: &[u8] = b"allowances";

pub const KEY_CONSTANTS: &[u8] = b"constants";
pub const KEY_TOTAL_SUPPLY: &[u8] = b"total_supply";

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    // ensure the validator is registered
    let vals = deps.querier.query_validators()?;
    if !vals.iter().any(|v| v.address == msg.validator) {
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

    let mut config_store = PrefixedStorage::new(PREFIX_CONFIG, &mut deps.storage);
    let constants = bincode2::serialize(&Constants {
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
    })
    .unwrap();
    config_store.set(KEY_CONSTANTS, &constants);
    config_store.set(KEY_TOTAL_SUPPLY, &total_token_supply.to_be_bytes());
    config_store.set(KEY_TOTAL_BALANCE, &total_scrt_balance.to_be_bytes());

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
        HandleMsg::Balance {} => try_balance(deps, env),
        HandleMsg::Transfer { recipient, amount } => try_transfer(deps, env, &recipient, &amount),
        HandleMsg::UpdateBalances {} => refresh_balances(deps, env),
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    _deps: &Extern<S, A, Q>,
    _msg: QueryMsg,
) -> StdResult<Binary> {
    Err(generic_err("Queries are not supported in this contract"))
}

fn try_transfer<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    recipient: &HumanAddr,
    amount: &Uint128,
) -> StdResult<HandleResponse> {
    let sender_address_raw = &env.message.sender;
    let recipient_address_raw = deps.api.canonical_address(recipient)?;
    let amount_raw = amount.u128();

    perform_transfer(
        &mut deps.storage,
        &sender_address_raw,
        &recipient_address_raw,
        amount_raw,
    )?;

    let symbol = read_constants(&deps.storage)?.symbol;

    store_transfer(
        &deps.api,
        &mut deps.storage,
        sender_address_raw,
        &recipient_address_raw,
        amount,
        symbol,
    );

    let res = HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "transfer"),
            log(
                "sender",
                deps.api.human_address(&env.message.sender)?.as_str(),
            ),
            log("recipient", recipient.as_str()),
        ],
        data: None,
    };
    Ok(res)
}

pub fn try_balance<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let sender_address_raw = &env.message.sender;
    let account_balance = read_balance(&deps.storage, sender_address_raw);

    let consts = read_constants(&deps.storage)?;

    if let Err(_e) = account_balance {
        Ok(HandleResponse {
            messages: vec![],
            log: vec![
                log("action", "balance"),
                log(
                    "account",
                    deps.api.human_address(&env.message.sender)?.as_str(),
                ),
                log("amount", "0"),
            ],
            data: None,
        })
    } else {
        let printable_token =
            to_display_token(account_balance.unwrap(), &consts.symbol, consts.decimals);

        Ok(HandleResponse {
            messages: vec![],
            log: vec![
                log("action", "balance"),
                log(
                    "account",
                    deps.api.human_address(&env.message.sender)?.as_str(),
                ),
                log("amount", printable_token),
            ],
            data: None,
        })
    }
}

fn refresh_balances<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    _env: Env,
) -> StdResult<HandleResponse> {
    let validator = deps
        .api
        .human_address(&get_validator_address(&deps.storage)?)?;

    let balance = get_bonded(&deps.querier, &validator)?;

    update_stored_balance(&mut deps.storage, balance.u128());

    let ratio = get_ratio(&deps.storage)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![log("ratio", format!("{:?}", ratio))],
        data: None,
    })
}

fn try_deposit<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let mut amount_raw: Uint128 = Uint128::default();

    let contract_addr = deps.api.human_address(&env.contract.address)?;
    let code_hash = env.contract_code_hash.unwrap();
    let validator = deps
        .api
        .human_address(&get_validator_address(&deps.storage)?)?;

    for coin in &env.message.sent_funds {
        if coin.denom == "uscrt" {
            amount_raw = coin.amount
        }
    }

    if amount_raw == Uint128::default() {
        return Err(generic_err(format!("Lol send some funds dude")));
    }

    let amount = amount_raw.u128();

    let sender_address_raw = &env.message.sender;

    let token_amount = deposit(&mut deps.storage, amount)?;

    add_balance(&mut deps.storage, sender_address_raw, token_amount);

    let res = HandleResponse {
        messages: vec![
            stake(&validator, amount),
            callback_update_balances(&contract_addr, &code_hash),
        ],
        log: vec![
            log("action", "deposit"),
            log(
                "account",
                deps.api.human_address(&env.message.sender)?.as_str(),
            ),
            log("amount", &token_amount.to_string()),
        ],
        data: None,
    };

    Ok(res)
}

fn try_withdraw<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    let owner_address_raw = &env.message.sender;
    let code_hash = env.contract_code_hash.unwrap();
    let validator = deps
        .api
        .human_address(&get_validator_address(&deps.storage)?)?;
    let contract_addr = deps.api.human_address(&env.contract.address)?;
    let withdrawal_address = deps.api.human_address(&env.message.sender)?;
    remove_balance(&mut deps.storage, owner_address_raw, amount.u128());

    let scrt_amount = withdraw(&mut deps.storage, amount.u128())?;

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
            callback_update_balances(&contract_addr, &code_hash),
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

fn to_display_token(amount: u128, symbol: &String, decimals: u8) -> String {
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
