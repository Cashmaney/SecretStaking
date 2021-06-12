/// This contract implements SNIP-20 standard:
/// https://github.com/SecretFoundation/SNIPs/blob/master/SNIP-20.md
use cosmwasm_std::{
    log, to_binary, Api, Binary, CanonicalAddr, CosmosMsg, Env, Extern, HandleResponse,
    HandleResult, HumanAddr, InitResponse, Querier, QueryRequest, QueryResult, ReadonlyStorage,
    StdError, StdResult, Storage, Uint128, VoteOption, WasmMsg, WasmQuery,
};

use crate::msg::{
    space_pad, ContractStatusLevel, HandleAnswer, HandleMsg, InitMsg, QueryAnswer, QueryMsg,
    ResponseStatus::Success,
};
use crate::rand::sha_256;
use crate::receiver::Snip20ReceiveMsg;
use crate::state::{
    get_receiver_hash, get_transfers, read_allowance, read_viewing_key, set_receiver_hash,
    store_transfer, write_allowance, write_viewing_key, Balances, Config, Constants,
    ReadonlyBalances, ReadonlyConfig,
};
use crate::viewing_key::{ViewingKey, VIEWING_KEY_SIZE};

use cargo_common::contract::Contract;
use cargo_common::tokens::{InitHook, TokenHandleMessage, TokenInitMsg};
use cargo_common::voting::{
    vote_option_to_u32, SingleVote, VoteChange, VoteResponse, VotingMessages,
};
use secret_toolkit::snip20;

/// We make sure that responses from `handle` are padded to a multiple of this size.
pub const RESPONSE_BLOCK_SIZE: usize = 256;
pub const GCASH_TOKEN_SYMBOL: &str = "CASH";

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    let init_config = msg.config();
    let mut total_supply: u128 = 0;
    {
        let mut balances = Balances::from_storage(&mut deps.storage);
        let initial_balances = msg.initial_balances.unwrap_or_default();
        for balance in initial_balances {
            let balance_address = deps.api.canonical_address(&balance.address)?;
            let amount = balance.amount.u128();
            balances.set_account_balance(&balance_address, amount);
            if let Some(new_total_supply) = total_supply.checked_add(amount) {
                total_supply = new_total_supply;
            } else {
                return Err(StdError::generic_err(
                    "The sum of all initial balances exceeds the maximum possible total supply",
                ));
            }
        }
    }

    let mut messages: Vec<CosmosMsg> = vec![];

    let mut admins = if let Some(ref admins) = msg.admin {
        admins.clone()
    } else {
        vec![]
    };

    admins.push(env.contract.address.clone());

    // init gov token
    if let Some(token_code_id) = msg.token_code_id {
        messages.push(CosmosMsg::Wasm(WasmMsg::Instantiate {
            code_id: token_code_id,
            msg: to_binary(&TokenInitMsg::new(
                "CASH Governance".to_string(),
                admins,
                GCASH_TOKEN_SYMBOL.to_string(),
                6,
                msg.prng_seed.clone(),
                InitHook {
                    msg: to_binary(&HandleMsg::PostInitialize {})?,
                    contract_addr: env.contract.address.clone(),
                    code_hash: env.contract_code_hash.clone(),
                },
                None,
                Some(true),
            ))?,
            send: vec![],
            label: format!("{}-gov", env.contract.address),
            callback_code_hash: env.contract_code_hash.clone(),
        }));
    }

    // Check name, symbol, decimals
    if !is_valid_name(&msg.name) {
        return Err(StdError::generic_err(
            "Name is not in the expected format (3-30 UTF-8 bytes)",
        ));
    }
    // if !is_valid_symbol(&msg.symbol) {
    //     return Err(StdError::generic_err(
    //         "Ticker symbol is not in expected format [A-Z]{3,6}",
    //     ));
    // }
    if msg.decimals > 18 {
        return Err(StdError::generic_err("Decimals must not exceed 18"));
    }

    let admin = msg
        .admin
        .unwrap_or_else(|| vec![env.message.sender.clone()]);

    let prng_seed_hashed = sha_256(&msg.prng_seed.0);

    let mut config = Config::from_storage(&mut deps.storage);
    config.set_constants(&Constants {
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
        admin: admin.clone(),
        prng_seed: prng_seed_hashed.to_vec(),
        total_supply_is_public: init_config.public_total_supply(),
        creator: env.message.sender,
    })?;
    config.set_minters(admin)?;
    config.set_total_supply(total_supply);
    config.set_contract_status(ContractStatusLevel::NormalRun);
    if let Some(is_being_minted) = msg.is_being_minted {
        config.set_is_being_minted(is_being_minted)?;
    }
    config.set_is_voting(false)?;

    if let Some(hook) = msg.init_hook {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: hook.contract_addr,
            callback_code_hash: hook.code_hash,
            msg: hook.msg,
            send: vec![],
        }));
    };

    Ok(InitResponse {
        messages,
        log: vec![],
    })
}

fn pad_response(response: StdResult<HandleResponse>) -> StdResult<HandleResponse> {
    response.map(|mut response| {
        response.data = response.data.map(|mut data| {
            space_pad(RESPONSE_BLOCK_SIZE, &mut data.0);
            data
        });
        response
    })
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    let contract_status = ReadonlyConfig::from_storage(&deps.storage).contract_status();

    match contract_status {
        ContractStatusLevel::StopAll | ContractStatusLevel::StopAllButRedeems => {
            let response = match msg {
                HandleMsg::SetContractStatus { level, .. } => set_contract_status(deps, env, level),
                _ => Err(StdError::generic_err(
                    "This contract is stopped and this action is not allowed",
                )),
            };
            return pad_response(response);
        }
        ContractStatusLevel::NormalRun => {} // If it's a normal run just continue
    }

    let response = match msg {
        // Voting
        HandleMsg::Vote { proposal, vote } => try_vote(deps, env, proposal, vote),
        // Base
        HandleMsg::Transfer {
            recipient, amount, ..
        } => try_transfer(deps, env, &recipient, amount),
        HandleMsg::Send {
            recipient,
            amount,
            msg,
            ..
        } => try_send(deps, env, &recipient, amount, msg),
        HandleMsg::RegisterReceive { code_hash, .. } => try_register_receive(deps, env, code_hash),
        HandleMsg::CreateViewingKey { entropy, .. } => try_create_key(deps, env, entropy),
        HandleMsg::SetViewingKey { key, .. } => try_set_key(deps, env, key),

        // Allowance
        HandleMsg::IncreaseAllowance {
            spender,
            amount,
            expiration,
            ..
        } => try_increase_allowance(deps, env, spender, amount, expiration),
        HandleMsg::DecreaseAllowance {
            spender,
            amount,
            expiration,
            ..
        } => try_decrease_allowance(deps, env, spender, amount, expiration),
        HandleMsg::TransferFrom {
            owner,
            recipient,
            amount,
            ..
        } => try_transfer_from(deps, env, &owner, &recipient, amount),
        HandleMsg::SendFrom {
            owner,
            recipient,
            amount,
            msg,
            ..
        } => try_send_from(deps, env, &owner, &recipient, amount, msg),

        // Burn
        HandleMsg::Burn { amount, .. } => try_burn(deps, env, amount),

        // Mint
        HandleMsg::Mint {
            amount, recipient, ..
        } => try_mint(deps, env, recipient, amount),

        // Other
        HandleMsg::SetMintingGov { minting } => stop_minting_gov(deps, env, minting),
        HandleMsg::SetIsBeingMinted { minting } => stop_being_minted(deps, env, minting),
        HandleMsg::ChangeAdmin { address, .. } => change_admin(deps, env, address),
        HandleMsg::AddAdmin { address, .. } => add_admin(deps, env, address),
        HandleMsg::RemoveAdmin { address, .. } => remove_admin(deps, env, address),
        HandleMsg::SetContractStatus { level, .. } => set_contract_status(deps, env, level),
        HandleMsg::AddMinters { minters, .. } => add_minters(deps, env, minters),
        HandleMsg::RemoveMinters { minters, .. } => remove_minters(deps, env, minters),
        HandleMsg::SetMinters { minters, .. } => set_minters(deps, env, minters),
        HandleMsg::PostInitialize { .. } => try_post_initialize(deps, env),
        HandleMsg::SetVotingContract {
            contract,
            gov_token,
        } => set_voting_contract(deps, env, contract, gov_token),
    };

    pad_response(response)
}

pub fn try_vote<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    proposal: u64,
    vote: VoteOption,
) -> HandleResult {
    let sender = deps.api.canonical_address(&env.message.sender)?;
    let config = Config::from_storage(&mut deps.storage);

    if !config.is_voting() {
        return Err(StdError::generic_err(
            "Voting is not enabled for this token",
        ));
    }

    let voting_contract = config.voting_contract();

    let balances = ReadonlyBalances::from_storage(&deps.storage);

    let from_balance = balances.account_amount(&sender);

    if from_balance == 0u128 {
        return Err(StdError::generic_err("Balance is 0"));
    }

    if from_balance > u64::max_value() as u128 {
        return Err(StdError::generic_err(
            "This should never happen, but you have too many tokens to vote",
        ));
    }

    let messages = vec![CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: voting_contract.address,
        callback_code_hash: voting_contract.hash,
        msg: to_binary(&VotingMessages::Vote {
            proposal,
            vote: SingleVote {
                address: env.message.sender,
                vote: vote_option_to_u32(vote),
                voting_power: from_balance as u64,
            },
        })?,
        send: vec![],
    })];

    Ok(HandleResponse {
        messages,
        log: vec![],
        data: None,
    })
}

