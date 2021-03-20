use cosmwasm_std::{
    log, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse, HumanAddr, Querier, StdError,
    StdResult, Storage,
};

use crate::claim::claim_all;
use crate::msg::HandleMsg;

use crate::state::{read_config, set_config, store_frozen_exchange_rate, KillSwitch};

use crate::staking::{exchange_rate, redelegate_msg};
use crate::validator_set::{get_validator_set, set_validator_set};
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
        // Send all matured unclaimed withdraws to their destination address
        HandleMsg::ClaimMaturedWithdraws {} => claim_all(deps, env),

        HandleMsg::ChangeUnbondingTime { new_time } => {
            config.unbonding_time = new_time;

            set_config(&mut deps.storage, &config);

            Ok(HandleResponse {
                messages: vec![],
                log: vec![log("new_time", format!("{:?}", new_time))],
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

        HandleMsg::Tally { proposal } => tally(deps, env, proposal),
        HandleMsg::AddValidator { address } => {
            let vals = deps.querier.query_validators()?;
            let human_addr_wrap = HumanAddr(address.clone());

            if !vals.iter().any(|v| v.address == human_addr_wrap) {
                return Err(StdError::generic_err(format!(
                    "{} is not in the current validator set",
                    address
                )));
            }

            let mut validator_set = get_validator_set(&deps.storage)?;

            validator_set.add(address);

            set_validator_set(&mut deps.storage, &validator_set)?;

            Ok(HandleResponse::default())
        }

        HandleMsg::RemoveValidator {
            address,
            redelegate,
        } => {
            let mut validator_set = get_validator_set(&deps.storage)?;

            let mut messages: Vec<CosmosMsg> = vec![];

            let redelegate_flag = redelegate.unwrap_or_else(|| true);

            let removed = validator_set.remove(&address, redelegate_flag)?;

            if let Some(validator) = removed {
                let to_stake = validator.staked;
                let dest_validator = validator_set.stake(to_stake)?;

                if redelegate_flag {
                    messages.push(redelegate_msg(&address, &dest_validator, to_stake));
                }
            }
            set_validator_set(&mut deps.storage, &validator_set)?;

            Ok(HandleResponse {
                messages,
                log: vec![],
                data: None,
            })
        }

        HandleMsg::Redelegate { from, to } => {
            let mut validator_set = get_validator_set(&deps.storage)?;
            let mut messages: Vec<CosmosMsg> = vec![];

            let removed = validator_set.remove(&from, true)?;

            if let Some(validator) = removed {
                let to_stake = validator.staked;
                validator_set.stake_at(&to, to_stake)?;

                messages.push(redelegate_msg(&from, &to, to_stake));
            }

            validator_set.add(from);

            set_validator_set(&mut deps.storage, &validator_set)?;

            Ok(HandleResponse {
                messages,
                log: vec![],
                data: None,
            })
        }
        HandleMsg::KillSwitchUnbond {} => {
            config.kill_switch = KillSwitch::Unbonding;
            set_config(&mut deps.storage, &config);

            let frozen_exchange_rate = exchange_rate(&deps.storage, &deps.querier)?;

            store_frozen_exchange_rate(&mut deps.storage, &frozen_exchange_rate);

            let mut validator_set = get_validator_set(&deps.storage)?;

            let messages = validator_set.unbond_all();
            validator_set.zero();

            set_validator_set(&mut deps.storage, &validator_set)?;

            Ok(HandleResponse {
                messages,
                log: vec![],
                data: None,
            })
        }

        HandleMsg::KillSwitchOpenWithdraws {} => {
            config.kill_switch = KillSwitch::Open;
            set_config(&mut deps.storage, &config);
            Ok(HandleResponse::default())
        }

        HandleMsg::RecoverToken {
            token,
            token_hash,
            amount,
            to,
            snip20_send_msg,
        } => Ok(HandleResponse {
            messages: vec![secret_toolkit::snip20::send_msg(
                to,
                amount,
                snip20_send_msg,
                None,
                256,
                token_hash,
                token,
            )?],
            log: vec![],
            data: None,
        }),
        HandleMsg::RecoverScrt { amount, to } => Ok(HandleResponse {
            messages: vec![CosmosMsg::Bank(BankMsg::Send {
                from_address: env.contract.address,
                to_address: to,
                amount: vec![Coin {
                    denom: "uscrt".to_string(),
                    amount,
                }],
            })],
            log: vec![],
            data: None,
        }),
        HandleMsg::ChangeOwner { new_owner } => {
            config.admin = new_owner;

            set_config(&mut deps.storage, &config);
            Ok(HandleResponse::default())
        }

        _ => Err(StdError::generic_err("Invalid message type".to_string())),
    }
}

// pub fn handle_restake_rewards() {}
