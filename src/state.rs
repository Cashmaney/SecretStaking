use bincode2;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    generic_err, CanonicalAddr, Coin, HumanAddr, ReadonlyStorage, StdError, StdResult, Storage,
    Uint128,
};
use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};

use crate::utils::{bytes_to_u128, bytes_to_u32};
use std::borrow::{Borrow, BorrowMut};
use std::ops::Deref;

pub const EXCHANGE_RATE_RESOLUTION: u32 = 1000;
pub const FEE_RESOLUTION: u32 = 10000;
pub const KEY_FEE: &[u8] = b"fee";
pub const PREFIX_BALANCES: &[u8] = b"balances";
pub const PREFIX_ALLOWANCES: &[u8] = b"allowances";
pub const PREFIX_CONFIG: &[u8] = b"config";

pub const INITIAL_LIQUIDITY_POOL: &[u8] = b"initial_liquidity_pool";

pub const LIQUIDITY_RATIO: &[u8] = b"liquidity_ratio";
pub const KEY_CONSTANTS: &[u8] = b"constants";
pub const KEY_TOTAL_TOKENS: &[u8] = b"total_supply";
pub const TARGET_RATIO: &[u8] = b"target_staking_ratio";
pub const KEY_LIQUIDITY_POOL: &[u8] = b"liquidity_pool";
pub const KEY_TOTAL_BALANCE: &[u8] = b"total_balance";
pub const VALIDATOR_ADDRESS_KEY: &[u8] = b"validator_address";

pub static CONFIG_KEY: &[u8] = b"config";
pub const PREFIX_TXS: &[u8] = b"transfers";
pub const CONTRACT_ADDRESS: &[u8] = "contract_address".as_bytes();

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Tx {
    pub sender: HumanAddr,
    pub receiver: HumanAddr,
    pub coins: Coin,
}

/// This is here so we can create constant length transactions if we want to return this on-chain instead of a query
impl Default for Tx {
    fn default() -> Self {
        Self {
            sender: Default::default(),
            receiver: Default::default(),
            coins: Coin {
                denom: "EMPT".to_string(),
                amount: Uint128::zero(),
            },
        }
    }
}

