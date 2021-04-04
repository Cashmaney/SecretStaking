#!/bin/bash

set -e

validator_address_1=secretvaloper1snzrncgy6p8aqtamr0w8gk7arhg2lmp2ecq5sm
validator_address_2=secretvaloper1nkd6vs95qwtmpwcqyn4zqyseqdh0u7gs6gd8w6

#function secretcli() {
#  secretcli "$@";
#}

function wait_for_tx() {
  until (secretcli q tx "$1"); do
      sleep 5
  done
}

export SGX_MODE=HW

deployer_name=t4

deployer_address=$(secretcli keys show -a $deployer_name)
echo "Deployer address: '$deployer_address'"

# validator_address=$(docker exec -it secretdev secretcli q staking validators | jq '.[0].operator_address')
echo "Validator address 1: '$validator_address_1'"
echo "Validator address 2: '$validator_address_2'"

secretcli tx compute store "../build/secretstaking_token.wasm" --from $deployer_name --gas 4000000 -b block -y
token_code_id=$(secretcli query compute list-code | jq '.[-1]."id"')
token_code_hash=$(secretcli query compute list-code | jq '.[-1]."data_hash"')
echo "Stored token: '$token_code_id', '$token_code_hash'"

secretcli tx compute store "../build/secret_staking.wasm" --from $deployer_name --gas 4000000 -b block -y
factory_code_id=$(secretcli query compute list-code | jq '.[-1]."id"')
echo "Stored staking: '$factory_code_id'"


echo "Deploying token..."
tokenlabel=$(date +"%T")
#export STORE_TX_HASH=$(
#  secretcli tx compute instantiate $token_code_id '{"admin": "'$deployer_address'", "symbol": "TST", "decimals": 6, "initial_balances": [], "prng_seed": "YWE", "name": "test"}' --from $deployer_name --gas 1500000 --label $label -b block -y |
#  jq -r .txhash
#)
#wait_for_tx "$STORE_TX_HASH" "Waiting for instantiate to finish on-chain..."
sleep 2

label=$(date +"%T")
export STORE_TX_HASH=$(
  secretcli tx compute instantiate $factory_code_id '{ "prng_seed": "YWE", "token_code_id": '$token_code_id', "token_code_hash": '$token_code_hash', "label": "'$tokenlabel'", "symbol": "", "validator": "'$validator_address_1'"}' --label $label --from $deployer_name --gas 1500000 -y |
  jq -r .txhash
)
wait_for_tx "$STORE_TX_HASH" "Waiting for instantiate to finish on-chain..."

token_addr=$(secretcli q compute label "$tokenlabel" | tail -c 46)
echo "Token address: $token_addr"
gtoken_addr=$(secretcli q compute label "$token_addr"-gov | tail -c 46)
echo "governance token address: $gtoken_addr"


staking_contract=$(secretcli query compute list-contract-by-code $factory_code_id | jq '.[-1].address')
echo "staking address: '$staking_contract'"

# secretcli tx compute execute $(echo "$token_addr" | tr -d '"') '{"add_minters": {"minters": ['$staking_contract']}}' -b block -y --from $deployer_name

secretcli tx compute execute $token_addr '{"set_viewing_key": {"key": "yo"}}' -b block -y --from $deployer_name
secretcli tx compute execute $gtoken_addr '{"set_viewing_key": {"key": "yo"}}' -b block -y --from $deployer_name


balance=$(secretcli q account $deployer_address | jq '.value.coins[0].amount')
echo "USCRT balance before deposit: '$balance'"

tbalance=$(secretcli q compute query $token_addr '{"balance": {"address": "'$deployer_address'", "key": "yo"}}' | jq '.balance.amount')
echo "Token balance before deposit: '$tbalance'"

secretcli tx compute execute $(echo "$staking_contract" | tr -d '"') '{"deposit": {}}' --amount 1000000uscrt -b block -y --gas 1000000 --from $deployer_name
#
tbalance=$(secretcli q compute query $token_addr '{"balance": {"address": "'$deployer_address'", "key": "yo"}}' | jq '.balance.amount')
echo "Token balance after deposit: '$tbalance'"
balance=$(secretcli q account $deployer_address | jq '.value.coins[0].amount')
echo "USCRT balance after deposit: '$balance'"
#
echo "Waiting for 2 blocks"
sleep 13
#
secretcli q compute query $(echo "$staking_contract" | tr -d '"') '{"exchange_rate": {}}'
#

echo "Adding second validator"
secretcli tx compute execute $(echo "$staking_contract" | tr -d '"') '{"add_validator": {"address": "'$validator_address_2'"}}' -b block -y --gas 1000000 --from $deployer_name

echo "Depositing 1,000,000 uscrt"
#
secretcli tx compute execute $(echo "$staking_contract" | tr -d '"') '{"deposit": {}}' --amount 1000000uscrt -b block -y --gas 1000000 --from $deployer_name
#


