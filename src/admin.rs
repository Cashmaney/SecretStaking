use cosmwasm_std::{
    generic_err, log, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse, Querier,
    StdResult, Storage, Uint128,
};

use crate::liquidity_pool::{
    current_staked_ratio, liquidity_pool_from_chain, update_exchange_rate,
};
use crate::msg::HandleMsg;
use crate::staking::{
    get_bonded, get_locked_balance, get_unbonding, restake, stake, undelegate, withdraw_to_self,
};
use crate::state::{get_staked_ratio, get_validator_address, read_constants};

/// This file contains only permissioned functions
/// Can only be run by contract deployer or the contract itself
pub fn admin_commands<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    let msg_sender = deps.api.human_address(&env.message.sender)?;
    let admin = read_constants(&deps.storage)?.admin;
    let contract_addr = deps.api.human_address(&env.contract.address)?;
    if admin != msg_sender && contract_addr != msg_sender {
        return Err(generic_err(
            "Admin commands can only be ran from deployer address",
        ));
    }

    // authenticate admin
    match msg {
        // returns the total liquidity pool to check the health of the liquidity pool
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
        HandleMsg::Restake { amount } => {
            let validator = get_validator_address(&deps.storage)?;

            return Ok(HandleResponse {
                messages: restake(&validator, amount.u128()),
                log: vec![],
                data: None,
            });
        }
        // Try to rebalance liquidity pool by either staking extra or undelegating funds
        HandleMsg::UpdateDailyLiquidity {} => {
            let validator = get_validator_address(&deps.storage)?;
            let contract_addr = deps.api.human_address(&env.contract.address)?;

            let pool = liquidity_pool_from_chain(&deps.querier, &contract_addr)?.u128();

            let staked_ratio = current_staked_ratio(&deps.querier, &deps.storage, &contract_addr)?;
            let target_ratio = get_staked_ratio(&deps.storage)?;
            let bonded = get_bonded(&deps.querier, &contract_addr)?.u128();

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
                    from_address: contract_addr,
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
        _ => Err(generic_err(format!("Invalid message type"))),
    }
}

// pub fn handle_restake_rewards() {}
