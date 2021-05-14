use cosmwasm_std::{
    to_binary, Api, Binary, Env, Extern, HandleResponse, InitResponse, Querier, StdResult, Storage,
};

use crate::admin::admin_commands;
use crate::msg::{HandleMsg, InitMsg, QueryAnswer, QueryMsg};
use crate::state::{set_config, Config};
use crate::voting::{change_votes, get_active_proposals, try_vote};

pub const PREFIX_CONFIG: &[u8] = b"config";
pub const PREFIX_BALANCES: &[u8] = b"balances";
pub const PREFIX_ALLOWANCES: &[u8] = b"allowances";

pub const KEY_CONSTANTS: &[u8] = b"constants";

// -- 21 days + 2 minutes (buffer to make sure unbond will be matured)
//const UNBONDING_TIME: u64 = 3600 * 24 * 21 + 120;
const UNBONDING_TIME: u64 = 15;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    // ensure the validator is registered

    let config = Config {
        admin: env.message.sender.clone(),
        staking_contract: msg.staking_contract.clone(),
        staking_contract_hash: msg.staking_contract_hash.clone(),
        gov_token: msg.gov_token.clone(),
        gov_token_hash: msg.gov_token_hash.clone(),
        voting_time: UNBONDING_TIME,
        viewing_key: "yo".to_string(),
    };

    set_config(&mut deps.storage, &config);

    Ok(InitResponse::default())
}

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::Vote { proposal, vote } => try_vote(deps, env, proposal, vote),
        HandleMsg::NotifyBalanceChange { changes } => change_votes(deps, env, changes),
        _ => admin_commands(deps, env, msg),
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::ActiveProposals => query_active_proposals(deps),
        QueryMsg::VoteState { proposal } => query_proposal_state(deps, proposal),
    }
}

pub fn query_proposal_state<S: Storage, A: Api, Q: Querier>(
    _deps: &Extern<S, A, Q>,
    proposal: u64,
) -> StdResult<Binary> {
    // todo: this

    Ok(to_binary(&QueryAnswer::VoteState {
        proposal,
        yes: 0,
        no: 0,
        no_with_veto: 0,
        abstain: 0,
        end_time: 0,
        active: 0,
        result: None,
    })?)
}

pub fn query_active_proposals<S: Storage, A: Api, Q: Querier>(
    _deps: &Extern<S, A, Q>,
) -> StdResult<Binary> {
    Ok(to_binary(&QueryAnswer::ActiveProposals {
        proposals: get_active_proposals(),
    })?)
}

// pub fn migrate<S: Storage, A: Api, Q: Querier>(
//     _deps: &mut Extern<S, A, Q>,
//     _env: Env,
//     _msg: MigrateMsg,
// ) -> StdResult<MigrateResponse> {
//     Ok(MigrateResponse::default())
// }
