use crate::types::config::read_config;
use cosmwasm_std::{
    log, Api, CosmosMsg, Env, Extern, GovMsg, HandleResponse, Querier, StdError, StdResult,
    Storage, VoteOption,
};

pub fn try_vote<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    proposal: u64,
    vote: VoteOption,
) -> StdResult<HandleResponse> {
    let config = read_config(&deps.storage)?;

    if env.message.sender != config.voting_admin {
        return Err(StdError::generic_err(
            "Voting can only be done from voting admin",
        ));
    }

    let messages = vec![CosmosMsg::Gov(GovMsg::Vote {
        proposal,
        vote_option: vote.clone(),
    })];

    let logs = vec![log("finalized", true), log("result", format!("{:?}", vote))];

    Ok(HandleResponse {
        messages,
        log: logs,
        data: None,
    })
}
