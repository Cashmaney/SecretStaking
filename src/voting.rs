use crate::state::{read_config, vote_option_to_u32, SingleVote, Votes};
use crate::tokens::query_balances;
use cosmwasm_std::{
    debug_print, log, Api, CosmosMsg, Env, Extern, GovMsg, HandleResponse, Querier, StdResult,
    Storage, VoteOption,
};

pub fn try_vote<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    proposal: u64,
    vote: VoteOption,
) -> StdResult<HandleResponse> {
    Votes::set(
        &mut deps.storage,
        proposal,
        SingleVote {
            address: env.message.sender,
            vote: vote_option_to_u32(vote),
        },
    )?;

    Ok(HandleResponse::default())
}

pub fn tally<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    proposal: u64,
    page: u32,
    page_size: u32,
) -> StdResult<HandleResponse> {
    let voters = Votes::get_voters(&deps.storage, proposal, page, page_size)?;

    debug_print(format!("Querying for {:?} voters", voters.len()));

    let config = read_config(&deps.storage)?;

    // load balances from token
    let balances = query_balances(
        &deps.querier,
        &config.gov_token,
        &config.gov_token_hash,
        &env.contract.address,
        &config.viewing_key,
        voters,
    )?;

    debug_print(format!("Got balances for {:?} voters", balances.0.len()));

    let winner = Votes::tally(&mut deps.storage, proposal, &balances)?;

    let logs;
    let mut messages = vec![];

    if let Some(winning_vote) = winner {
        messages = vec![CosmosMsg::Gov(GovMsg::Vote {
            proposal,
            vote_option: winning_vote.clone(),
        })];

        logs = vec![
            log("finalized", true),
            log("result", format!("{:?}", winning_vote)),
        ]
    } else {
        logs = vec![log("finalized", false)]
    }

    Ok(HandleResponse {
        messages,
        log: logs,
        data: None,
    })
}