tbalance=$(secretcli q compute query $token_addr '{"balance": {"address": "'$deployer_address'", "key": "yo"}}' | jq '.balance.amount')
echo "Token balance after deposit2: '$tbalance'"
balance=$(secretcli q account $deployer_address | jq '.value.coins[0].amount')
echo "USCRT balance after deposit2: '$balance'"


secretcli tx compute execute $(echo "$staking_contract" | tr -d '"') '{"change_weight": {"address": "secretvaloper1nkd6vs95qwtmpwcqyn4zqyseqdh0u7gs6gd8w6", "weight": 4}}' -b block -y --gas 1000000 --from $deployer_name

secretcli tx compute execute $(echo "$token_addr" | tr -d '"') '{"send": {"recipient": '$staking_contract', "amount": "1000000", "msg": "eyJ3aXRoZHJhdyI6IHt9fQ"}}' -b block -y --gas 1000000 --from $deployer_name

#
#secretcli tx compute execute $(echo "$token_addr" | tr -d '"') '{"send": {"recipient": '$staking_contract', "amount": "1000000"}}' -b block -y --gas 1000000 --from $deployer_name
#
#tbalance=$(secretcli q compute query $(echo "$token_addr" | tr -d '"') '{"balance": {"address": "'$deployer_address'", "key": "yo"}}' | jq '.balance.amount')
#echo "Token balance after withdraw: '$tbalance'"
#balance=$(secretcli q account $deployer_address | jq '.value.coins[0].amount')
#echo "USCRT balance after withdraw: '$balance'"
#
#
## Test exchange rate
#
#secretcli q compute query $(echo "$staking_contract" | tr -d '"') '{"exchange_rate": {}}'
#echo "Waiting for 2 blocks"
#sleep 7
#
#secretcli q compute query $(echo "$staking_contract" | tr -d '"') '{"exchange_rate": {}}'
#
## Test claims query
#
#secretcli q compute query $(echo "$staking_contract" | tr -d '"') '{"pending_claims": {"address": "'$deployer_address'"}}'
#
#echo "Current time: '$(date "+%s")'"
#
#echo "Waiting 5 seconds..."
#sleep 5
#
#echo "Current time: '$(date "+%s")'"
#
#secretcli q compute query $(echo "$staking_contract" | tr -d '"') '{"pending_claims": {"address": "'$deployer_address'", "current_time": '$(date "+%s")'}}'
#
#secretcli tx compute execute $(echo "$staking_contract" | tr -d '"') '{"claim": {}}' -b block -y --gas 1000000 --from $deployer_name
#balance=$(secretcli q account $deployer_address | jq '.value.coins[0].amount')
#echo "USCRT balance after claim: '$balance'"
#
## Test withdraw removed from claims
#
#secretcli q compute query $(echo "$staking_contract" | tr -d '"') '{"pending_claims": {"address": "'$deployer_address'", "current_time": '$(date "+%s")'}}'
#
#
## Test failed to withdraw
#
#echo "Depositing 1,000,000 uscrt"
#secretcli tx compute execute $(echo "$staking_contract" | tr -d '"') '{"deposit": {}}' --amount 1000000uscrt -b block -y --gas 1000000 --from $deployer_name
#tbalance=$(secretcli q compute query $(echo "$token_addr" | tr -d '"') '{"balance": {"address": "'$deployer_address'", "key": "yo"}}' | jq '.balance.amount')
#echo "Token balance after withdraw: '$tbalance'"
#echo "Withdrawing 1,000,000 uscrt"
#secretcli tx compute execute $(echo "$token_addr" | tr -d '"') '{"send": {"recipient": '$staking_contract', "amount": '$tbalance'}}' -b block -y --gas 1000000 --from $deployer_name
#
#echo "Current time: '$(date "+%s")'"
#secretcli q compute query $(echo "$staking_contract" | tr -d '"') '{"pending_claims": {"address": "'$deployer_address'", "current_time": '$(date "+%s")'}}'
#secretcli tx compute execute $(echo "$staking_contract" | tr -d '"') '{"claim": {}}' -b block -y --gas 1000000 --from $deployer_name
#
## voting
#
#secretcli tx gov submit-proposal community-pool-spend /root/code/build/proposal.json -b block -y --from $deployer_name
#
#secretcli query gov proposal 1
#
#secretcli tx compute execute $(echo "$staking_contract" | tr -d '"') '{"vote": {"proposal": 1, "vote": "Yes"}}' -b block -y --gas 1000000 --from $deployer_name
#
#secretcli tx compute execute $(echo "$staking_contract" | tr -d '"') '{"tally": {"proposal": 1}}' -b block -y --gas 1000000 --from $deployer_name
#
#secretcli query gov votes 1