// Must token contract execute it
pub fn set_voting_contract<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    contract: Option<Contract>,
    gov_token: bool,
) -> HandleResult {
    let mut config = Config::from_storage(&mut deps.storage);

    check_if_admin(&config, &env.message.sender)?;
    let mut messages = vec![];

    if let Some(contract) = contract {
        // staking token controls the voting
        if !gov_token {
            // todo: consider adding more entropy
            let password = ViewingKey::new(
                &env,
                &config.constants().unwrap().prng_seed,
                &env.block.time.clone().to_be_bytes(),
            );

            config.set_is_voting(true)?;
            config.set_voting_contract(&contract)?;
            config.set_voting_password(&password.0)?;

            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract.address,
                callback_code_hash: contract.hash,
                msg: to_binary(&VotingMessages::SetPassword {
                    password: password.0,
                })?,
                send: vec![],
            }))
        } else if !config.gov_token().is_empty() {
            // the gov token controls the voting
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.gov_token(),
                callback_code_hash: env.contract_code_hash,
                msg: to_binary(&TokenHandleMessage::SetVotingContract {
                    contract: Some(contract),
                    gov_token: true,
                })?,
                send: vec![],
            }))
        } else {
            return Err(StdError::generic_err(
                "Failed to set address for voting contract",
            ));
        }
    } else {
        // disable voting
        config.set_is_voting(false)?;
        if !config.gov_token().is_empty() {
            // the gov token controls the voting
            messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: config.gov_token(),
                callback_code_hash: env.contract_code_hash,
                msg: to_binary(&TokenHandleMessage::SetVotingContract {
                    contract: None,
                    gov_token: true,
                })?,
                send: vec![],
            }))
        }
    }

    Ok(HandleResponse {
        messages,
        log: vec![],
        data: None,
    })
}

// Must token contract execute it
pub fn try_post_initialize<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    let mut config = Config::from_storage(&mut deps.storage);

    config.set_gov_token(&env.message.sender)?;
    config.set_is_minting_gov(true)?;

    Ok(HandleResponse::default())
}

pub fn query<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, msg: QueryMsg) -> QueryResult {
    match msg {
        QueryMsg::TokenInfo {} => query_token_info(&deps.storage),
        QueryMsg::ExchangeRate {} => query_exchange_rate(),
        QueryMsg::Minters { .. } => query_minters(deps),
        _ => authenticated_queries(deps, msg),
    }
}

/// Burn tokens
///
/// Remove `amount` tokens from the system irreversibly, from signer account
///
/// @param amount the amount of money to burn
fn try_burn<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    let sender_address = deps.api.canonical_address(&env.message.sender)?;
    let amount = amount.u128();

    let mut balances = Balances::from_storage(&mut deps.storage);
    let mut account_balance = balances.balance(&sender_address);

    if let Some(new_account_balance) = account_balance.checked_sub(amount) {
        account_balance = new_account_balance;
    } else {
        return Err(StdError::generic_err(format!(
            "insufficient funds to burn: balance={}, required={}",
            account_balance, amount
        )));
    }

    balances.set_account_balance(&sender_address, account_balance);

    let mut config = Config::from_storage(&mut deps.storage);
    let mut total_supply = config.total_supply();
    if let Some(new_total_supply) = total_supply.checked_sub(amount) {
        total_supply = new_total_supply;
    } else {
        return Err(StdError::generic_err(
            "You're trying to burn more than is available in the total supply",
        ));
    }
    config.set_total_supply(total_supply);

    let mut messages = vec![];
    if config.is_minting_gov() && !config.gov_token().is_empty() {
        messages.push(snip20::transfer_from_msg(
            env.message.sender.clone(),
            env.contract.address,
            amount.into(),
            None,
            256,
            env.contract_code_hash.clone(),
            config.gov_token(),
        )?);
        messages.push(snip20::burn_msg(
            amount.into(),
            None,
            256,
            env.contract_code_hash.clone(),
            config.gov_token(),
        )?)
    }
    let ro_config = ReadonlyConfig::from_storage(&deps.storage);
    if let Some(msg) = update_voting_msg(deps, vec![&env.message.sender], &ro_config)? {
        messages.push(msg);
    }

    let res = HandleResponse {
        messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Burn { status: Success })?),
    };

    Ok(res)
}

fn try_mint<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    address: HumanAddr,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);

    let minters = config.minters();
    if !minters.contains(&env.message.sender) {
        return Err(StdError::generic_err(
            "Minting is allowed to minter accounts only",
        ));
    }

    let amount = amount.u128();

    let mut total_supply = config.total_supply();
    if let Some(new_total_supply) = total_supply.checked_add(amount) {
        total_supply = new_total_supply;
    } else {
        return Err(StdError::generic_err(
            "This mint attempt would increase the total supply above the supported maximum",
        ));
    }
    config.set_total_supply(total_supply);

    let mut messages = vec![];
    if config.is_minting_gov() && !config.gov_token().is_empty() {
        messages.push(snip20::mint_msg(
            address.clone(),
            amount.into(),
            None,
            256,
            env.contract_code_hash,
            config.gov_token(),
        )?)
    }

    let receipient_account = &deps.api.canonical_address(&address)?;

    let mut balances = Balances::from_storage(&mut deps.storage);

    let mut account_balance = balances.balance(receipient_account);

    if let Some(new_balance) = account_balance.checked_add(amount) {
        account_balance = new_balance;
    } else {
        // This error literally can not happen, since the account's funds are a subset
        // of the total supply, both are stored as u128, and we check for overflow of
        // the total supply just a couple lines before.
        // Still, writing this to cover all overflows.
        return Err(StdError::generic_err(
            "This mint attempt would increase the account's balance above the supported maximum",
        ));
    }

    balances.set_account_balance(receipient_account, account_balance);

    let ro_config = ReadonlyConfig::from_storage(&deps.storage);
    if let Some(msg) = update_voting_msg(deps, vec![&address], &ro_config)? {
        messages.push(msg);
    }

    let res = HandleResponse {
        messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Mint { status: Success })?),
    };

    Ok(res)
}

fn update_voting_msg<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    addresses_affected: Vec<&HumanAddr>,
    config: &ReadonlyConfig<S>,
) -> StdResult<Option<CosmosMsg>> {
    if config.is_voting() {
        let changes = update_voting_balances(deps, addresses_affected)?;

        let voting_contract = config.voting_contract();
        return Ok(Some(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: voting_contract.address,
            callback_code_hash: voting_contract.hash,
            msg: to_binary(&VotingMessages::NotifyBalanceChange { changes })?,
            send: vec![],
        })));
    }

    Ok(None)
}

pub fn authenticated_queries<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> QueryResult {
    let (addresses, key) = msg.get_validation_params();

    for address in addresses {
        let canonical_addr = deps.api.canonical_address(address)?;

        let expected_key = read_viewing_key(&deps.storage, &canonical_addr);

        if expected_key.is_none() {
            // Checking the key will take significant time. We don't want to exit immediately if it isn't set
            // in a way which will allow to time the command and determine if a viewing key doesn't exist
            key.check_viewing_key(&[0u8; VIEWING_KEY_SIZE]);
        } else if key.check_viewing_key(expected_key.unwrap().as_slice()) {
            return match msg {
                // Base
                QueryMsg::Balance { address, .. } => query_balance(&deps, &address),
                QueryMsg::TransferHistory {
                    address,
                    page,
                    page_size,
                    ..
                } => query_transactions(&deps, &address, page.unwrap_or(0), page_size),
                QueryMsg::Allowance { owner, spender, .. } => {
                    try_check_allowance(deps, owner, spender)
                }
                QueryMsg::ViewVote {
                    proposal, address, ..
                } => query_view_vote(deps, proposal, address),
                // QueryMsg::MultipleBalances {
                //     address, addresses, ..
                // } => {
                //     let config = ReadonlyConfig::from_storage(&deps.storage);
                //
                //     if address != config.constants()?.admin {
                //         return Err(StdError::unauthorized());
                //     }
                //
                //     query_multiple_balances(deps, addresses)
                // }
                _ => panic!("This query type does not require authentication"),
            };
        }
    }

    Ok(to_binary(&QueryAnswer::ViewingKeyError {
        msg: "Wrong viewing key for this address or viewing key not set".to_string(),
    })?)
}

fn query_minters<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>) -> StdResult<Binary> {
    let minters = ReadonlyConfig::from_storage(&deps.storage).minters();

    let response = QueryAnswer::Minters { minters };
    to_binary(&response)
}

