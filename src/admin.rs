use cosmwasm_std::{
    log, Api, BankMsg, Binary, Coin, CosmosMsg, Env, Extern, HandleResponse, Querier, StdError,
    StdResult, Storage, Uint128, WasmMsg,
};

use crate::deposit::amount_to_stake_from_deposit;
use crate::liquidity_pool::{
    current_staked_ratio, liquidity_pool_from_chain, update_exchange_rate,
    update_exchange_rate_message,
};
use crate::msg::HandleMsg;
use crate::staking::{
    get_bonded, get_locked_balance, get_rewards, get_unbonding, restake, stake, undelegate,
    withdraw_to_self,
};
use crate::state::{get_staked_ratio, get_validator_address, read_constants};
use crate::validator_set::{get_validator_set, set_validator_set};

/// This file contains only permissioned functions
/// Can only be run by contract deployer or the contract itself
pub fn admin_commands<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    let admin = read_constants(&deps.storage)?.admin;
    let code_hash = &env.contract_code_hash;
    if admin != env.message.sender && env.contract.address != env.message.sender {
        return Err(StdError::generic_err(
            "Admin commands can only be ran from deployer address",
        ));
    }

    // authenticate admin
    match msg {
        // returns the total liquidity pool to check the health of the liquidity pool
        HandleMsg::RegisterReceive {
            address,
            token_contract_hash,
        } => {
            return Ok(HandleResponse {
                messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: address,
                    callback_code_hash: token_contract_hash,
                    msg: Binary(
                        format!(
                            r#"{{"register_receive": {{"code_hash":{}}}}}"#,
                            env.contract_code_hash
                        )
                        .as_bytes()
                        .to_vec(),
                    ),
                    send: vec![],
                })],
                log: vec![],
                data: None,
            });
        }
        HandleMsg::QueryBalances {} => {
            let liquidity_pool = crate::state::liquidity_pool_balance(&deps.storage);
            let tokens = crate::state::get_delegation_tokens(&deps.storage);
            let ratio = crate::state::get_exchange_rate(&deps.storage)?;
            return Ok(HandleResponse {
                messages: vec![],
                log: vec![
                    log("liquidity pool", format!("{:?} uscrt", liquidity_pool)),
                    log("tokens", format!("{:?} tokens", tokens)),
                    log("ratio", format!("{:?} scrt per token", ratio)),
                ],
                data: None,
            });
        }
        // withdraw more funds for the liquidity pool manually
        HandleMsg::WithdrawToLiquidityPool {} => {
            let validator = get_validator_address(&deps.storage)?;

            return Ok(HandleResponse {
                messages: vec![withdraw_to_self(&validator)],
                log: vec![],
                data: None,
            });
        }
        // Update balances
        HandleMsg::UpdateExchangeRate {} => update_exchange_rate(deps, env),
        // Distribute rewards back to liquidity pool or stake them, depending on liquidity ratio
        HandleMsg::HandleRewards {} => {
            let rewards_balance = get_rewards(&deps.querier, &env.contract.address)?;
            let amount = amount_to_stake_from_deposit(
                &deps.querier,
                &deps.storage,
                rewards_balance.u128(),
                &env.contract.address,
            )?;

            if amount == 0 {
                return Ok(HandleResponse::default());
            }

            let mut validator_set = get_validator_set(&deps.storage)?;

            let validator = validator_set.stake(amount as u64)?;
            validator_set.rebalance();
            set_validator_set(&mut deps.storage, &validator_set)?;

            let mut restake_msgs = restake(&validator, amount);
            restake_msgs.push(update_exchange_rate_message(
                &env.contract.address,
                &code_hash,
            ));

            return Ok(HandleResponse {
                messages: restake_msgs,
                log: vec![],
                data: None,
            });
        }
        // Try to rebalance liquidity pool by either staking extra or undelegating funds
        HandleMsg::UpdateDailyLiquidity {} => {
            let validator = get_validator_address(&deps.storage)?;

            let pool = liquidity_pool_from_chain(&deps.querier, &env.contract.address)?.u128();

            let staked_ratio =
                current_staked_ratio(&deps.querier, &deps.storage, &env.contract.address)?;
            let target_ratio = get_staked_ratio(&deps.storage)?;
            let bonded = get_bonded(&deps.querier, &env.contract.address)?.u128();

            return if staked_ratio > target_ratio {
                let amount_to_stake =
                    (pool - ((bonded + pool) / (target_ratio + u128::from(1 as u8)))) / 2;

                Ok(HandleResponse {
                    messages: vec![stake(&validator, amount_to_stake)],
                    log: vec![],
                    data: None,
                })
            } else {
                let mut amount_to_undelegate =
                    (((bonded + pool) / (target_ratio + u128::from(1 as u8))) - pool) * 2;

                if amount_to_undelegate > bonded {
                    amount_to_undelegate = bonded
                }

                Ok(HandleResponse {
                    messages: vec![undelegate(&validator, amount_to_undelegate)],
                    log: vec![],
                    data: None,
                })
            };
        }
        // Remove liquidity from the pool
        HandleMsg::WithdrawLiquidity { address, amount } => {
            return Ok(HandleResponse {
                messages: vec![CosmosMsg::Bank(BankMsg::Send {
                    from_address: env.contract.address,
                    to_address: address,
                    amount: vec![Coin {
                        denom: "uscrt".to_string(),
                        amount,
                    }],
                })],
                log: vec![],
                data: None,
            });
        }
        //todo
        //HandleMsg::UpdateValidatorWhitelist {} => notimplemented!(),
        _ => Err(StdError::generic_err(format!("Invalid message type"))),
    }
}

// pub fn handle_restake_rewards() {}
