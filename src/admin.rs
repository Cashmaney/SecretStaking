use cosmwasm_std::{
    debug_print, log, to_binary, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse,
    HumanAddr, Querier, StdError, StdResult, Storage, WasmMsg,
};

use crate::claim::claim_multiple;
use crate::msg::HandleMsg;

use crate::state::store_frozen_exchange_rate;

use crate::staking::{exchange_rate, redelegate_msg};
use crate::types::config::{read_config, set_config};
use crate::types::killswitch::KillSwitch;
use crate::types::validator_set::{get_validator_set, set_validator_set, DEFAULT_WEIGHT};

use cargo_common::tokens::TokenHandleMessage;

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
        HandleMsg::ClaimMaturedWithdraws { amount } => claim_multiple(deps, &env, amount),

        HandleMsg::ChangeUnbondingTime { new_time } => {
            config.unbonding_time = new_time;

            set_config(&mut deps.storage, &config);

            Ok(HandleResponse {
                messages: vec![],
                log: vec![log("new_time", format!("{:?}", new_time))],
                data: None,
            })
        }

        HandleMsg::ChangeDevFee {
            dev_fee,
            dev_address,
        } => {
            if let Some(dev_fee) = dev_fee {
                config.dev_fee = dev_fee;
            }

            if let Some(dev_address) = dev_address {
                config.dev_address = dev_address;
            }

            set_config(&mut deps.storage, &config);

            Ok(HandleResponse {
                messages: vec![],
                log: vec![
                    log("dev_fee", format!("{:?}", config.dev_fee)),
                    log("dev_address", format!("{:?}", config.dev_address)),
                ],
                data: None,
            })
        }

        HandleMsg::SetVotingContract {
            voting_admin,
            voting_contract,
            gov_token,
        } => {
            let mut messages = vec![];
            if let Some(admin) = voting_admin {
                config.voting_admin = admin;
            } else if let Some(contract) = voting_contract {
                config.voting_admin = contract.address.clone();

                messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: config.token_contract.clone(),
                    callback_code_hash: config.token_contract_hash.clone(),
                    msg: to_binary(&TokenHandleMessage::SetVotingContract {
                        contract,
                        gov_token: gov_token.unwrap_or_default(),
                    })?,
                    send: vec![],
                }))
            }

            set_config(&mut deps.storage, &config);

            Ok(HandleResponse {
                messages,
                log: vec![log("gov_token", format!("{:?}", config.voting_admin))],
                data: None,
            })
        }

        HandleMsg::RemoveValidator {
            address,
            redelegate,
        } => {
            let mut validator_set = get_validator_set(&deps.storage)?;

            let mut messages: Vec<CosmosMsg> = vec![];

            let redelegate_flag = redelegate.unwrap_or(true);

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

        HandleMsg::AddValidator { address, weight } => {
            let vals = deps.querier.query_validators()?;
            let human_addr_wrap = HumanAddr(address.clone());

            if !vals.iter().any(|v| v.address == human_addr_wrap) {
                return Err(StdError::generic_err(format!(
                    "{} is not in the current validator set",
                    address
                )));
            }

            let mut validator_set = get_validator_set(&deps.storage)?;

            validator_set.add(address, weight);

            set_validator_set(&mut deps.storage, &validator_set)?;

            Ok(HandleResponse::default())
        }

        HandleMsg::Redelegate { from, to } => {
            let mut validator_set = get_validator_set(&deps.storage)?;
            let mut messages: Vec<CosmosMsg> = vec![];
            let mut weight: u8 = DEFAULT_WEIGHT;
            let removed = validator_set.remove(&from, true)?;

            if let Some(validator) = removed {
                let to_stake = validator.staked;
                weight = validator.weight;
                validator_set.stake_at(&to, to_stake)?;

                messages.push(redelegate_msg(&from, &to, to_stake));
            }

            validator_set.add(from, Some(weight));

            set_validator_set(&mut deps.storage, &validator_set)?;

            Ok(HandleResponse {
                messages,
                log: vec![],
                data: None,
            })
        }
        HandleMsg::KillSwitchUnbond {} => {
            let frozen_exchange_rate = exchange_rate(&deps.storage, &deps.querier)?;
            debug_print(format!("Frozen exchange rate at: {}", frozen_exchange_rate));
            config.kill_switch = KillSwitch::Unbonding.into();
            set_config(&mut deps.storage, &config);

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
            config.kill_switch = KillSwitch::Open.into();
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

        HandleMsg::RecoverScrt { amount, denom, to } => Ok(HandleResponse {
            messages: vec![CosmosMsg::Bank(BankMsg::Send {
                from_address: env.contract.address,
                to_address: to,
                amount: vec![Coin { denom, amount }],
            })],
            log: vec![],
            data: None,
        }),

        HandleMsg::ChangeOwner { new_owner } => {
            config.admin = new_owner;

            set_config(&mut deps.storage, &config);
            Ok(HandleResponse::default())
        }

        HandleMsg::ChangeWeight { address, weight } => {
            let mut validator_set = get_validator_set(&deps.storage)?;

            validator_set.change_weight(&address, weight)?;

            set_validator_set(&mut deps.storage, &validator_set)?;

            Ok(HandleResponse::default())
        }

        _ => Err(StdError::generic_err("Invalid message type".to_string())),
    }
}

// pub fn handle_restake_rewards() {}