fn query_view_vote<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    proposal: u64,
    address: HumanAddr,
) -> StdResult<Binary> {
    let ro_config = ReadonlyConfig::from_storage(&deps.storage);
    let voting_contract = ro_config.voting_contract();
    let resp: VoteResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: voting_contract.address,
        callback_code_hash: voting_contract.hash,
        msg: to_binary(&VotingMessages::QueryVote {
            proposal,
            address,
            password: ro_config.voting_password(),
        })?,
    }))?;

    let response = match resp {
        VoteResponse::QueryVote {
            proposal,
            voting_power,
            vote,
            address,
        } => QueryAnswer::ViewVote {
            proposal,
            vote,
            voting_power,
            address,
        },
    };

    to_binary(&response)
}

/// This function just returns a constant 1:1 rate to uscrt, since that's the purpose of this
/// contract.
fn query_exchange_rate() -> QueryResult {
    to_binary(&QueryAnswer::ExchangeRate {
        rate: Uint128(1),
        denom: "uscrt".to_string(),
    })
}

fn query_token_info<S: ReadonlyStorage>(storage: &S) -> QueryResult {
    let config = ReadonlyConfig::from_storage(storage);

    // a quick hack to make the token harder to use when it's being minted
    if config.is_being_minted() {
        return Ok(Binary::default());
    }

    let constants = config.constants()?;

    let total_supply = if constants.total_supply_is_public {
        Some(Uint128(config.total_supply()))
    } else {
        None
    };

    to_binary(&QueryAnswer::TokenInfo {
        name: constants.name,
        symbol: constants.symbol,
        decimals: constants.decimals,
        total_supply,
    })
}

pub fn query_transactions<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    account: &HumanAddr,
    page: u32,
    page_size: u32,
) -> StdResult<Binary> {
    let address = deps.api.canonical_address(account).unwrap();
    let txs = get_transfers(&deps.api, &deps.storage, &address, page, page_size)?;

    let result = QueryAnswer::TransferHistory { txs };
    to_binary(&result)
}

pub fn query_balance<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    account: &HumanAddr,
) -> StdResult<Binary> {
    let address = deps.api.canonical_address(account)?;

    let amount = Uint128(ReadonlyBalances::from_storage(&deps.storage).account_amount(&address));
    let response = QueryAnswer::Balance { amount };
    to_binary(&response)
}

// pub fn query_multiple_balances<S: Storage, A: Api, Q: Querier>(
//     deps: &Extern<S, A, Q>,
//     accounts: Vec<HumanAddr>,
// ) -> QueryResult {
//
//     let mut balances = balances::Balances::default();
//
//     for account in accounts {
//         let amount = Uint128(
//             ReadonlyBalances::from_storage(&deps.storage)
//                 .account_amount(&deps.api.canonical_address(&account)?),
//         )
//         .u128();
//         balances.0.push(Balance { account, amount });
//     }
//
//     let response = QueryAnswer::MultipleBalances {
//         balances: balances.to_binary()?,
//     };
//     to_binary(&response)
// }
fn stop_minting_gov<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    minting: bool,
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);

    check_if_admin(&config, &env.message.sender)?;

    config.set_is_minting_gov(minting)?;

    let mut messages: Vec<CosmosMsg> = vec![];

    if !config.gov_token().is_empty() {
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: config.gov_token(),
            callback_code_hash: env.contract_code_hash,
            msg: to_binary(&HandleMsg::SetIsBeingMinted { minting })?,
            send: vec![],
        }))
    }

    Ok(HandleResponse {
        messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::StopMintingGov {
            status: Success,
            minting,
        })?),
    })
}

fn stop_being_minted<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    minting: bool,
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);

    check_if_admin(&config, &env.message.sender)?;

    // this is just for turning this off - we don't want the option to turn it on again
    if config.is_being_minted() {
        config.set_is_being_minted(minting)?;
    }

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::SetIsBeingMinted {
            status: Success,
            minting,
        })?),
    })
}

fn add_admin<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    address: HumanAddr,
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);

    check_if_admin(&config, &env.message.sender)?;

    let mut consts = config.constants()?;

    if !consts.admin.contains(&address) {
        consts.admin.push(address);
        config.set_constants(&consts)?;
    }

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::AddAdmin { status: Success })?),
    })
}

fn remove_admin<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    address: HumanAddr,
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);

    check_if_admin(&config, &env.message.sender)?;

    let mut consts = config.constants()?;

    if consts.admin.contains(&address) {
        let _ = consts.admin.drain_filter(|a| a == &address);
        config.set_constants(&consts)?;
    }

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::AddAdmin { status: Success })?),
    })
}

fn change_admin<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    address: HumanAddr,
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);

    check_if_admin(&config, &env.message.sender)?;

    let mut consts = config.constants()?;
    consts.admin = vec![address];
    config.set_constants(&consts)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::ChangeAdmin { status: Success })?),
    })
}

pub fn try_set_key<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    key: String,
) -> StdResult<HandleResponse> {
    let vk = ViewingKey(key);

    let message_sender = deps.api.canonical_address(&env.message.sender)?;
    write_viewing_key(&mut deps.storage, &message_sender, &vk);

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::SetViewingKey { status: Success })?),
    })
}

pub fn try_create_key<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    entropy: String,
) -> StdResult<HandleResponse> {
    let constants = ReadonlyConfig::from_storage(&deps.storage).constants()?;
    let prng_seed = constants.prng_seed;

    let key = ViewingKey::new(&env, &prng_seed, (&entropy).as_ref());

    let message_sender = deps.api.canonical_address(&env.message.sender)?;
    write_viewing_key(&mut deps.storage, &message_sender, &key);

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::CreateViewingKey { key })?),
    })
}

fn set_contract_status<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    status_level: ContractStatusLevel,
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);

    check_if_admin(&config, &env.message.sender)?;

    config.set_contract_status(status_level);

    Ok(HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::SetContractStatus {
            status: Success,
        })?),
    })
}

pub fn try_check_allowance<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    owner: HumanAddr,
    spender: HumanAddr,
) -> StdResult<Binary> {
    let owner_address = deps.api.canonical_address(&owner)?;
    let spender_address = deps.api.canonical_address(&spender)?;

    let allowance = read_allowance(&deps.storage, &owner_address, &spender_address)?;

    let response = QueryAnswer::Allowance {
        owner,
        spender,
        allowance: Uint128(allowance.amount),
        expiration: allowance.expiration,
    };
    to_binary(&response)
}

fn try_transfer_impl<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: &Env,
    recipient: &HumanAddr,
    amount: Uint128,
) -> StdResult<()> {
    let sender_address = deps.api.canonical_address(&env.message.sender)?;
    let recipient_address = deps.api.canonical_address(recipient)?;

    perform_transfer(
        &mut deps.storage,
        &sender_address,
        &recipient_address,
        amount.u128(),
    )?;

    let symbol = Config::from_storage(&mut deps.storage).constants()?.symbol;

    store_transfer(
        &mut deps.storage,
        &sender_address,
        &sender_address,
        &recipient_address,
        amount,
        symbol,
    )?;

    Ok(())
}

fn try_transfer<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    recipient: &HumanAddr,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    let config = Config::from_storage(&mut deps.storage);

    if recipient == &config.constants()?.creator {
        return Err(StdError::generic_err(
            "Cannot send tokens to staking contract".to_string(),
        ));
    }
    let mut messages = vec![];
    if config.is_minting_gov() && !config.gov_token().is_empty() {
        messages.push(snip20::transfer_from_msg(
            env.message.sender.clone(),
            recipient.clone(),
            amount,
            None,
            256,
            env.contract_code_hash.clone(),
            config.gov_token(),
        )?)
    }

    try_transfer_impl(deps, &env, recipient, amount)?;

    let ro_config = ReadonlyConfig::from_storage(&deps.storage);
    if let Some(msg) = update_voting_msg(deps, vec![&env.message.sender, recipient], &ro_config)? {
        messages.push(msg);
    }

    let res = HandleResponse {
        messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Transfer { status: Success })?),
    };
    Ok(res)
}

fn try_add_receiver_api_callback<S: ReadonlyStorage>(
    messages: &mut Vec<CosmosMsg>,
    storage: &S,
    recipient: &HumanAddr,
    msg: Option<Binary>,
    sender: HumanAddr,
    from: HumanAddr,
    amount: Uint128,
) -> StdResult<()> {
    let receiver_hash = get_receiver_hash(storage, recipient);
    if let Some(receiver_hash) = receiver_hash {
        let receiver_hash = receiver_hash?;
        let receiver_msg = Snip20ReceiveMsg::new(sender, from, amount, msg);
        let callback_msg = receiver_msg.into_cosmos_msg(receiver_hash, recipient.clone())?;

        messages.push(callback_msg);
    }
    Ok(())
}

