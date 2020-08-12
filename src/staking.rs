use crate::contract::query;
use cosmwasm_std::{
    generic_err, log, to_binary, to_vec, Api, BankMsg, BankQuery, Binary, CanonicalAddr, Coin,
    CosmosMsg, Decimal, Env, Extern, HandleResponse, HumanAddr, InitResponse, MigrateResponse,
    Querier, QueryRequest, QueryResult, ReadonlyStorage, StakingMsg, StakingQuery, StdResult,
    Storage, Uint128,
};

pub fn get_onchain_balance<Q: Querier>(
    querier: &Q,
    contract_address: &HumanAddr,
) -> StdResult<u128> {
    let staked_balance = get_bonded(querier, contract_address)?;

    let bank_balance = get_bank_balance(querier, contract_address)?;

    Ok(staked_balance.u128() + bank_balance.u128())
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
            Err(generic_err(format!(
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
pub fn get_bank_balance<Q: Querier>(querier: &Q, contract: &HumanAddr) -> StdResult<Uint128> {
    let balances = querier.query_all_balances(contract)?;
    if balances.is_empty() {
        return Ok(Uint128(0));
    }
    let denom = balances[0].denom.as_str();
    balances.iter().fold(Ok(Uint128(0)), |racc, d| {
        let acc = racc?;
        if d.denom.as_str() != denom {
            Err(generic_err(format!(
                "different denoms in bonds: '{}' vs '{}'",
                denom, &d.denom
            )))
        } else {
            Ok(acc + d.amount)
        }
    })
}

pub fn stake(validator: &HumanAddr, amount: u128) -> CosmosMsg {
    CosmosMsg::Staking(StakingMsg::Delegate {
        validator: validator.clone(),
        amount: Coin {
            denom: "uscrt".to_string(),
            amount: Uint128(amount),
        },
    })
}

pub fn undelegate(validator: &HumanAddr, amount: u128) -> CosmosMsg {
    CosmosMsg::Staking(StakingMsg::Undelegate {
        validator: validator.clone(),
        amount: Coin {
            denom: "uscrt".to_string(),
            amount: Uint128(amount),
        },
    })
}