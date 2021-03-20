use crate::state::{read_config, vote_option_to_u32, SingleVote, Votes};
use crate::tokens::query_balances;
use cosmwasm_std::{
    Api, CosmosMsg, Env, Extern, GovMsg, HandleResponse, Querier, StdResult, Storage, VoteOption,
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
) -> StdResult<HandleResponse> {
    let voters = Votes::get_voters(&deps.storage, proposal)?;

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

    let winner = Votes::tally(&deps.storage, proposal, &balances)?;

    let messages = vec![CosmosMsg::Gov(GovMsg::Vote {
        proposal,
        vote_option: winner,
    })];

    Ok(HandleResponse {
        messages,
        log: vec![],
        data: None,
    })
}
