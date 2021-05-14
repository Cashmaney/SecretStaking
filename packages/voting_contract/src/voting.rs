use crate::state::{read_config, SingleVote, VoteTotals, Votes};
use cosmwasm_std::{
    log, Api, CosmosMsg, Env, Extern, HandleResponse, HandleResult, Querier, StdResult, Storage,
    WasmMsg,
};

use cosmwasm_std::{to_binary, StdError};

use crate::msg::VoteChange;
use cargo_common::voting::{u32_to_vote_option, VotingMessages};

pub fn change_votes<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    changes: Vec<VoteChange>,
) -> HandleResult {
    let config = read_config(&deps.storage)?;

    if env.message.sender != config.gov_token {
        return Err(StdError::unauthorized());
    }

    let active_proposals = get_active_proposals();

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

pub fn get_active_proposals() -> Vec<u64> {
    return vec![1u64];
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

    let active_proposals = get_active_proposals();

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

    // todo: check voting time/if proposal is active etc.

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

    let logs = vec![
        log("finalized", true),
        log("result", format!("{:?}", winner)),
    ];

    Ok(HandleResponse {
        messages,
        log: logs,
        data: None,
    })
}

// pub fn query_balances<Q: Querier>(
//     querier: &Q,
//     token_contract: &HumanAddr,
//     token_contract_hash: &str,
//     address: &HumanAddr,
//     key: &str,
//     voters: Vec<HumanAddr>,
// ) -> StdResult<Balances> {
//     let query = QueryRequest::Wasm(WasmQuery::Smart {
//         contract_addr: token_contract.clone(),
//         callback_code_hash: token_contract_hash.to_string(),
//         msg: to_binary(&TokenQuery::MultipleBalances {
//             address: address.clone(),
//             key: key.to_string(),
//             addresses: voters,
//         })?,
//     });
//
//     if let TokenQueryResponse::MultipleBalances { balances } = querier.query(&query)? {
//         let deserialized = Balances::try_from(balances)?;
//         Ok(deserialized)
//     } else {
//         Err(StdError::generic_err("Failed to get balances"))
//     }
// }