fn try_send<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    recipient: &HumanAddr,
    amount: Uint128,
    msg: Option<Binary>,
) -> StdResult<HandleResponse> {
    let sender = env.message.sender.clone();

    let mut messages = vec![];
    let config = Config::from_storage(&mut deps.storage);

    if config.is_minting_gov() && !config.gov_token().is_empty() {
        messages.push(snip20::transfer_from_msg(
            sender.clone(),
            recipient.clone(),
            amount,
            None,
            256,
            env.contract_code_hash.clone(),
            config.gov_token(),
        )?)
    }

    try_transfer_impl(deps, &env, recipient, amount)?;

    let ro_config = ReadonlyConfig::from_storage(&deps.storage);
    if let Some(msg) = update_voting_msg(deps, vec![&env.message.sender, recipient], &ro_config)? {
        messages.push(msg);
    }

    try_add_receiver_api_callback(
        &mut messages,
        &deps.storage,
        recipient,
        msg,
        sender.clone(),
        sender,
        amount,
    )?;

    let res = HandleResponse {
        messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::Send { status: Success })?),
    };
    Ok(res)
}

fn try_register_receive<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    code_hash: String,
) -> StdResult<HandleResponse> {
    set_receiver_hash(&mut deps.storage, &env.message.sender, code_hash);
    let res = HandleResponse {
        messages: vec![],
        log: vec![log("register_status", "success")],
        data: Some(to_binary(&HandleAnswer::RegisterReceive {
            status: Success,
        })?),
    };
    Ok(res)
}

fn insufficient_allowance(allowance: u128, required: u128) -> StdError {
    StdError::generic_err(format!(
        "insufficient allowance: allowance={}, required={}",
        allowance, required
    ))
}

fn update_voting_balances<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    addresses: Vec<&HumanAddr>,
) -> StdResult<Vec<VoteChange>> {
    let balances = ReadonlyBalances::from_storage(&deps.storage);
    let mut changes = vec![];
    for address in addresses {
        let address_canonical = deps.api.canonical_address(address)?;

        let balance = balances.account_amount(&address_canonical);

        changes.push(VoteChange {
            voting_power: balance as u64,
            address: address.clone(),
        })
    }

    Ok(changes)
}

fn try_transfer_from_impl<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: &HumanAddr,
    recipient: &HumanAddr,
    amount: Uint128,
    is_being_minted: bool,
    is_sent_by_admin: bool,
) -> StdResult<()> {
    let spender_address = deps.api.canonical_address(&env.message.sender)?;
    let owner_address = deps.api.canonical_address(owner)?;
    let recipient_address = deps.api.canonical_address(recipient)?;
    let amount_raw = amount.u128();

    if !(is_being_minted && is_sent_by_admin) {
        let mut allowance = read_allowance(&deps.storage, &owner_address, &spender_address)?;

        if allowance.expiration.map(|ex| ex < env.block.time) == Some(true) {
            allowance.amount = 0;
            write_allowance(
                &mut deps.storage,
                &owner_address,
                &spender_address,
                allowance,
            )?;
            return Err(insufficient_allowance(0, amount_raw));
        }

        if let Some(new_allowance) = allowance.amount.checked_sub(amount_raw) {
            allowance.amount = new_allowance;
        } else {
            return Err(insufficient_allowance(allowance.amount, amount_raw));
        }

        write_allowance(
            &mut deps.storage,
            &owner_address,
            &spender_address,
            allowance,
        )?;
    }

    perform_transfer(
        &mut deps.storage,
        &owner_address,
        &recipient_address,
        amount_raw,
    )?;

    let symbol = Config::from_storage(&mut deps.storage).constants()?.symbol;

    store_transfer(
        &mut deps.storage,
        &owner_address,
        &spender_address,
        &recipient_address,
        amount,
        symbol,
    )?;

    Ok(())
}

fn try_transfer_from<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: &HumanAddr,
    recipient: &HumanAddr,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    let config = Config::from_storage(&mut deps.storage);

    let mut messages = vec![];
    if config.is_minting_gov() && !config.gov_token().is_empty() {
        messages.push(snip20::transfer_from_msg(
            owner.clone(),
            recipient.clone(),
            amount,
            None,
            256,
            env.contract_code_hash.clone(),
            config.gov_token(),
        )?)
    }

    let is_being_minted = config.is_being_minted();
    let is_admin = is_admin(&config, &env.message.sender)?;
    try_transfer_from_impl(
        deps,
        env,
        owner,
        recipient,
        amount,
        is_being_minted,
        is_admin,
    )?;

    let ro_config = ReadonlyConfig::from_storage(&deps.storage);
    if let Some(msg) = update_voting_msg(deps, vec![owner, recipient], &ro_config)? {
        messages.push(msg);
    }

    let res = HandleResponse {
        messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::TransferFrom { status: Success })?),
    };
    Ok(res)
}

fn try_send_from<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    owner: &HumanAddr,
    recipient: &HumanAddr,
    amount: Uint128,
    msg: Option<Binary>,
) -> StdResult<HandleResponse> {
    let sender = env.message.sender.clone();

    let mut messages = vec![];

    try_add_receiver_api_callback(
        &mut messages,
        &deps.storage,
        recipient,
        msg,
        sender,
        owner.clone(),
        amount,
    )?;

    let config = Config::from_storage(&mut deps.storage);

    if config.is_minting_gov() && !config.gov_token().is_empty() {
        messages.push(snip20::transfer_from_msg(
            owner.clone(),
            recipient.clone(),
            amount,
            None,
            256,
            env.contract_code_hash.clone(),
            config.gov_token(),
        )?)
    }

    let is_being_minted = config.is_being_minted();
    let is_admin = is_admin(&config, &env.message.sender)?;

    try_transfer_from_impl(
        deps,
        env,
        owner,
        recipient,
        amount,
        is_being_minted,
        is_admin,
    )?;

    let ro_config = ReadonlyConfig::from_storage(&deps.storage);
    if let Some(msg) = update_voting_msg(deps, vec![owner, recipient], &ro_config)? {
        messages.push(msg);
    }

    let res = HandleResponse {
        messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::SendFrom { status: Success })?),
    };
    Ok(res)
}

fn try_increase_allowance<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    spender: HumanAddr,
    amount: Uint128,
    expiration: Option<u64>,
) -> StdResult<HandleResponse> {
    let owner_address = deps.api.canonical_address(&env.message.sender)?;
    let spender_address = deps.api.canonical_address(&spender)?;

    let mut allowance = read_allowance(&deps.storage, &owner_address, &spender_address)?;
    allowance.amount = allowance.amount.saturating_add(amount.u128());
    if expiration.is_some() {
        allowance.expiration = expiration;
    }
    let new_amount = allowance.amount;
    write_allowance(
        &mut deps.storage,
        &owner_address,
        &spender_address,
        allowance,
    )?;

    let res = HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::IncreaseAllowance {
            owner: env.message.sender,
            spender,
            allowance: Uint128(new_amount),
        })?),
    };
    Ok(res)
}

fn try_decrease_allowance<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    spender: HumanAddr,
    amount: Uint128,
    expiration: Option<u64>,
) -> StdResult<HandleResponse> {
    let owner_address = deps.api.canonical_address(&env.message.sender)?;
    let spender_address = deps.api.canonical_address(&spender)?;

    let mut allowance = read_allowance(&deps.storage, &owner_address, &spender_address)?;
    allowance.amount = allowance.amount.saturating_sub(amount.u128());
    if expiration.is_some() {
        allowance.expiration = expiration;
    }
    let new_amount = allowance.amount;
    write_allowance(
        &mut deps.storage,
        &owner_address,
        &spender_address,
        allowance,
    )?;

    let res = HandleResponse {
        messages: vec![],
        log: vec![],
        data: Some(to_binary(&HandleAnswer::DecreaseAllowance {
            owner: env.message.sender,
            spender,
            allowance: Uint128(new_amount),
        })?),
    };
    Ok(res)
}

fn add_minters<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    minters_to_add: Vec<HumanAddr>,
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);

    check_if_admin(&config, &env.message.sender)?;

    config.add_minters(minters_to_add.clone())?;

    let mut messages = vec![];
    if !config.gov_token().is_empty() {
        messages.push(snip20::add_minters_msg(
            minters_to_add,
            None,
            256,
            env.contract_code_hash,
            config.gov_token(),
        )?)
    }

    Ok(HandleResponse {
        messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::AddMinters { status: Success })?),
    })
}

fn remove_minters<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    minters_to_remove: Vec<HumanAddr>,
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);

    check_if_admin(&config, &env.message.sender)?;

    config.remove_minters(minters_to_remove.clone())?;

    let mut messages = vec![];
    if !config.gov_token().is_empty() {
        messages.push(snip20::remove_minters_msg(
            minters_to_remove,
            None,
            256,
            env.contract_code_hash,
            config.gov_token(),
        )?)
    }

    Ok(HandleResponse {
        messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::RemoveMinters { status: Success })?),
    })
}

