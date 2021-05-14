use cosmwasm_std::{
    log, to_binary, Api, Binary, CosmosMsg, Env, Extern, HandleResponse, HumanAddr, InitResponse,
    MigrateResponse, Querier, StdError, StdResult, Storage, WasmMsg,
};
use secret_toolkit::snip20;

use cargo_common::tokens::{InitHook, TokenInitMsg};

use crate::admin::admin_commands;
use crate::claim::claim;
use crate::deposit::try_deposit;
use crate::msg::{HandleMsg, InitMsg, MigrateMsg, QueryMsg};
use crate::queries::{query_exchange_rate, query_interest_rate, query_pending_claims};
use crate::state::store_address;
use crate::types::config::{read_config, set_config, Config};
use crate::types::killswitch::KillSwitch;
use crate::types::shared_withdraw_config::SharedWithdrawConfig;
use crate::types::validator_set::{set_validator_set, ValidatorSet};
use crate::voting::try_vote;
use crate::withdraw::try_withdraw;

use crate::constants::UNBONDING_TIME;

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

    // save the current address (used in queries because we don't actually know the address)
    store_address(&mut deps.storage, &env.contract.address);

    let config = Config {
        admin: env.message.sender.clone(),
        token_contract: HumanAddr::default(),
        token_contract_hash: msg.token_code_hash.clone(),
        gov_token: HumanAddr::default(),
        gov_token_hash: msg.token_code_hash.clone(),
        voting_admin: env.message.sender,
        symbol: msg.symbol,
        unbonding_time: UNBONDING_TIME,
        viewing_key: "yo".to_string(),
        kill_switch: KillSwitch::Closed.into(),
        dev_fee: msg.dev_fee.unwrap_or(1000),
        dev_address: msg.dev_address.unwrap_or_else(|| {
            HumanAddr("secret1lfhy2amwlxlu4usd4put9jm77v86gkd057gkhr".to_string())
        }),
        shared_withdrawals: SharedWithdrawConfig::All.into(),
    };

    set_config(&mut deps.storage, &config);

    let mut valset = ValidatorSet::default();

    valset.add((&msg.validator).clone(), None);

    set_validator_set(&mut deps.storage, &valset)?;

    /* append set viewing key messages and store viewing keys */
    let mut messages = vec![];

    let init_token_msg = TokenInitMsg::new(
        "Staking Derivative Token".to_string(),
        env.contract.address.clone(),
        "CASH".to_string(),
        6,
        msg.prng_seed,
        InitHook {
            msg: to_binary(&HandleMsg::PostInitialize {})?,
            contract_addr: env.contract.address,
            code_hash: env.contract_code_hash,
        },
        Some(msg.token_code_id),
        None,
    );

    // validate that shit
    init_token_msg.validate()?;

    // Create Staking Derivative token
    messages.extend(vec![CosmosMsg::Wasm(WasmMsg::Instantiate {
        code_id: msg.token_code_id,
        msg: to_binary(&init_token_msg)?,
        send: vec![],
        label: msg.label.to_string(),
        callback_code_hash: msg.token_code_hash,
    })]);

    Ok(InitResponse {
        messages,
        log: vec![],
    })
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::Deposit {} => try_deposit(deps, env),
        HandleMsg::Receive {
            amount,
            sender,
            msg,
        } => try_withdraw(deps, env, amount, sender, msg),
        HandleMsg::Claim {} => claim(deps, env),
        HandleMsg::PostInitialize {} => post_initialize(deps, env),
        HandleMsg::VoteOnChain { proposal, vote } => try_vote(deps, env, proposal, vote),
        // HandleMsg::Vote {
        //
        // }
        _ => admin_commands(deps, env, msg),
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::ExchangeRate {} => query_exchange_rate(&deps.storage, &deps.querier),
        QueryMsg::InterestRate {} => query_interest_rate(&deps.querier),
        QueryMsg::PendingClaims {
            address,
            current_time,
        } => query_pending_claims(&deps.storage, address, current_time),
    }
}

pub fn post_initialize<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> StdResult<HandleResponse> {
    let mut config = read_config(&deps.storage)?;

    if config.token_contract != HumanAddr::default() {
        return Err(StdError::unauthorized());
    }

    config.token_contract = env.message.sender.clone();

    // easier to change this manually later probably?
    // config.gov_token = gov_token.unwrap_or_default();

    config.viewing_key = "yo".to_string();

    set_config(&mut deps.storage, &config);

    Ok(HandleResponse {
        messages: vec![
            snip20::register_receive_msg(
                env.contract_code_hash,
                None,
                256,
                config.token_contract_hash.clone(),
                env.message.sender.clone(),
            )?,
            snip20::set_viewing_key_msg(
                config.viewing_key,
                None,
                256,
                config.token_contract_hash,
                env.message.sender.clone(),
            )?,
        ],
        log: vec![log("dx_token_address", env.message.sender.as_str())],
        data: None,
    })
}

pub fn migrate<S: Storage, A: Api, Q: Querier>(
    _deps: &mut Extern<S, A, Q>,
    _env: Env,
    _msg: MigrateMsg,
) -> StdResult<MigrateResponse> {
    Ok(MigrateResponse::default())
}
