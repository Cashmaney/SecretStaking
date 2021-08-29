#!/bin/bash

set -e

docker_name=secretdev

function secretcli() {
  docker exec "$docker_name" secretcli "$@";
}

function wait_for_tx() {
  until (secretcli q tx "$1"); do
      sleep 5
  done
}

export SGX_MODE=SW

deployer_name=b

deployer_address=$(secretcli keys show -a $deployer_name)
echo "Deployer address: '$deployer_address'"

secretcli tx send $deployer_address secret18qluets3enmm20f5sk6282c609q2ggg9kp2yuy 5000000000000uscrt -b block -y

echo "Sent 5M SCRT from '$deployer_address' to secret18qluets3enmm20f5sk6282c609q2ggg9kp2yuy"

#
#validator_address=$(docker exec -it secretdev secretcli q staking validators | jq '.[0].operator_address')
#echo "Validator address: '$validator_address'"
#
#docker exec -it "$docker_name" secretcli tx compute store "/root/code/build/secretstaking_token.wasm" --from $deployer_name --gas 4000000 -b block -y
#token_code_id=$(secretcli query compute list-code | jq '.[-1]."id"')
#token_code_hash=$(secretcli query compute list-code | jq '.[-1]."data_hash"')
#echo "Stored token: '$token_code_id', '$token_code_hash'"
#
#docker exec -it $docker_name secretcli tx compute store "/root/code/build/secret_staking.wasm" --from $deployer_name --gas 4000000 -b block -y
#factory_code_id=$(secretcli query compute list-code | jq '.[-1]."id"')
#echo "Stored staking: '$factory_code_id'"
#
#
#echo "Deploying contract..."
#tokenlabel=$(date +"%T")
##export STORE_TX_HASH=$(
##  secretcli tx compute instantiate $token_code_id '{"admin": "'$deployer_address'", "symbol": "TST", "decimals": 6, "initial_balances": [], "prng_seed": "YWE", "name": "test"}' --from $deployer_name --gas 1500000 --label $label -b block -y |
##  jq -r .txhash
##)
##wait_for_tx "$STORE_TX_HASH" "Waiting for instantiate to finish on-chain..."
#sleep 2
#
#label=$(date +"%T")
#export STORE_TX_HASH=$(
#  secretcli tx compute instantiate $factory_code_id '{ "prng_seed": "YWE", "token_code_id": '$token_code_id', "token_code_hash": '$token_code_hash', "label": "'$tokenlabel'", "symbol": "", "validator": '$validator_address'}' --label $label --from $deployer_name --gas 1500000 -y |
#  jq -r .txhash
#)
#wait_for_tx "$STORE_TX_HASH" "Waiting for instantiate to finish on-chain..."
#
#token_addr=$(secretcli q compute label "$tokenlabel" | tail -c 46)
#echo "Token address: $token_addr"
#gtoken_addr=$(secretcli q compute label "$token_addr"-gov | tail -c 46)
#echo "governance token address: $gtoken_addr"
#
#
#
#staking_contract=$(secretcli query compute list-contract-by-code $factory_code_id | jq '.[-1].address')
#echo "staking address: '$staking_contract'"
#
## secretcli tx compute execute $(echo "$token_addr" | tr -d '"') '{"add_minters": {"minters": ['$staking_contract']}}' -b block -y --from $deployer_name
#
#secretcli tx compute execute $token_addr '{"set_viewing_key": {"key": "yo"}}' -b block -y --from $deployer_name
#secretcli tx compute execute $gtoken_addr '{"set_viewing_key": {"key": "yo"}}' -b block -y --from $deployer_name
#
#secretcli tx send $deployer_address secret18qluets3enmm20f5sk6282c609q2ggg9kp2yuy 5000000000uscrt -b block -y
#
#secretcli tx gov submit-proposal community-pool-spend /root/code/build/proposal.json -b block -y --from $deployer_name
#
#export CASH_TOKEN_ADDRESS=$token_addr
#export CASH_CONTRACT_ADDRESS=$staking_contract
#export GCASH_TOKEN_ADDRESS=$gtoken_addr
#
#echo "CASH_CONTRACT_ADDRESS=$staking_contract"
#echo "CASH_TOKEN_ADDRESS=$token_addr"
#echo "GCASH_TOKEN_ADDRESS=$gtoken_addr"