fn set_minters<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    minters_to_set: Vec<HumanAddr>,
) -> StdResult<HandleResponse> {
    let mut config = Config::from_storage(&mut deps.storage);

    check_if_admin(&config, &env.message.sender)?;

    config.set_minters(minters_to_set.clone())?;

    let mut messages = vec![];
    if !config.gov_token().is_empty() {
        messages.push(snip20::set_minters_msg(
            minters_to_set,
            None,
            256,
            env.contract_code_hash,
            config.gov_token(),
        )?)
    }

    Ok(HandleResponse {
        messages,
        log: vec![],
        data: Some(to_binary(&HandleAnswer::SetMinters { status: Success })?),
    })
}

fn perform_transfer<T: Storage>(
    store: &mut T,
    from: &CanonicalAddr,
    to: &CanonicalAddr,
    amount: u128,
) -> StdResult<()> {
    let mut balances = Balances::from_storage(store);

    let mut from_balance = balances.balance(from);
    if let Some(new_from_balance) = from_balance.checked_sub(amount) {
        from_balance = new_from_balance;
    } else {
        return Err(StdError::generic_err(format!(
            "insufficient funds: balance={}, required={}",
            from_balance, amount
        )));
    }
    balances.set_account_balance(from, from_balance);

    let mut to_balance = balances.balance(to);
    to_balance = to_balance.checked_add(amount).ok_or_else(|| {
        StdError::generic_err("This tx will literally make them too rich. Try transferring less")
    })?;
    balances.set_account_balance(to, to_balance);

    Ok(())
}

fn is_admin<S: Storage>(config: &Config<S>, account: &HumanAddr) -> StdResult<bool> {
    let consts = config.constants()?;
    if consts.admin.contains(account) {
        return Ok(true);
    }

    Ok(false)
}

fn check_if_admin<S: Storage>(config: &Config<S>, account: &HumanAddr) -> StdResult<()> {
    if !is_admin(config, account)? {
        return Err(StdError::generic_err(
            "This is an admin command. Admin commands can only be run from admin address",
        ));
    }

    Ok(())
}

fn is_valid_name(name: &str) -> bool {
    let len = name.len();
    3 <= len && len <= 30
}

#[allow(dead_code)]
fn is_valid_symbol(symbol: &str) -> bool {
    let len = symbol.len();
    let len_is_valid = 3 <= len && len <= 12;

    len_is_valid
        && symbol
            .bytes()
            .all(|byte| (b'A' <= byte && byte <= b'Z') || (b'a' <= byte && byte <= b'z'))
}

#[cfg(test)]
mod tests {
    use std::any::Any;

    use cosmwasm_std::testing::*;
    use cosmwasm_std::{from_binary, BlockInfo, ContractInfo, MessageInfo, QueryResponse, WasmMsg};

    use crate::msg::ResponseStatus;
    use crate::msg::{InitConfig, InitialBalance};

    use super::*;

    // Helper functions

    fn init_helper(
        initial_balances: Vec<InitialBalance>,
    ) -> (
        StdResult<InitResponse>,
        Extern<MockStorage, MockApi, MockQuerier>,
    ) {
        let mut deps = mock_dependencies(20, &[]);
        let env = mock_env("instantiator", &[]);

        let init_msg = InitMsg {
            name: "sec-sec".to_string(),
            admin: Some(vec![HumanAddr("admin".to_string())]),
            symbol: "SECSEC".to_string(),
            decimals: 8,
            initial_balances: Some(initial_balances),
            prng_seed: Binary::from("lolz fun yay".as_bytes()),
            config: None,
            init_hook: None,
            token_code_id: None,
            is_being_minted: None,
        };

        (init(&mut deps, env, init_msg), deps)
    }

    /// Will return a ViewingKey only for the first account in `initial_balances`
    fn auth_query_helper(
        initial_balances: Vec<InitialBalance>,
    ) -> (ViewingKey, Extern<MockStorage, MockApi, MockQuerier>) {
        let (init_result, mut deps) = init_helper(initial_balances.clone());
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let account = initial_balances[0].address.clone();
        let create_vk_msg = HandleMsg::CreateViewingKey {
            entropy: "42".to_string(),
            padding: None,
        };
        let handle_response = handle(&mut deps, mock_env(account.0, &[]), create_vk_msg).unwrap();
        let vk = match from_binary(&handle_response.data.unwrap()).unwrap() {
            HandleAnswer::CreateViewingKey { key } => key,
            _ => panic!("Unexpected result from handle"),
        };

        (vk, deps)
    }

    fn extract_error_msg<T: Any>(error: StdResult<T>) -> String {
        match error {
            Ok(response) => {
                let bin_err = (&response as &dyn Any)
                    .downcast_ref::<QueryResponse>()
                    .expect("An error was expected, but no error could be extracted");
                match from_binary(bin_err).unwrap() {
                    QueryAnswer::ViewingKeyError { msg } => msg,
                    _ => panic!("Unexpected query answer"),
                }
            }
            Err(err) => match err {
                StdError::GenericErr { msg, .. } => msg,
                _ => panic!("Unexpected result from init"),
            },
        }
    }

    fn ensure_success(handle_result: HandleResponse) -> bool {
        let handle_result: HandleAnswer = from_binary(&handle_result.data.unwrap()).unwrap();

        match handle_result {
            HandleAnswer::Deposit { status }
            | HandleAnswer::Redeem { status }
            | HandleAnswer::Transfer { status }
            | HandleAnswer::Send { status }
            | HandleAnswer::RegisterReceive { status }
            | HandleAnswer::SetViewingKey { status }
            | HandleAnswer::TransferFrom { status }
            | HandleAnswer::SendFrom { status }
            | HandleAnswer::ChangeAdmin { status }
            | HandleAnswer::SetContractStatus { status } => {
                matches!(status, ResponseStatus::Success {..})
            }
            _ => panic!("HandleAnswer not supported for success extraction"),
        }
    }

    // Init tests

