use cosmwasm_std::{
    to_binary, Api, Binary, Env, Extern, HandleResponse, InitResponse, Querier, StdError,
    StdResult, Storage, Uint128,
};

use crate::admin::{admin_commands, SNAPSHOTS};
use crate::msg::{HandleMsg, InitMsg, QueryAnswer, QueryMsg};
use crate::state::{get_active_proposals, get_inactive_proposals, set_config, Config, VoteTotals};
use crate::voting::{
    active_proposals, change_votes, get_proposal, query_vote, set_password, try_vote,
};
use cargo_common::cashmap::ReadOnlyCashMap;

pub const PREFIX_CONFIG: &[u8] = b"config";
pub const PREFIX_BALANCES: &[u8] = b"balances";
pub const PREFIX_ALLOWANCES: &[u8] = b"allowances";

pub const KEY_CONSTANTS: &[u8] = b"constants";

// -- 21 days + 2 minutes (buffer to make sure unbond will be matured)
//const UNBONDING_TIME: u64 = 3600 * 24 * 7 - 120;
// End the voting 1 hour before the real vote should end
const VOTING_TIME: u64 = 3600 * 24 * 7 - 3600 * 12;

pub fn init<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: InitMsg,
) -> StdResult<InitResponse> {
    // ensure the validator is registered

    let config = Config {
        admin: env.message.sender,
        staking_contract: msg.staking_contract,
        staking_contract_hash: msg.staking_contract_hash,
        gov_token: msg.gov_token,
        gov_token_hash: msg.gov_token_hash,
        voting_time: VOTING_TIME,
        password: None,
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
        HandleMsg::SetPassword { password } => set_password(deps, env, password),
        HandleMsg::NotifyBalanceChange { changes } => change_votes(deps, env, changes),
        _ => admin_commands(deps, env, msg),
    }
}

pub fn query<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    msg: QueryMsg,
) -> StdResult<Binary> {
    match msg {
        QueryMsg::Proposals {} => query_active_proposals(deps),
        QueryMsg::ExpiredProposals {} => query_inactive_proposals(deps),
        QueryMsg::VoteState { proposal } => query_proposal_state(deps, proposal),
        QueryMsg::QueryVote {
            address,
            proposal,
            password,
        } => query_vote(deps, proposal, address, password),
    }
}

pub fn query_proposal_state<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    proposal: u64,
) -> StdResult<Binary> {
    let cashmap = ReadOnlyCashMap::init(SNAPSHOTS, &deps.storage);

    let option_totals: Option<VoteTotals> = cashmap.get(&proposal.to_be_bytes());

    let is_active = active_proposals(&deps.storage).contains(&proposal);
    let end_time = if is_active {
        get_proposal(&deps.storage, &proposal).unwrap().end_time
    } else {
        0
    };

    if let Some(totals) = option_totals {
        Ok(to_binary(&QueryAnswer::VoteState {
            proposal,
            yes: Uint128(totals.yes),
            no: Uint128(totals.no),
            no_with_veto: Uint128(totals.no_with_veto),
            abstain: Uint128(totals.abstain),
            end_time,
            active: is_active,
            result: Some(totals.winner()),
        })?)
    } else {
        Err(StdError::generic_err("No snapshots for this proposal"))
    }
}

pub fn query_active_proposals<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<Binary> {
    Ok(to_binary(&QueryAnswer::Proposals {
        proposals: get_active_proposals(&deps.storage).proposals,
    })?)
}

pub fn query_inactive_proposals<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
) -> StdResult<Binary> {
    Ok(to_binary(&QueryAnswer::ExpiredProposals {
        proposals: get_inactive_proposals(&deps.storage).proposals,
    })?)
}
