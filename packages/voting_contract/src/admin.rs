use cosmwasm_std::{
    log, Api, Env, Extern, HandleResponse, HandleResult, Querier, StdError, StdResult, Storage,
};

use crate::msg::HandleMsg;

use crate::state::{read_config, set_config};

use crate::voting::tally;

/// This file contains only permissioned functions
/// Can only be run by contract deployer or the contract itself
pub fn admin_commands<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    let mut config = read_config(&deps.storage)?;
    if config.admin != env.message.sender && env.contract.address != env.message.sender {
        return Err(StdError::generic_err(
            "Admin commands can only be ran from deployer address",
        ));
    }

    // authenticate admin
    match msg {
        // HandleMsg::NewProposal {
        //     start_time,
        //     proposal_id,
        // } => Ok(HandleResponse::default()),
        HandleMsg::ChangeVotingTime { new_time } => {
            config.voting_time = new_time;

            set_config(&mut deps.storage, &config);

            Ok(HandleResponse {
                messages: vec![],
                log: vec![log("new_time", format!("{:?}", new_time))],
                data: None,
            })
        }
        HandleMsg::SetStakingContract {
            staking_contract,
            staking_contract_hash,
        } => {
            config.staking_contract = staking_contract;
            if let Some(hash) = staking_contract_hash {
                config.staking_contract_hash = hash;
            }

            set_config(&mut deps.storage, &config);

            Ok(HandleResponse {
                messages: vec![],
                log: vec![log(
                    "staking_contract",
                    format!("{:?}", config.staking_contract),
                )],
                data: None,
            })
        }

        HandleMsg::SetGovToken {
            gov_token,
            gov_token_hash,
        } => {
            config.gov_token = gov_token;
            if let Some(hash) = gov_token_hash {
                config.gov_token_hash = hash;
            }

            set_config(&mut deps.storage, &config);

            Ok(HandleResponse {
                messages: vec![],
                log: vec![log("gov_token", format!("{:?}", config.gov_token))],
                data: None,
            })
        }

        HandleMsg::Tally { proposal, .. } => tally(deps, env, proposal),

        HandleMsg::ChangeOwner { new_owner } => {
            config.admin = new_owner;

            set_config(&mut deps.storage, &config);
            Ok(HandleResponse::default())
        }
        HandleMsg::CreateSnapshot { proposal } => create_snapshot(deps, env, proposal),
        HandleMsg::InitVote { proposal } => init_vote(deps, env, proposal),
        _ => Err(StdError::generic_err("Invalid message type".to_string())),
    }
}

pub fn init_vote<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    _proposal: u64,
) -> HandleResult {
    let config = read_config(&deps.storage)?;

    if env.message.sender != config.admin {
        return Err(StdError::unauthorized());
    }

    // todo: this

    Ok(HandleResponse::default())
}

pub fn create_snapshot<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    _proposal: u64,
) -> HandleResult {
    let config = read_config(&deps.storage)?;

    if env.message.sender != config.admin {
        return Err(StdError::unauthorized());
    }

    // todo: this

    Ok(HandleResponse::default())
}

// pub fn handle_restake_rewards() {}
