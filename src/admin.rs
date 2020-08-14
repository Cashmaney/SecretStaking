use cosmwasm_std::{
    generic_err, log, Api, BankMsg, Coin, CosmosMsg, Env, Extern, HandleResponse, Querier,
    StdResult, Storage,
};

use crate::balance::refresh_balances;
use crate::msg::HandleMsg;
use crate::staking::{restake, withdraw_to_self};
use crate::state::{get_validator_address, read_constants};

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
            let liquidity_pool = crate::state::get_total_balance(&deps.storage);
            let tokens = crate::state::get_total_tokens(&deps.storage);
            let ratio = crate::state::get_ratio(&deps.storage)?;
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
        HandleMsg::UpdateBalances {} => refresh_balances(deps, env),
        HandleMsg::Restake { amount } => {
            let validator = get_validator_address(&deps.storage)?;

            return Ok(HandleResponse {
                messages: restake(&validator, amount.u128()),
                log: vec![],
                data: None,
            });
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
