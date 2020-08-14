// use crate::utils::ConstLenStr;
use bincode2;
use core::fmt;
use cosmwasm_std::{
    generic_err, log, Api, CanonicalAddr, Coin, Env, Extern, HandleResponse, HumanAddr, Querier,
    ReadonlyStorage, StdError, StdResult, Storage, Uint128,
};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};
use serde::export::Formatter;
use serde::{Deserialize, Serialize};
use std::path::Display;

use crate::state::{read_constants, read_u128, Tx, PREFIX_BALANCES, PREFIX_TXS};

pub fn try_transfer<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    recipient: &HumanAddr,
    amount: &Uint128,
) -> StdResult<HandleResponse> {
    let sender_address_raw = &env.message.sender;
    let recipient_address_raw = deps.api.canonical_address(recipient)?;
    let amount_raw = amount.u128();

    perform_transfer(
        &mut deps.storage,
        &sender_address_raw,
        &recipient_address_raw,
        amount_raw,
    )?;

    let symbol = read_constants(&deps.storage)?.symbol;

    store_transfer(
        &deps.api,
        &mut deps.storage,
        sender_address_raw,
        &recipient_address_raw,
        amount,
        symbol,
    );

    let res = HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "transfer"),
            log(
                "sender",
                deps.api.human_address(&env.message.sender)?.as_str(),
            ),
            log("recipient", recipient.as_str()),
        ],
        data: None,
    };
    Ok(res)
}

pub fn store_transfer<A: Api, S: Storage>(
    api: &A,
    storage: &mut S,
    from_address: &CanonicalAddr,
    to_address: &CanonicalAddr,
    amount: &Uint128,
    symbol: String,
) {
    let sender = api.human_address(from_address).unwrap();
    let receiver = api.human_address(to_address).unwrap();
    let coins = Coin {
        denom: symbol,
        amount: amount.clone(),
    };

    let tx = Tx {
        sender,
        receiver,
        coins,
    };

    let mut store = PrefixedStorage::new(PREFIX_TXS, storage);

    append_tx(&mut store, &tx, from_address);
    append_tx(&mut store, &tx, to_address);
}

fn append_tx<S: Storage>(store: &mut PrefixedStorage<S>, tx: &Tx, for_address: &CanonicalAddr) {
    let mut new_txs: Vec<Tx> = vec![];

    let txs = store.get(for_address.as_slice());

    if let Some(txs_bytes) = txs {
        new_txs = bincode2::deserialize(txs_bytes.as_slice()).unwrap();
    }

    new_txs.push(tx.clone());

    let tx_bytes: Vec<u8> = bincode2::serialize(&new_txs).unwrap();

    store.set(for_address.as_slice(), &tx_bytes);
}

pub fn get_transfers<S: Storage>(storage: &S, for_address: &CanonicalAddr) -> StdResult<Vec<Tx>> {
    let store = ReadonlyPrefixedStorage::new(PREFIX_TXS, storage);

    if let Some(tx_bytes) = store.get(for_address.as_slice()) {
        let txs: Vec<Tx> = bincode2::deserialize(&tx_bytes).unwrap();
        Ok(txs)
    } else {
        Ok(vec![])
    }
}

pub fn perform_transfer<T: Storage>(
    store: &mut T,
    from: &CanonicalAddr,
    to: &CanonicalAddr,
    amount: u128,
) -> StdResult<()> {
    let mut balances_store = PrefixedStorage::new(PREFIX_BALANCES, store);

    let mut from_balance = read_u128(&balances_store, from.as_slice())?;
    if from_balance < amount {
        return Err(generic_err(format!(
            "Insufficient funds: balance={}, required={}",
            from_balance, amount
        )));
    }
    from_balance -= amount;
    balances_store.set(from.as_slice(), &from_balance.to_be_bytes());

    let mut to_balance = read_u128(&balances_store, to.as_slice())?;
    to_balance += amount;
    balances_store.set(to.as_slice(), &to_balance.to_be_bytes());

    Ok(())
}