#[derive(Serialize, Debug, Deserialize, Clone, PartialEq)]
pub struct Constants {
    pub admin: HumanAddr,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

pub fn store_address<S: Storage>(storage: &mut S, address: &CanonicalAddr) {
    let address_bytes: Vec<u8> = bincode2::serialize(&address).unwrap();

    storage.set(&CONTRACT_ADDRESS, &address_bytes);
}

pub fn get_address<S: Storage>(storage: &mut S) -> StdResult<CanonicalAddr> {
    if let Some(address_bytes) = storage.get(&CONTRACT_ADDRESS) {
        let record: CanonicalAddr = bincode2::deserialize(&address_bytes).unwrap();
        Ok(record)
    } else {
        Err(StdError::GenericErr {
            msg: "Privacy token not available for this token".to_string(),
            backtrace: None,
        })
    }
}

/// Reads 4 byte storage value into u32
/// Returns zero if key does not exist. Errors if data found that is not 4 bytes
pub fn read_u32<S: ReadonlyStorage>(store: &S, key: &[u8]) -> StdResult<u32> {
    let result = store.get(key);
    match result {
        Some(data) => bytes_to_u32(data.as_slice()),
        None => Ok(0u32),
    }
}

/// Reads 16 byte storage value into u128
/// Returns zero if key does not exist. Errors if data found that is not 16 bytes
pub fn read_u128<S: ReadonlyStorage>(store: &S, key: &[u8]) -> StdResult<u128> {
    let result = store.get(key);
    match result {
        Some(data) => bytes_to_u128(&data),
        None => Ok(0u128),
    }
}

pub fn read_token_balance<S: Storage>(store: &S, owner: &CanonicalAddr) -> StdResult<u128> {
    let balance_store = ReadonlyPrefixedStorage::new(PREFIX_BALANCES, store);
    read_u128(&balance_store, owner.as_slice())
}

pub fn add_token_balance<S: Storage>(
    store: &mut S,
    owner: &CanonicalAddr,
    amount: u128,
) -> StdResult<u128> {
    let mut balance_store = PrefixedStorage::new(PREFIX_BALANCES, store);

    let mut balance = read_u128(&balance_store, owner.as_slice())?;
    balance += amount;

    balance_store.set(owner.as_slice(), &balance.to_be_bytes());

    Ok(balance)
}

pub fn remove_balance<S: Storage>(
    store: &mut S,
    owner: &CanonicalAddr,
    amount: u128,
) -> StdResult<u128> {
    let mut balance_store = PrefixedStorage::new(PREFIX_BALANCES, store);

    let mut balance = read_u128(&balance_store, owner.as_slice())?;
    balance -= amount;

    balance_store.set(owner.as_slice(), &balance.to_be_bytes());

    Ok(balance)
}

pub fn set_initial_liquidity<S: Storage>(store: &mut S, amount: u128) -> StdResult<()> {
    let mut config_store = PrefixedStorage::new(CONFIG_KEY, store);
    config_store.set(INITIAL_LIQUIDITY_POOL, &amount.to_be_bytes());

    Ok(())
}

pub fn get_initial_liquidity<S: Storage>(store: &S) -> StdResult<u128> {
    let config_store = ReadonlyPrefixedStorage::new(CONFIG_KEY, store);
    let liquidity_ratio = read_u128(&config_store, INITIAL_LIQUIDITY_POOL)?;
    Ok(liquidity_ratio)
}

pub fn get_fee<S: Storage>(store: &S) -> StdResult<u32> {
    let config_store = ReadonlyPrefixedStorage::new(CONFIG_KEY, store);
    let fee = read_u32(&config_store, KEY_FEE)?;
    Ok(fee)
}

// units of fee are set in the constant FEE_RESOLUTION -- currently 0.0001 (== pips)
pub fn set_fee<S: Storage>(store: &mut S, fee: u32) -> StdResult<()> {
    let mut config_store = PrefixedStorage::new(CONFIG_KEY, store);
    config_store.set(KEY_FEE, &fee.to_be_bytes());

    Ok(())
}

pub fn get_staked_ratio<S: Storage>(store: &S) -> StdResult<u128> {
    let config_store = ReadonlyPrefixedStorage::new(CONFIG_KEY, store);
    let liquidity_ratio = read_u128(&config_store, TARGET_RATIO)?;
    Ok(liquidity_ratio)
}

pub fn set_liquidity_ratio<S: Storage>(store: &mut S, amount: u128) -> StdResult<()> {
    let mut config_store = PrefixedStorage::new(CONFIG_KEY, store);
    config_store.set(TARGET_RATIO, &amount.to_be_bytes());

    Ok(())
}

/// todo: validator address is a String till we test with HumanAddr and see that secretval address are working
pub fn get_validator_address<S: Storage>(store: &S) -> StdResult<String> {
    let config_store = ReadonlyPrefixedStorage::new(CONFIG_KEY, store);
    let x = config_store.get(VALIDATOR_ADDRESS_KEY).unwrap();
    let record =
        String::from_utf8(x).map_err(|_| generic_err("Error unpacking validator address"))?;
    Ok(record)
}

pub fn set_validator_address<S: Storage>(
    store: &mut S,
    validator_address: &String,
) -> StdResult<()> {
    let mut config_store = PrefixedStorage::new(CONFIG_KEY, store);
    config_store.set(
        VALIDATOR_ADDRESS_KEY,
        &validator_address.as_bytes().to_vec(),
    );

    Ok(())
}

pub fn read_constants<S: Storage>(store: &S) -> StdResult<Constants> {
    let config_store = ReadonlyPrefixedStorage::new(PREFIX_CONFIG, store);
    let consts_bytes = config_store.get(KEY_CONSTANTS).unwrap();

    let consts: Constants = bincode2::deserialize(&consts_bytes).unwrap();

    Ok(consts)
}

pub fn get_delegation_tokens<S: Storage>(store: &S) -> u128 {
    let config_store = ReadonlyPrefixedStorage::new(CONFIG_KEY, store);
    let data = config_store
        .get(KEY_TOTAL_TOKENS)
        .expect("no total supply data stored");
    let total_supply = bytes_to_u128(&data).unwrap();

    total_supply
}

/// used to cache the liquidity pool balance so we don't have query the chain every time
pub fn liquidity_pool_balance<S: Storage>(store: &S) -> u128 {
    let config_store = ReadonlyPrefixedStorage::new(CONFIG_KEY, store);
    let data = config_store
        .get(KEY_LIQUIDITY_POOL)
        .expect("no total supply data stored");
    let total_supply = bytes_to_u128(&data).unwrap();

    total_supply
}

/// Updates the cached liquidity pool size to the amount of SCRT it contains - basically the available balance of the contract
pub fn update_cached_liquidity_balance<S: Storage>(store: &mut S, amount: u128) {
    let mut config_store = PrefixedStorage::new(CONFIG_KEY, store);
    config_store.set(KEY_LIQUIDITY_POOL, &amount.to_be_bytes())
}

/// Updates the total balance according to the amount of SCRT earned
/// This is cached once a day and is used to calculate the exchange rate
pub fn update_total_balance<S: Storage>(store: &mut S, amount: u128) {
    let mut config_store = PrefixedStorage::new(CONFIG_KEY, store);
    config_store.set(KEY_TOTAL_BALANCE, &amount.to_be_bytes())
}

/// returns the exchange ratio of delegation tokens to native coins
/// returns the exchange rate per 1000 tokens
pub fn get_exchange_rate<S: Storage>(store: &S) -> StdResult<u128> {
    let config_store = ReadonlyPrefixedStorage::new(CONFIG_KEY, store);
    let mut token_supply = read_u128(&config_store, KEY_TOTAL_TOKENS)?;
    let total_balance = read_u128(&config_store, KEY_TOTAL_BALANCE)?;
    let initial_balance = read_u128(&config_store, INITIAL_LIQUIDITY_POOL)?;

    // this will help us get the resolution we want even though we're just working with uints
    token_supply = token_supply / (EXCHANGE_RATE_RESOLUTION as u128);

    // if we have static liquidity we only want to return the difference without taking that
    // liquidity into account
    return if initial_balance > 0 && total_balance > initial_balance {
        Ok(1)
    } else {
        if token_supply == 0 {
            return Ok(1);
        }
        Ok((total_balance - initial_balance) / token_supply)
    };
}

/// Calculates how much your withdrawn tokens are worth in SCRT
/// Removes the balance from the total supply and balance
/// Returns amount of SCRT your tokens earned
pub fn withdraw<S: Storage>(
    store: &mut S,
    amount: u128,
    exchange_rate: u128,
    fee: u32,
) -> StdResult<u128> {
    let mut config_store = PrefixedStorage::new(CONFIG_KEY, store);
    let mut total_supply = read_u128(&config_store, KEY_TOTAL_TOKENS)?;
    let mut total_balance = read_u128(&config_store, KEY_TOTAL_BALANCE)?;

    let raw_amount = exchange_rate * amount;
    let fee_amount = raw_amount * fee as u128 / (FEE_RESOLUTION as u128);

    let coins_to_withdraw = (raw_amount - fee_amount) / (EXCHANGE_RATE_RESOLUTION as u128);

    total_supply -= amount;
    total_balance -= coins_to_withdraw;

    config_store.set(KEY_TOTAL_TOKENS, &total_supply.to_be_bytes());
    config_store.set(KEY_TOTAL_BALANCE, &total_balance.to_be_bytes());

    Ok(coins_to_withdraw)
}

/// Calculates how much your deposited SCRT is worth in tokens
/// Adds the balance from the total supply and balance
/// Returns amount of tokens you get
pub fn deposit<S: Storage>(store: &mut S, amount: u128, exchange_rate: u128) -> StdResult<u128> {
    let mut config_store = { PrefixedStorage::new(CONFIG_KEY, store) };

    let mut total_supply = read_u128(&config_store, KEY_TOTAL_TOKENS)?;
    let mut total_balance = read_u128(&config_store, KEY_TOTAL_BALANCE)?;

    let tokens_to_mint = exchange_rate * amount / (EXCHANGE_RATE_RESOLUTION as u128);

    total_supply += amount;
    total_balance += tokens_to_mint;

    config_store.set(KEY_TOTAL_TOKENS, &total_supply.to_be_bytes());
    config_store.set(KEY_TOTAL_BALANCE, &total_balance.to_be_bytes());

    Ok(tokens_to_mint)
}
