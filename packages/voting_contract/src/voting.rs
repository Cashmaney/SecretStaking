use crate::state::{
    get_active_proposals, get_inactive_proposals, read_config, set_active_proposals, set_config,
    set_inactive_proposals, Proposal, SingleVote, VoteTotals, Votes,
};
use cosmwasm_std::{
    log, Api, CosmosMsg, Env, Extern, HandleResponse, HandleResult, HumanAddr, Querier,
    QueryResult, StdResult, Storage, Uint128, WasmMsg,
};

use cosmwasm_std::{to_binary, StdError};

use subtle::ConstantTimeEq;

use crate::admin::create_snapshot;
use crate::msg::{QueryAnswer, VoteChange};
use cargo_common::voting::{u32_to_vote_option, VotingMessages};

pub fn ct_slice_compare(s1: &[u8], s2: &[u8]) -> bool {
    bool::from(s1.ct_eq(s2))
}

pub fn change_votes<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    changes: Vec<VoteChange>,
) -> HandleResult {
    let config = read_config(&deps.storage)?;

    if env.message.sender != config.gov_token {
        return Err(StdError::unauthorized());
    }

    let active_proposals = active_proposals(&deps.storage);

    for proposal in active_proposals {
        for change in &changes {
            let old_vote = Votes::get(&deps.storage, proposal, &change.address);
            if let Some(vote) = old_vote {
                let new_vote = SingleVote {
                    address: vote.address,
                    vote: vote.vote,
                    voting_power: change.voting_power.clone(),
                };
                Votes::set(&mut deps.storage, proposal, new_vote)?;

                let mut totals = VoteTotals::load(&deps.storage, proposal);

                if vote.voting_power > change.voting_power {
                    totals.change(
                        u32_to_vote_option(vote.vote),
                        vote.voting_power.saturating_sub(change.voting_power) as u128,
                        false,
                    );
                } else {
                    totals.change(
                        u32_to_vote_option(vote.vote),
                        change.voting_power.saturating_sub(vote.voting_power) as u128,
                        true,
                    );
                }

                totals.store(&mut deps.storage, proposal)?;
            }
        }
    }

    Ok(HandleResponse::default())
}

pub fn disable_proposal<S: Storage>(storage: &mut S, proposal: &u64) -> StdResult<()> {
    let mut active = get_active_proposals(storage);
    let pos = active
        .proposals
        .iter()
        .position(|p| &p.proposal_id == proposal);

    if let Some(to_remove) = pos {
        let mut inactive = get_inactive_proposals(storage);

        let removed = active.proposals.remove(to_remove);
        inactive.proposals.push(removed);

        set_inactive_proposals(storage, &inactive);
        set_active_proposals(storage, &active);
    } else {
        return Err(StdError::generic_err(
            "Failed to remove proposal - not found in active list somehow?",
        ));
    }

    Ok(())
}

pub fn get_proposal<S: Storage>(storage: &S, proposal: &u64) -> Option<Proposal> {
    let active = get_active_proposals(storage);
    active
        .proposals
        .into_iter()
        .find(|prop| proposal == &prop.proposal_id)
}

pub fn active_proposals<S: Storage>(storage: &S) -> Vec<u64> {
    let active = get_active_proposals(storage);
    active
        .proposals
        .iter()
        .map(|prop| prop.proposal_id)
        .collect()
}

pub fn inactive_proposals<S: Storage>(storage: &S) -> Vec<u64> {
    let active = get_inactive_proposals(storage);
    active
        .proposals
        .iter()
        .map(|prop| prop.proposal_id)
        .collect()
}

pub fn set_password<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    password: String,
) -> StdResult<HandleResponse> {
    let mut config = read_config(&deps.storage)?;

    if env.message.sender != config.gov_token {
        return Err(StdError::unauthorized());
    }

    config.password = Some(password);

    set_config(&mut deps.storage, &config);

    Ok(HandleResponse::default())
}

pub fn query_vote<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    proposal: u64,
    address: HumanAddr,
    password: String,
) -> QueryResult {
    let config = read_config(&deps.storage)?;

    if config.password.is_none() {
        return Err(StdError::generic_err(
            "Password not set or voting contract not registered",
        ));
    }

    if !ct_slice_compare(password.as_bytes(), config.password.unwrap().as_bytes()) {
        return Err(StdError::unauthorized());
    }

    let old_vote = Votes::get(&deps.storage, proposal, &address);

    let (vote, voting_power) = if let Some(old) = old_vote {
        (Some(u32_to_vote_option(old.vote)), old.voting_power)
    } else {
        (None, 0)
    };

    return Ok(to_binary(&QueryAnswer::QueryVote {
        address,
        proposal,
        vote,
        voting_power: Uint128(voting_power as u128),
    })?);
}

pub fn try_vote<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    proposal: u64,
    vote: SingleVote,
) -> StdResult<HandleResponse> {
    let config = read_config(&deps.storage)?;

    if env.message.sender != config.gov_token {
        return Err(StdError::unauthorized());
    }

    let active_proposals = active_proposals(&deps.storage);

    if !active_proposals.contains(&proposal) {
        return Err(StdError::generic_err(format!(
            "Proposal {} is not active yet",
            proposal
        )));
    }

    let mut totals = VoteTotals::load(&deps.storage, proposal);
    let old_vote = Votes::get(&mut deps.storage, proposal, &vote.address);
    if let Some(old) = old_vote {
        totals.change(
            u32_to_vote_option(old.vote),
            old.voting_power as u128,
            false,
        );
    }

    totals.change(
        u32_to_vote_option(vote.vote),
        vote.voting_power as u128,
        true,
    );

    Votes::set(&mut deps.storage, proposal, vote)?;
    totals.store(&mut deps.storage, proposal)?;

    Ok(HandleResponse::default())
}

pub fn tally<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    proposal: u64,
) -> StdResult<HandleResponse> {
    let config = read_config(&deps.storage)?;

    if env.message.sender != config.admin {
        return Err(StdError::unauthorized());
    }

    let stored_proposal = get_proposal(&deps.storage, &proposal);

    if stored_proposal.is_none() {
        return Err(StdError::generic_err("Cannot tally an inactive proposal"));
    }

    let totals = VoteTotals::load(&deps.storage, proposal);

    let winner = totals.winner();

    let messages = vec![CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.staking_contract,
        callback_code_hash: config.staking_contract_hash,
        msg: to_binary(&VotingMessages::VoteOnChain {
            proposal,
            vote: winner.clone(),
        })?,
        send: vec![],
    })];

    let finalized = if stored_proposal.unwrap().end_time < env.block.time {
        disable_proposal(&mut deps.storage, &proposal)?;
        true
    } else {
        false
    };

    create_snapshot(deps, env, proposal)?;

    let logs = vec![
        log("finalized", finalized),
        log("result", format!("{:?}", winner)),
    ];

    Ok(HandleResponse {
        messages,
        log: logs,
        data: None,
    })
}
