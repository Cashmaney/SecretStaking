use crate::state::{get_address, read_config};
use crate::tokens::query_total_supply;
use cosmwasm_std::{
    BondedRatioResponse, Coin, CosmosMsg, DistQuery, HumanAddr, InflationResponse, MintQuery,
    Querier, RewardsResponse, StakingMsg, StdError, StdResult, Storage, Uint128,
};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;

pub fn exchange_rate<S: Storage, Q: Querier>(store: &S, querier: &Q) -> StdResult<Decimal> {
    let contract_address = get_address(store)?;

    let config = read_config(store)?;

    let total_on_chain = get_total_onchain_balance(querier, &contract_address)?;
    let tokens =
        query_total_supply(querier, &config.token_contract, &config.token_contract_hash).u128();

    _calc_exchange_rate(total_on_chain, tokens)
}

fn _calc_exchange_rate(total_on_chain: u128, tokens: u128) -> Result<Decimal, StdError> {
    let scrt_balance = Decimal::from(total_on_chain as u64);
    let token_bal = Decimal::from(tokens as u64);

    let ratio = if total_on_chain == 0 {
        Decimal::one()
    } else {
        token_bal / scrt_balance
    };

    Ok(ratio.round_dp(12))
}

/// returns the yearly expected APR
pub fn interest_rate<Q: Querier>(querier: &Q) -> StdResult<u128> {
    let query = MintQuery::Inflation {};

    let resp: InflationResponse = querier.query(&query.into())?;

    let inflation = crate::utils::dec_to_uint(resp.inflation_rate)?;

    let query = MintQuery::BondedRatio {};

    let resp: BondedRatioResponse = querier.query(&query.into())?;

    let bonded_ratio = crate::utils::dec_to_uint(resp.bonded_ratio)?;

    Ok(inflation / bonded_ratio)
}

pub fn get_locked_balance<Q: Querier>(
    querier: &Q,
    contract_address: &HumanAddr,
) -> StdResult<u128> {
    let staked_balance = get_bonded(querier, contract_address)?;

    Ok(staked_balance.u128())
}

pub fn get_total_onchain_balance<Q: Querier>(
    querier: &Q,
    contract_address: &HumanAddr,
) -> StdResult<u128> {
    let locked_balance = get_locked_balance(querier, contract_address)?;
    let rewards_balance = get_rewards(querier, contract_address)?.u128();

    Ok(locked_balance + rewards_balance)
}

pub fn get_balance<Q: Querier>(querier: &Q, address: &HumanAddr) -> StdResult<Uint128> {
    let balance = querier.query_balance(address.clone(), &"uscrt".to_string())?;

    return Ok(balance.amount);
}

pub fn get_rewards<Q: Querier>(querier: &Q, contract: &HumanAddr) -> StdResult<Uint128> {
    let query = DistQuery::Rewards {
        delegator: contract.clone(),
    };

    let query_rewards: RewardsResponse =
        querier
            .query(&query.into())
            .unwrap_or_else(|_| RewardsResponse {
                rewards: vec![],
                total: vec![],
            });

    if query_rewards.total.is_empty() {
        return Ok(Uint128(0));
    }
    let denom = query_rewards.total[0].denom.as_str();
    query_rewards.total.iter().fold(Ok(Uint128(0)), |racc, d| {
        let acc = racc?;
        if d.denom.as_str() != denom {
            Err(StdError::generic_err(format!(
                "different denoms in bonds: '{}' vs '{}'",
                denom, &d.denom
            )))
        } else {
            Ok(acc + d.amount)
        }
    })
}

// get_bonded returns the total amount of delegations from contract
// it ensures they are all the same denom
// Simon I'm trusting you that this works don't let me down bro
pub fn get_bonded<Q: Querier>(querier: &Q, contract: &HumanAddr) -> StdResult<Uint128> {
    let bonds = querier.query_all_delegations(contract)?;
    if bonds.is_empty() {
        return Ok(Uint128(0));
    }
    let denom = bonds[0].amount.denom.as_str();
    bonds.iter().fold(Ok(Uint128(0)), |racc, d| {
        let acc = racc?;
        if d.amount.denom.as_str() != denom {
            Err(StdError::generic_err(format!(
                "different denoms in bonds: '{}' vs '{}'",
                denom, &d.amount.denom
            )))
        } else {
            Ok(acc + d.amount.amount)
        }
    })
}

// get_bonded returns the total amount of delegations from contract
// it ensures they are all the same denom
// Simon I'm trusting you that this works don't let me down bro
// pub fn get_unbonding<Q: Querier>(querier: &Q, contract: &HumanAddr) -> StdResult<Uint128> {
//     let query = StakingQuery::UnbondingDelegations {
//         delegator: contract.clone(),
//     };
//
//     let query_rewards: UnbondingDelegationsResponse = querier.query(&query.into())?;
//
//     let bonds = query_rewards.delegations;
//     if bonds.is_empty() {
//         return Ok(Uint128(0));
//     }
//     let denom = bonds[0].amount.denom.as_str();
//     bonds.iter().fold(Ok(Uint128(0)), |racc, d| {
//         let acc = racc?;
//         if d.amount.denom.as_str() != denom {
//             Err(StdError::generic_err(format!(
//                 "different denoms in bonds: '{}' vs '{}'",
//                 denom, &d.amount.denom
//             )))
//         } else {
//             Ok(acc + d.amount.amount)
//         }
//     })
// }

pub fn withdraw_to_self(validator: &String) -> CosmosMsg {
    CosmosMsg::Staking(StakingMsg::Withdraw {
        validator: HumanAddr(validator.clone()),
        recipient: None,
    })
}

// pub fn restake(validator: &String, amount: u128) -> Vec<CosmosMsg> {
//     vec![
//         CosmosMsg::Staking(StakingMsg::Withdraw {
//             validator: HumanAddr(validator.clone()),
//             recipient: None,
//         }),
//         CosmosMsg::Staking(StakingMsg::Delegate {
//             validator: HumanAddr(validator.clone()),
//             amount: Coin {
//                 denom: "uscrt".to_string(),
//                 amount: Uint128(amount),
//             },
//         }),
//     ]
// }

pub fn stake_msg(validator: &String, amount: u128) -> CosmosMsg {
    CosmosMsg::Staking(StakingMsg::Delegate {
        validator: HumanAddr(validator.clone()),
        amount: Coin {
            denom: "uscrt".to_string(),
            amount: Uint128(amount),
        },
    })
}

pub fn undelegate_msg(validator: &String, amount: u128) -> CosmosMsg {
    CosmosMsg::Staking(StakingMsg::Undelegate {
        validator: HumanAddr(validator.clone()),
        amount: Coin {
            denom: "uscrt".to_string(),
            amount: Uint128(amount),
        },
    })
}

pub fn redelegate_msg(from: &String, to: &String, amount: u128) -> CosmosMsg {
    CosmosMsg::Staking(StakingMsg::Redelegate {
        src_validator: HumanAddr(from.clone()),
        amount: Coin {
            denom: "uscrt".to_string(),
            amount: Uint128(amount),
        },
        dst_validator: HumanAddr(to.clone()),
    })
}