    #[test]
    fn test_init_sanity() {
        let (init_result, deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("lebron".to_string()),
            amount: Uint128(5000),
        }]);
        assert_eq!(init_result.unwrap(), InitResponse::default());

        let config = ReadonlyConfig::from_storage(&deps.storage);
        let constants = config.constants().unwrap();
        assert_eq!(config.total_supply(), 5000);
        assert_eq!(config.contract_status(), ContractStatusLevel::NormalRun);
        assert_eq!(constants.name, "sec-sec".to_string());
        assert_eq!(constants.admin, HumanAddr("admin".to_string()));
        assert_eq!(constants.symbol, "SECSEC".to_string());
        assert_eq!(constants.decimals, 8);
        assert_eq!(
            constants.prng_seed,
            sha_256("lolz fun yay".to_owned().as_bytes())
        );
        assert_eq!(constants.total_supply_is_public, false);
    }

    #[test]
    fn test_total_supply_overflow() {
        let (init_result, _deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("lebron".to_string()),
            amount: Uint128(u128::max_value()),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let (init_result, _deps) = init_helper(vec![
            InitialBalance {
                address: HumanAddr("lebron".to_string()),
                amount: Uint128(u128::max_value()),
            },
            InitialBalance {
                address: HumanAddr("giannis".to_string()),
                amount: Uint128(1),
            },
        ]);
        let error = extract_error_msg(init_result);
        assert_eq!(
            error,
            "The sum of all initial balances exceeds the maximum possible total supply"
        );
    }

    // Handle tests

    #[test]
    fn test_handle_transfer() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("bob".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let handle_msg = HandleMsg::Transfer {
            recipient: HumanAddr("alice".to_string()),
            amount: Uint128(1000),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        let result = handle_result.unwrap();
        assert!(ensure_success(result));
        let bob_canonical = deps
            .api
            .canonical_address(&HumanAddr("bob".to_string()))
            .unwrap();
        let alice_canonical = deps
            .api
            .canonical_address(&HumanAddr("alice".to_string()))
            .unwrap();
        let balances = ReadonlyBalances::from_storage(&deps.storage);
        assert_eq!(5000 - 1000, balances.account_amount(&bob_canonical));
        assert_eq!(1000, balances.account_amount(&alice_canonical));

        let handle_msg = HandleMsg::Transfer {
            recipient: HumanAddr("alice".to_string()),
            amount: Uint128(10000),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        let error = extract_error_msg(handle_result);
        assert!(error.contains("insufficient funds"));
    }

    #[test]
    fn test_handle_send() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("bob".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let handle_msg = HandleMsg::RegisterReceive {
            code_hash: "this_is_a_hash_of_a_code".to_string(),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("contract", &[]), handle_msg);
        let result = handle_result.unwrap();
        assert!(ensure_success(result));

        let handle_msg = HandleMsg::Send {
            recipient: HumanAddr("contract".to_string()),
            amount: Uint128(100),
            padding: None,
            msg: Some(to_binary("hey hey you you").unwrap()),
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        let result = handle_result.unwrap();
        assert!(ensure_success(result.clone()));
        assert!(result.messages.contains(&CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr("contract".to_string()),
            callback_code_hash: "this_is_a_hash_of_a_code".to_string(),
            msg: Snip20ReceiveMsg::new(
                HumanAddr("bob".to_string()),
                HumanAddr("bob".to_string()),
                Uint128(100),
                Some(to_binary("hey hey you you").unwrap()),
            )
            .into_binary()
            .unwrap(),
            send: vec![],
        })));
    }

    #[test]
    fn test_handle_register_receive() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("bob".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let handle_msg = HandleMsg::RegisterReceive {
            code_hash: "this_is_a_hash_of_a_code".to_string(),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("contract", &[]), handle_msg);
        let result = handle_result.unwrap();
        assert!(ensure_success(result));

        let hash = get_receiver_hash(&deps.storage, &HumanAddr("contract".to_string()))
            .unwrap()
            .unwrap();
        assert_eq!(hash, "this_is_a_hash_of_a_code".to_string());
    }

    #[test]
    fn test_handle_create_viewing_key() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("bob".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let handle_msg = HandleMsg::CreateViewingKey {
            entropy: "".to_string(),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        assert!(
            handle_result.is_ok(),
            "handle() failed: {}",
            handle_result.err().unwrap()
        );
        let answer: HandleAnswer = from_binary(&handle_result.unwrap().data.unwrap()).unwrap();

        let key = match answer {
            HandleAnswer::CreateViewingKey { key } => key,
            _ => panic!("NOPE"),
        };
        let bob_canonical = deps
            .api
            .canonical_address(&HumanAddr("bob".to_string()))
            .unwrap();
        let saved_vk = read_viewing_key(&deps.storage, &bob_canonical).unwrap();
        assert!(key.check_viewing_key(saved_vk.as_slice()));
    }

    #[test]
    fn test_handle_set_viewing_key() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("bob".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        // Set VK
        let handle_msg = HandleMsg::SetViewingKey {
            key: "hi lol".to_string(),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        let unwrapped_result: HandleAnswer =
            from_binary(&handle_result.unwrap().data.unwrap()).unwrap();
        assert_eq!(
            to_binary(&unwrapped_result).unwrap(),
            to_binary(&HandleAnswer::SetViewingKey {
                status: ResponseStatus::Success
            })
            .unwrap(),
        );

        // Set valid VK
        let actual_vk = ViewingKey("x".to_string().repeat(VIEWING_KEY_SIZE));
        let handle_msg = HandleMsg::SetViewingKey {
            key: actual_vk.0.clone(),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        let unwrapped_result: HandleAnswer =
            from_binary(&handle_result.unwrap().data.unwrap()).unwrap();
        assert_eq!(
            to_binary(&unwrapped_result).unwrap(),
            to_binary(&HandleAnswer::SetViewingKey { status: Success }).unwrap(),
        );
        let bob_canonical = deps
            .api
            .canonical_address(&HumanAddr("bob".to_string()))
            .unwrap();
        let saved_vk = read_viewing_key(&deps.storage, &bob_canonical).unwrap();
        assert!(actual_vk.check_viewing_key(&saved_vk));
    }

    #[test]
    fn test_handle_transfer_from() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("bob".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        // Transfer before allowance
        let handle_msg = HandleMsg::TransferFrom {
            owner: HumanAddr("bob".to_string()),
            recipient: HumanAddr("alice".to_string()),
            amount: Uint128(2500),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("alice", &[]), handle_msg);
        let error = extract_error_msg(handle_result);
        assert!(error.contains("insufficient allowance"));

        // Transfer more than allowance
        let handle_msg = HandleMsg::IncreaseAllowance {
            spender: HumanAddr("alice".to_string()),
            amount: Uint128(2000),
            padding: None,
            expiration: Some(1_571_797_420),
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        assert!(
            handle_result.is_ok(),
            "handle() failed: {}",
            handle_result.err().unwrap()
        );
        let handle_msg = HandleMsg::TransferFrom {
            owner: HumanAddr("bob".to_string()),
            recipient: HumanAddr("alice".to_string()),
            amount: Uint128(2500),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("alice", &[]), handle_msg);
        let error = extract_error_msg(handle_result);
        assert!(error.contains("insufficient allowance"));

        // Transfer after allowance expired
        let handle_msg = HandleMsg::TransferFrom {
            owner: HumanAddr("bob".to_string()),
            recipient: HumanAddr("alice".to_string()),
            amount: Uint128(2000),
            padding: None,
        };
        let handle_result = handle(
            &mut deps,
            Env {
                block: BlockInfo {
                    height: 12_345,
                    time: 1_571_797_420,
                    chain_id: "cosmos-testnet-14002".to_string(),
                },
                message: MessageInfo {
                    sender: HumanAddr("bob".to_string()),
                    sent_funds: vec![],
                },
                contract: ContractInfo {
                    address: HumanAddr::from(MOCK_CONTRACT_ADDR),
                },
                contract_key: Some("".to_string()),
                contract_code_hash: "".to_string(),
            },
            handle_msg,
        );
        let error = extract_error_msg(handle_result);
        assert!(error.contains("insufficient allowance"));

        // Sanity check
        let handle_msg = HandleMsg::TransferFrom {
            owner: HumanAddr("bob".to_string()),
            recipient: HumanAddr("alice".to_string()),
            amount: Uint128(2000),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("alice", &[]), handle_msg);
        assert!(
            handle_result.is_ok(),
            "handle() failed: {}",
            handle_result.err().unwrap()
        );
        let bob_canonical = deps
            .api
            .canonical_address(&HumanAddr("bob".to_string()))
            .unwrap();
        let alice_canonical = deps
            .api
            .canonical_address(&HumanAddr("alice".to_string()))
            .unwrap();
        let bob_balance = crate::state::ReadonlyBalances::from_storage(&deps.storage)
            .account_amount(&bob_canonical);
        let alice_balance = crate::state::ReadonlyBalances::from_storage(&deps.storage)
            .account_amount(&alice_canonical);
        assert_eq!(bob_balance, 5000 - 2000);
        assert_eq!(alice_balance, 2000);
        let total_supply = ReadonlyConfig::from_storage(&deps.storage).total_supply();
        assert_eq!(total_supply, 5000);

        // Second send more than allowance
        let handle_msg = HandleMsg::TransferFrom {
            owner: HumanAddr("bob".to_string()),
            recipient: HumanAddr("alice".to_string()),
            amount: Uint128(1),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("alice", &[]), handle_msg);
        let error = extract_error_msg(handle_result);
        assert!(error.contains("insufficient allowance"));
    }

    #[test]
    fn test_handle_send_from() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("bob".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        // Send before allowance
        let handle_msg = HandleMsg::SendFrom {
            owner: HumanAddr("bob".to_string()),
            recipient: HumanAddr("alice".to_string()),
            amount: Uint128(2500),
            msg: None,
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("alice", &[]), handle_msg);
        let error = extract_error_msg(handle_result);
        assert!(error.contains("insufficient allowance"));

        // Send more than allowance
        let handle_msg = HandleMsg::IncreaseAllowance {
            spender: HumanAddr("alice".to_string()),
            amount: Uint128(2000),
            padding: None,
            expiration: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        assert!(
            handle_result.is_ok(),
            "handle() failed: {}",
            handle_result.err().unwrap()
        );
        let handle_msg = HandleMsg::SendFrom {
            owner: HumanAddr("bob".to_string()),
            recipient: HumanAddr("alice".to_string()),
            amount: Uint128(2500),
            msg: None,
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("alice", &[]), handle_msg);
        let error = extract_error_msg(handle_result);
        assert!(error.contains("insufficient allowance"));

        // Sanity check
        let handle_msg = HandleMsg::RegisterReceive {
            code_hash: "lolz".to_string(),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("contract", &[]), handle_msg);
        assert!(
            handle_result.is_ok(),
            "handle() failed: {}",
            handle_result.err().unwrap()
        );
        let send_msg = Binary::from(r#"{ "some_msg": { "some_key": "some_val" } }"#.as_bytes());
        let snip20_msg = Snip20ReceiveMsg::new(
            HumanAddr("alice".to_string()),
            HumanAddr("bob".to_string()),
            Uint128(2000),
            Some(send_msg.clone()),
        );
        let handle_msg = HandleMsg::SendFrom {
            owner: HumanAddr("bob".to_string()),
            recipient: HumanAddr("contract".to_string()),
            amount: Uint128(2000),
            msg: Some(send_msg),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("alice", &[]), handle_msg);
        assert!(
            handle_result.is_ok(),
            "handle() failed: {}",
            handle_result.err().unwrap()
        );
        assert!(handle_result.unwrap().messages.contains(
            &snip20_msg
                .into_cosmos_msg("lolz".to_string(), HumanAddr("contract".to_string()))
                .unwrap()
        ));
        let bob_canonical = deps
            .api
            .canonical_address(&HumanAddr("bob".to_string()))
            .unwrap();
        let contract_canonical = deps
            .api
            .canonical_address(&HumanAddr("contract".to_string()))
            .unwrap();
        let bob_balance = crate::state::ReadonlyBalances::from_storage(&deps.storage)
            .account_amount(&bob_canonical);
        let contract_balance = crate::state::ReadonlyBalances::from_storage(&deps.storage)
            .account_amount(&contract_canonical);
        assert_eq!(bob_balance, 5000 - 2000);
        assert_eq!(contract_balance, 2000);
        let total_supply = ReadonlyConfig::from_storage(&deps.storage).total_supply();
        assert_eq!(total_supply, 5000);

        // Second send more than allowance
        let handle_msg = HandleMsg::SendFrom {
            owner: HumanAddr("bob".to_string()),
            recipient: HumanAddr("alice".to_string()),
            amount: Uint128(1),
            msg: None,
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("alice", &[]), handle_msg);
        let error = extract_error_msg(handle_result);
        assert!(error.contains("insufficient allowance"));
    }

    #[test]
    fn test_handle_decrease_allowance() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("bob".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let handle_msg = HandleMsg::DecreaseAllowance {
            spender: HumanAddr("alice".to_string()),
            amount: Uint128(2000),
            padding: None,
            expiration: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        assert!(
            handle_result.is_ok(),
            "handle() failed: {}",
            handle_result.err().unwrap()
        );

        let bob_canonical = deps
            .api
            .canonical_address(&HumanAddr("bob".to_string()))
            .unwrap();
        let alice_canonical = deps
            .api
            .canonical_address(&HumanAddr("alice".to_string()))
            .unwrap();

        let allowance = read_allowance(&deps.storage, &bob_canonical, &alice_canonical).unwrap();
        assert_eq!(
            allowance,
            crate::state::Allowance {
                amount: 0,
                expiration: None,
            }
        );

        let handle_msg = HandleMsg::IncreaseAllowance {
            spender: HumanAddr("alice".to_string()),
            amount: Uint128(2000),
            padding: None,
            expiration: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        assert!(
            handle_result.is_ok(),
            "handle() failed: {}",
            handle_result.err().unwrap()
        );

        let handle_msg = HandleMsg::DecreaseAllowance {
            spender: HumanAddr("alice".to_string()),
            amount: Uint128(50),
            padding: None,
            expiration: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        assert!(
            handle_result.is_ok(),
            "handle() failed: {}",
            handle_result.err().unwrap()
        );

        let allowance = read_allowance(&deps.storage, &bob_canonical, &alice_canonical).unwrap();
        assert_eq!(
            allowance,
            crate::state::Allowance {
                amount: 1950,
                expiration: None,
            }
        );
    }

    #[test]
    fn test_handle_increase_allowance() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("bob".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let handle_msg = HandleMsg::IncreaseAllowance {
            spender: HumanAddr("alice".to_string()),
            amount: Uint128(2000),
            padding: None,
            expiration: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        assert!(
            handle_result.is_ok(),
            "handle() failed: {}",
            handle_result.err().unwrap()
        );

        let bob_canonical = deps
            .api
            .canonical_address(&HumanAddr("bob".to_string()))
            .unwrap();
        let alice_canonical = deps
            .api
            .canonical_address(&HumanAddr("alice".to_string()))
            .unwrap();

        let allowance = read_allowance(&deps.storage, &bob_canonical, &alice_canonical).unwrap();
        assert_eq!(
            allowance,
            crate::state::Allowance {
                amount: 2000,
                expiration: None,
            }
        );

        let handle_msg = HandleMsg::IncreaseAllowance {
            spender: HumanAddr("alice".to_string()),
            amount: Uint128(2000),
            padding: None,
            expiration: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        assert!(
            handle_result.is_ok(),
            "handle() failed: {}",
            handle_result.err().unwrap()
        );

        let allowance = read_allowance(&deps.storage, &bob_canonical, &alice_canonical).unwrap();
        assert_eq!(
            allowance,
            crate::state::Allowance {
                amount: 4000,
                expiration: None,
            }
        );
    }

    #[test]
    fn test_handle_change_admin() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("bob".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let handle_msg = HandleMsg::ChangeAdmin {
            address: HumanAddr("bob".to_string()),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("admin", &[]), handle_msg);
        assert!(
            handle_result.is_ok(),
            "handle() failed: {}",
            handle_result.err().unwrap()
        );

        let admin = ReadonlyConfig::from_storage(&deps.storage)
            .constants()
            .unwrap()
            .admin;
        assert_eq!(admin, HumanAddr("bob".to_string()));
    }

    #[test]
    fn test_handle_set_contract_status() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("admin".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let handle_msg = HandleMsg::SetContractStatus {
            level: ContractStatusLevel::StopAll,
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("admin", &[]), handle_msg);
        assert!(
            handle_result.is_ok(),
            "handle() failed: {}",
            handle_result.err().unwrap()
        );

        let contract_status = ReadonlyConfig::from_storage(&deps.storage).contract_status();
        assert!(matches!(contract_status, ContractStatusLevel::StopAll{..}));
    }

    #[test]
    fn test_handle_admin_commands() {
        let admin_err = "Admin commands can only be run from admin address".to_string();

        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("lebron".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let pause_msg = HandleMsg::SetContractStatus {
            level: ContractStatusLevel::StopAllButRedeems,
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("not_admin", &[]), pause_msg);
        let error = extract_error_msg(handle_result);
        assert!(error.contains(&admin_err.clone()));

        let change_admin_msg = HandleMsg::ChangeAdmin {
            address: HumanAddr("not_admin".to_string()),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("not_admin", &[]), change_admin_msg);
        let error = extract_error_msg(handle_result);
        assert!(error.contains(&admin_err.clone()));
    }

    #[test]
    fn test_handle_pause_with_withdrawals() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("lebron".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let pause_msg = HandleMsg::SetContractStatus {
            level: ContractStatusLevel::StopAllButRedeems,
            padding: None,
        };

        let handle_result = handle(&mut deps, mock_env("admin", &[]), pause_msg);
        assert!(
            handle_result.is_ok(),
            "Pause handle failed: {}",
            handle_result.err().unwrap()
        );

        let send_msg = HandleMsg::Transfer {
            recipient: HumanAddr("account".to_string()),
            amount: Uint128(123),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("admin", &[]), send_msg);
        let error = extract_error_msg(handle_result);
        assert_eq!(
            error,
            "This contract is stopped and this action is not allowed".to_string()
        );

        let withdraw_msg = HandleMsg::Redeem {
            amount: Uint128(5000),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("lebron", &[]), withdraw_msg);
        assert!(
            handle_result.is_ok(),
            "Withdraw failed: {}",
            handle_result.err().unwrap()
        );
    }

    #[test]
    fn test_handle_pause_all() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("lebron".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let pause_msg = HandleMsg::SetContractStatus {
            level: ContractStatusLevel::StopAll,
            padding: None,
        };

        let handle_result = handle(&mut deps, mock_env("admin", &[]), pause_msg);
        assert!(
            handle_result.is_ok(),
            "Pause handle failed: {}",
            handle_result.err().unwrap()
        );

        let send_msg = HandleMsg::Transfer {
            recipient: HumanAddr("account".to_string()),
            amount: Uint128(123),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("admin", &[]), send_msg);
        let error = extract_error_msg(handle_result);
        assert_eq!(
            error,
            "This contract is stopped and this action is not allowed".to_string()
        );

        let withdraw_msg = HandleMsg::Redeem {
            amount: Uint128(5000),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("lebron", &[]), withdraw_msg);
        let error = extract_error_msg(handle_result);
        assert_eq!(
            error,
            "This contract is stopped and this action is not allowed".to_string()
        );
    }

    // Query tests

    #[test]
    fn test_authenticated_queries() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("giannis".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let no_vk_yet_query_msg = QueryMsg::Balance {
            address: HumanAddr("giannis".to_string()),
            key: "no_vk_yet".to_string(),
        };
        let query_result = query(&deps, no_vk_yet_query_msg);
        let error = extract_error_msg(query_result);
        assert_eq!(
            error,
            "Wrong viewing key for this address or viewing key not set".to_string()
        );

        let create_vk_msg = HandleMsg::CreateViewingKey {
            entropy: "34".to_string(),
            padding: None,
        };
        let handle_response = handle(&mut deps, mock_env("giannis", &[]), create_vk_msg).unwrap();
        let vk = match from_binary(&handle_response.data.unwrap()).unwrap() {
            HandleAnswer::CreateViewingKey { key } => key,
            _ => panic!("Unexpected result from handle"),
        };

        let query_balance_msg = QueryMsg::Balance {
            address: HumanAddr("giannis".to_string()),
            key: vk.0,
        };

        let query_response = query(&deps, query_balance_msg).unwrap();
        let balance = match from_binary(&query_response).unwrap() {
            QueryAnswer::Balance { amount } => amount,
            _ => panic!("Unexpected result from query"),
        };
        assert_eq!(balance, Uint128(5000));

        let wrong_vk_query_msg = QueryMsg::Balance {
            address: HumanAddr("giannis".to_string()),
            key: "wrong_vk".to_string(),
        };
        let query_result = query(&deps, wrong_vk_query_msg);
        let error = extract_error_msg(query_result);
        assert_eq!(
            error,
            "Wrong viewing key for this address or viewing key not set".to_string()
        );
    }

    #[test]
    fn test_query_token_info() {
        let init_name = "sec-sec".to_string();
        let init_admin = HumanAddr("admin".to_string());
        let init_symbol = "SECSEC".to_string();
        let init_decimals = 8;
        let init_config: InitConfig = from_binary(&Binary::from(
            r#"{ "public_total_supply": true }"#.as_bytes(),
        ))
        .unwrap();
        let init_supply = Uint128(5000);

        let mut deps = mock_dependencies(20, &[]);
        let env = mock_env("instantiator", &[]);
        let init_msg = InitMsg {
            name: init_name.clone(),
            admin: Some(vec![init_admin.clone()]),
            symbol: init_symbol.clone(),
            decimals: init_decimals.clone(),
            initial_balances: Some(vec![InitialBalance {
                address: HumanAddr("giannis".to_string()),
                amount: init_supply,
            }]),
            prng_seed: Binary::from("lolz fun yay".as_bytes()),
            config: Some(init_config),
            init_hook: None,
            token_code_id: None,
            is_being_minted: None,
        };
        let init_result = init(&mut deps, env, init_msg);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let query_msg = QueryMsg::TokenInfo {};
        let query_result = query(&deps, query_msg);
        assert!(
            query_result.is_ok(),
            "Init failed: {}",
            query_result.err().unwrap()
        );
        let query_answer: QueryAnswer = from_binary(&query_result.unwrap()).unwrap();
        match query_answer {
            QueryAnswer::TokenInfo {
                name,
                symbol,
                decimals,
                total_supply,
            } => {
                assert_eq!(name, init_name);
                assert_eq!(symbol, init_symbol);
                assert_eq!(decimals, init_decimals);
                assert_eq!(total_supply, Some(Uint128(5000)));
            }
            _ => panic!("unexpected"),
        }
    }

    #[test]
    fn test_query_allowance() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("giannis".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let handle_msg = HandleMsg::IncreaseAllowance {
            spender: HumanAddr("lebron".to_string()),
            amount: Uint128(2000),
            padding: None,
            expiration: None,
        };
        let handle_result = handle(&mut deps, mock_env("giannis", &[]), handle_msg);
        assert!(
            handle_result.is_ok(),
            "handle() failed: {}",
            handle_result.err().unwrap()
        );

        let vk1 = ViewingKey("key1".to_string());
        let vk2 = ViewingKey("key2".to_string());

        let query_msg = QueryMsg::Allowance {
            owner: HumanAddr("giannis".to_string()),
            spender: HumanAddr("lebron".to_string()),
            key: vk1.0.clone(),
            padding: None,
        };
        let query_result = query(&deps, query_msg);
        assert!(
            query_result.is_ok(),
            "Query failed: {}",
            query_result.err().unwrap()
        );
        let error = extract_error_msg(query_result);
        assert!(error.contains("Wrong viewing key"));

        let handle_msg = HandleMsg::SetViewingKey {
            key: vk1.0.clone(),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("lebron", &[]), handle_msg);
        let unwrapped_result: HandleAnswer =
            from_binary(&handle_result.unwrap().data.unwrap()).unwrap();
        assert_eq!(
            to_binary(&unwrapped_result).unwrap(),
            to_binary(&HandleAnswer::SetViewingKey {
                status: ResponseStatus::Success
            })
            .unwrap(),
        );

        let handle_msg = HandleMsg::SetViewingKey {
            key: vk2.0.clone(),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("giannis", &[]), handle_msg);
        let unwrapped_result: HandleAnswer =
            from_binary(&handle_result.unwrap().data.unwrap()).unwrap();
        assert_eq!(
            to_binary(&unwrapped_result).unwrap(),
            to_binary(&HandleAnswer::SetViewingKey {
                status: ResponseStatus::Success
            })
            .unwrap(),
        );

        let query_msg = QueryMsg::Allowance {
            owner: HumanAddr("giannis".to_string()),
            spender: HumanAddr("lebron".to_string()),
            key: vk1.0.clone(),
            padding: None,
        };
        let query_result = query(&deps, query_msg);
        let allowance = match from_binary(&query_result.unwrap()).unwrap() {
            QueryAnswer::Allowance { allowance, .. } => allowance,
            _ => panic!("Unexpected"),
        };
        assert_eq!(allowance, Uint128(2000));

        let query_msg = QueryMsg::Allowance {
            owner: HumanAddr("giannis".to_string()),
            spender: HumanAddr("lebron".to_string()),
            key: vk2.0.clone(),
            padding: None,
        };
        let query_result = query(&deps, query_msg);
        let allowance = match from_binary(&query_result.unwrap()).unwrap() {
            QueryAnswer::Allowance { allowance, .. } => allowance,
            _ => panic!("Unexpected"),
        };
        assert_eq!(allowance, Uint128(2000));

        let query_msg = QueryMsg::Allowance {
            owner: HumanAddr("lebron".to_string()),
            spender: HumanAddr("giannis".to_string()),
            key: vk2.0.clone(),
            padding: None,
        };
        let query_result = query(&deps, query_msg);
        let allowance = match from_binary(&query_result.unwrap()).unwrap() {
            QueryAnswer::Allowance { allowance, .. } => allowance,
            _ => panic!("Unexpected"),
        };
        assert_eq!(allowance, Uint128(0));
    }

    #[test]
    fn test_query_balance() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("bob".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let handle_msg = HandleMsg::SetViewingKey {
            key: "key".to_string(),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        let unwrapped_result: HandleAnswer =
            from_binary(&handle_result.unwrap().data.unwrap()).unwrap();
        assert_eq!(
            to_binary(&unwrapped_result).unwrap(),
            to_binary(&HandleAnswer::SetViewingKey {
                status: ResponseStatus::Success
            })
            .unwrap(),
        );

        let query_msg = QueryMsg::Balance {
            address: HumanAddr("bob".to_string()),
            key: "wrong_key".to_string(),
        };
        let query_result = query(&deps, query_msg);
        let error = extract_error_msg(query_result);
        assert!(error.contains("Wrong viewing key"));

        let query_msg = QueryMsg::Balance {
            address: HumanAddr("bob".to_string()),
            key: "key".to_string(),
        };
        let query_result = query(&deps, query_msg);
        let balance = match from_binary(&query_result.unwrap()).unwrap() {
            QueryAnswer::Balance { amount } => amount,
            _ => panic!("Unexpected"),
        };
        assert_eq!(balance, Uint128(5000));
    }

    #[test]
    fn test_query_transfer_history() {
        let (init_result, mut deps) = init_helper(vec![InitialBalance {
            address: HumanAddr("bob".to_string()),
            amount: Uint128(5000),
        }]);
        assert!(
            init_result.is_ok(),
            "Init failed: {}",
            init_result.err().unwrap()
        );

        let handle_msg = HandleMsg::SetViewingKey {
            key: "key".to_string(),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        assert!(ensure_success(handle_result.unwrap()));

        let handle_msg = HandleMsg::Transfer {
            recipient: HumanAddr("alice".to_string()),
            amount: Uint128(1000),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        let result = handle_result.unwrap();
        assert!(ensure_success(result));
        let handle_msg = HandleMsg::Transfer {
            recipient: HumanAddr("banana".to_string()),
            amount: Uint128(500),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        let result = handle_result.unwrap();
        assert!(ensure_success(result));
        let handle_msg = HandleMsg::Transfer {
            recipient: HumanAddr("mango".to_string()),
            amount: Uint128(2500),
            padding: None,
        };
        let handle_result = handle(&mut deps, mock_env("bob", &[]), handle_msg);
        let result = handle_result.unwrap();
        assert!(ensure_success(result));

        let query_msg = QueryMsg::TransferHistory {
            address: HumanAddr("bob".to_string()),
            key: "key".to_string(),
            page: None,
            page_size: 0,
        };
        let query_result = query(&deps, query_msg);
        // let a: QueryAnswer = from_binary(&query_result.unwrap()).unwrap();
        // println!("{:?}", a);
        let transfers = match from_binary(&query_result.unwrap()).unwrap() {
            QueryAnswer::TransferHistory { txs } => txs,
            _ => panic!("Unexpected"),
        };
        assert!(transfers.is_empty());

        let query_msg = QueryMsg::TransferHistory {
            address: HumanAddr("bob".to_string()),
            key: "key".to_string(),
            page: None,
            page_size: 10,
        };
        let query_result = query(&deps, query_msg);
        let transfers = match from_binary(&query_result.unwrap()).unwrap() {
            QueryAnswer::TransferHistory { txs } => txs,
            _ => panic!("Unexpected"),
        };
        assert_eq!(transfers.len(), 3);

        let query_msg = QueryMsg::TransferHistory {
            address: HumanAddr("bob".to_string()),
            key: "key".to_string(),
            page: None,
            page_size: 2,
        };
        let query_result = query(&deps, query_msg);
        let transfers = match from_binary(&query_result.unwrap()).unwrap() {
            QueryAnswer::TransferHistory { txs } => txs,
            _ => panic!("Unexpected"),
        };
        assert_eq!(transfers.len(), 2);
    }
}
