const {
    SigningCosmWasmClient,
    Secp256k1Pen,
    pubkeyToAddress,
    encodeSecp256k1Pubkey,
    CosmWasmClient,
} = require("secretjs");
const fs = require("fs");

const {
    Snip20GetBalance,
    Snip20SetViewingKey,
    GetSnip20Params
} = require("./snip20");

const axios = require("axios");

require("dotenv").config();

const { Bip39, Random } = require("@iov/crypto");

require('dotenv').config();

const createAccount = async () => {
    // Create random address and mnemonic
    const mnemonic = Bip39.encode(Random.getBytes(16)).toString();

    // This wraps a single keypair and allows for signing.
    const signingPen = await Secp256k1Pen.fromMnemonic(mnemonic);

    // Get the public key
    const pubkey = encodeSecp256k1Pubkey(signingPen.pubkey);

    // Get the wallet address
    const accAddress = pubkeyToAddress(pubkey, 'secret');

    // Query the account
    const client = new CosmWasmClient(process.env.SECRET_REST_URL);
    const account = await client.getAccount(accAddress);

    console.log('mnemonic: ', mnemonic);
    console.log('address: ', accAddress);
    console.log('account: ', account);

    return [mnemonic, accAddress, account]
}

const customFees = {
    exec: {
        amount: [{ amount: "250000", denom: "uscrt" }],
        gas: "1000000",
    },
    init: {
        amount: [{ amount: "500000", denom: "uscrt" }],
        gas: "2000000",
    },
    upload: {
        amount: [{ amount: "1000000", denom: "uscrt" }],
        gas: "4000000",
    },
};

const sleep = async (ms) => new Promise((r) => setTimeout(r, ms));

const Instantiate = async (client, initMsg, codeId) => {
    const contract = await client.instantiate(codeId, initMsg, "My Counter" + Math.ceil(Math.random()*10000));
    console.log('contract: ', contract);

    const contractAddress = contract.contractAddress;

    console.log(`Address: ${contractAddress}`);

    return contractAddress;
}


const storeCode = async (path, client) => {
    const wasm = fs.readFileSync(path);
    console.log('Uploading contract')
    const uploadReceipt = await client.upload(wasm, {});
    const codeId = uploadReceipt.codeId;
    console.log('codeId: ', codeId);

    const contractCodeHash = await client.restClient.getCodeHashByCodeId(codeId);
    console.log(`Contract hash: ${contractCodeHash}`);

    return [codeId, contractCodeHash]
}

const createCli = async (mnemonic) => {
    const signingPen = await Secp256k1Pen.fromMnemonic(mnemonic || process.env.MNEMONIC);
    const pubkey = encodeSecp256k1Pubkey(signingPen.pubkey);
    const accAddress = pubkeyToAddress(pubkey, "secret");
    return new SigningCosmWasmClient(
        process.env.SECRET_REST_URL,
        accAddress,
        (data) => signingPen.sign(data),
        signingPen.privkey,
        customFees
    );
};

const queryClaim = async (secretNetwork, contractAddress) => {
    try {
        return secretNetwork.queryContractSmart(
            contractAddress,
            {
                claims: {
                    address: secretNetwork.senderAddress,
                    current_time: Math.trunc( Date.now() / 1000)
                },
            },
        );
    } catch (e) {
        console.log(`Failed to query claim ${e}`);
    }
    return null;
}

const claim = async (secretNetwork, contractAddress) => {
    try {
        return secretNetwork.execute(
            contractAddress,
            {
                claim: {},
            }
        );
    } catch (e) {
        console.log(`Failed to claim ${e}`);
    }
    return null;
}

const deposit = async (secretNetwork, amount, stakingContractAddress) => {
    try {
        return secretNetwork.execute(
            stakingContractAddress,
            {
                stake: {},
            },
            "",
            [{ amount: String(amount), denom: "uscrt" }],
        );
    } catch (e) {
        console.log(`Failed to deposit ${e}`);
    }
    return null;
}

const withdraw = async (secretNetwork, amount, contractAddress, tokenContractAddress) => {
    try {
        return secretNetwork.execute(
            tokenContractAddress,
            {
                send: {recipient: contractAddress, amount: String(amount), msg: "eyJ3aXRoZHJhdyI6IHt9fQ"},
            },
        );
    } catch (e) {
        console.log(`Failed to withdraw ${e}`);
    }
    return null;
}

const viewVote = async (secretNetwork, proposalId, viewingKey, address, tokenContract) => {
    try {
        return secretNetwork.queryContractSmart(
            tokenContract,
            {
                view_vote: { proposal: proposalId, key: viewingKey, address },
            },
        );
    } catch (e) {
        console.log(`Failed to vote ${e}`);
    }
    return null;
}

const vote = async (secretNetwork, proposalId, voteOption, tokenContract) => {
    try {
        return secretNetwork.execute(
            tokenContract,
            {
                vote: { proposal: proposalId, vote: voteOption },
            },
        );
    } catch (e) {
        console.log(`Failed to vote ${e}`);
    }
    return null;
}


const KillSwitchUnbond = async (secretNetwork, stakingContractAddress) => {
    try {
        return secretNetwork.execute(
            stakingContractAddress,
            {
                kill_switch_unbond: {}
            },
        );
    } catch (e) {
        console.log(`Failed to withdraw ${e}`);
    }
    return null;
}


const KillSwitchOpenWithdraws = async (secretNetwork, stakingContractAddress) => {
    try {
        return secretNetwork.execute(
            stakingContractAddress,
            {
                kill_switch_open_withdraws: {}
            },
        );
    } catch (e) {
        console.log(`Failed to withdraw ${e}`);
    }
    return null;
}

const tallyVote = async (secretNetwork, proposalId, votingContractAddress) => {
    try {
        return secretNetwork.execute(
            votingContractAddress,
            {
                tally: { proposal: proposalId },
            },
        );
    } catch (e) {
        console.log(`Failed to withdraw ${e}`);
    }
    return null;
}

const createVote = async (secretNetwork, proposalId, votingContractAddress) => {
    try {
        return secretNetwork.execute(
            votingContractAddress,
            {
                init_vote: { voting_time: 1_000_000, proposal: proposalId },
            },
        );
    } catch (e) {
        console.log(`Failed to withdraw ${e}`);
    }
    return null;
}

const set_voting_contract = async (secretNetwork, contractAddress, votingContractAddress, votingContractHash) => {
    try {
        return secretNetwork.execute(
            contractAddress,
            {
                set_voting_contract: {
                    voting_contract: {address: votingContractAddress, hash: votingContractHash},
                    gov_token: false
                },
            },
        );
    } catch (e) {
        console.log(`Failed to set voting contract ${e}`);
    }
    return null;
}

// const deposit = async () => {
//     try {
//         const rawResults = await axios({
//             method: "post",
//             url: process.env.BAND_URL,
//             headers: {
//                 "Content-Type": "application/json",
//             },
//             data: JSON.stringify({ symbols, min_count: 10, ask_count: 16 }),
//         }).then((r) => r.data["result"]);
//
//         let rates = [];
//         let resolve_times = [];
//         let request_ids = [];
//
//         for ({ multiplier, px, request_id, resolve_time } of rawResults) {
//             rates.push(px);
//             resolve_times.push(Number(resolve_time));
//             request_ids.push(Number(request_id));
//         }
//
//         return { symbols, rates, resolve_times, request_ids };
//     } catch (e) {
//         console.log(e);
//         return null;
//     }
// };


const getExchangeRate = async (secretNetwork, stakingContractAddress) => {
    try {
        return secretNetwork.queryContractSmart(
            stakingContractAddress,
            {
                exchange_rate: {},
            }
        );
    } catch (e) {
        console.log("Fail to get rate from proxy contract");
        console.log(e);
    }
    return null;
};


const validateTx = async (secretNetwork, txHash) => {
    let max_retry = 30;
    while (max_retry > 0) {
        await sleep(1000);
        max_retry--;
        try {
            process.stdout.clearLine();
            process.stdout.cursorTo(0);
            process.stdout.write("polling: " + (30 - max_retry));
            const tx = await secretNetwork.restClient.txById(txHash);

            return tx;
        } catch (err) {
            if (err.isAxiosError && err.response && err.response.status !== 404) {
                console.error(err.response.data);
            } else if (!err.isAxiosError) {
                console.error(err.message);
            }
        }
    }
    return null;
};

const getTokenAddresses = async (secretNetwork, tokenCodeId) => {
    const result = await secretNetwork.getContracts(tokenCodeId);

    console.log(result)

    let gTokenAddress;
    let tokenAddress;

    if (result[0].label.includes('-gov')) {
        tokenAddress = result[1].address
        gTokenAddress = result[0].address
    } else {
        tokenAddress = result[0].address
        gTokenAddress = result[1].address
    }

    return [tokenAddress, gTokenAddress]
}


const getValidator = async () => {
    const result = await axios.get(`${process.env.SECRET_REST_URL}/staking/validators`);

    if (result.status !== 200) {
        throw new Error("Failed to get validators")
    }

    return result.data.result[0].operator_address
}


async function test_multiple_depositors(secretNetwork, tokenContractAddress, stakingContractAddress) {

    for (let i = 0; i < 1000; i++) {
        let [mnemonic, account, _] = await createAccount()

        console.log(`created user: ${account}`)

        const DEPOSIT_AMOUNT = 3_000_000
        const FEE_AMOUNT = 1_000_000

        await secretNetwork.sendTokens(account, [{ amount: String(DEPOSIT_AMOUNT + FEE_AMOUNT), denom: "uscrt" }], "",
            {        amount: [{ amount: "50000", denom: "uscrt" }],
                gas: "200000",})

        console.log(`\tsent scrt from main account to user`)

        let userCli = await createCli(mnemonic);

        await deposit(userCli, DEPOSIT_AMOUNT, stakingContractAddress)

        console.log(`Done deposit number ${i} for user ${account}`)
    }

}

async function test_killswitch(secretNetwork, tokenContractAddress, stakingContractAddress) {
    const users = [];
    const NUM_OF_WITHDRAWS = 5;
    console.log(`testing killswitch, using ${NUM_OF_WITHDRAWS} users`);
    for (let i = 0; i < NUM_OF_WITHDRAWS; i++) {
        let [mnemonic, account, _] = await createAccount()
        let userCli = await createCli(mnemonic);

        users.push({mnemonic, account})

        console.log(`created user: ${account}`)

        const DEPOSIT_AMOUNT = 3_000_000
        const FEE_AMOUNT = 1_000_000

        await secretNetwork.sendTokens(account, [{amount: String(DEPOSIT_AMOUNT + FEE_AMOUNT), denom: "uscrt"}], "",
            {
                amount: [{amount: "50000", denom: "uscrt"}],
                gas: "200000",
            })

        console.log(`\tsent scrt from main account to user`)

        await deposit(userCli, DEPOSIT_AMOUNT, stakingContractAddress);
    }

    console.log(`Done depositing. Unbonding all...`)
    await KillSwitchUnbond(secretNetwork, stakingContractAddress);

    console.log(`Waiting for unbond`);
    await sleep(15000);

    console.log(`Opening withdrawals`);
    await KillSwitchOpenWithdraws(secretNetwork, stakingContractAddress);

    for (let i = 0; i < NUM_OF_WITHDRAWS; i++) {
        let userCli = await createCli(users[i].mnemonic);

        await Snip20SetViewingKey({
            secretjs: userCli,
            address: tokenContractAddress,
            key: "yo"
        });

        let balance = await Snip20GetBalance({
            secretjs: userCli,
            token: tokenContractAddress,
            address: userCli.senderAddress,
            key: "yo"
        });

        let exchange_rate = await getExchangeRate(secretNetwork, stakingContractAddress);
        console.log(`exchange rate: ${exchange_rate.exchange_rate.rate}`);
        console.log(`got ${balance} tokens`);

        let expectedWithdraw = Number(balance) * Number(exchange_rate.exchange_rate.rate)
        console.log(`expected ${expectedWithdraw} uscrt`)

        const scrtBalanceBefore = (await userCli.getAccount(userCli.senderAddress)).balance[0];
        console.log(`balance before: ${scrtBalanceBefore.amount} uscrt`);

        let expectedBalance = Number(scrtBalanceBefore.amount);
        expectedBalance += Number(expectedWithdraw);
        expectedBalance -= 250_000;
        expectedBalance = Math.trunc(expectedBalance);

        console.log(`withdrawing...`)
        await withdraw(userCli, balance, stakingContractAddress, tokenContractAddress);
        console.log(`Done withdraw #${i}`);

        const scrtBalanceAfter = (await userCli.getAccount(userCli.senderAddress)).balance[0];
        console.log(`${JSON.stringify(scrtBalanceAfter)}`)

        if (Number(scrtBalanceAfter.amount) !== expectedBalance) {
            console.error(`Mismatched balances: ${scrtBalanceAfter.amount} !== ${expectedBalance}`)
        } else {
            console.log('Withdrawn successfully')
        }

    }
}

async function test_voting(secretNetwork, tokenContractAddress, stakingContractAddress) {
    const NUM_OF_VOTERS = 5;

    console.log(`Testing voting with ${NUM_OF_VOTERS} voters`)


    for (let i = 0; i < NUM_OF_VOTERS; i++) {
        let [mnemonic, account, _] = await createAccount()
        let userCli = await createCli(mnemonic);

        console.log(`created user: ${account}`)

        const DEPOSIT_AMOUNT = 3_000_000
        const FEE_AMOUNT = 1_000_000

        await secretNetwork.sendTokens(account, [{amount: String(DEPOSIT_AMOUNT + FEE_AMOUNT), denom: "uscrt"}], "",
            {
                amount: [{amount: "50000", denom: "uscrt"}],
                gas: "200000",
            })

        console.log(`\tsent scrt from main account to user`)

        await deposit(userCli, DEPOSIT_AMOUNT, stakingContractAddress)

        console.log(`Done deposit number ${i} for user ${account}`)

        await vote(userCli, 1, "NoWithVeto", tokenContractAddress);

        await Snip20SetViewingKey({
            secretjs: userCli,
            address: tokenContractAddress,
            key: "yo"
        });

        let voteResult = await viewVote(userCli,  1, "yo", userCli.senderAddress, tokenContractAddress);

        console.log(`voted ${voteResult.view_vote.vote} with ${voteResult.view_vote.voting_power}`);

        if (voteResult.view_vote.vote !== "NoWithVeto") {
            throw new Error("Failed to validate vote");
        }

        if (voteResult.view_vote.voting_power <= 0) {
            throw new Error("Failed to validate voting power");
        }
    }

    console.log('Done testing voting')
}

async function test_multiple_withdraws(secretNetwork, tokenContractAddress, stakingContractAddress) {

    const users = [];

    const NUM_OF_WITHDRAWS = 10;
    for (let i = 0; i < NUM_OF_WITHDRAWS; i++) {
        let [mnemonic, account, _] = await createAccount()
        let userCli = await createCli(mnemonic);

        users.push({mnemonic, account})

        console.log(`created user: ${account}`)

        const DEPOSIT_AMOUNT = 3_000_000
        const FEE_AMOUNT = 1_000_000

        await secretNetwork.sendTokens(account, [{ amount: String(DEPOSIT_AMOUNT + FEE_AMOUNT), denom: "uscrt" }], "",
            {        amount: [{ amount: "50000", denom: "uscrt" }],
                gas: "200000",})

        console.log(`\tsent scrt from main account to user`)

        await deposit(userCli, DEPOSIT_AMOUNT, stakingContractAddress)

        console.log(`Done deposit number ${i} for user ${account}`)

        await Snip20SetViewingKey({
            secretjs: userCli,
            address: tokenContractAddress,
            key: "yo"
        });

        let balance = await Snip20GetBalance({
            secretjs: userCli,
            token: tokenContractAddress,
            address: userCli.senderAddress,
            key: "yo"
        });

        console.log(`got ${balance} tokens`);

        console.log(`withdrawing...`)
        await withdraw(userCli, balance, stakingContractAddress, tokenContractAddress)
        console.log(`Done withdraw #${i}`);
    }

    await sleep(15);

    for (let i = 0; i < NUM_OF_WITHDRAWS; i++) {
        let userCli = await createCli(users[i].mnemonic);
        const scrtBalanceBefore = (await userCli.getAccount(userCli.senderAddress)).balance[0];
        console.log(`balance before: ${scrtBalanceBefore.amount} uscrt`);

        let expectedBalance = Number(scrtBalanceBefore.amount);
        const claimResultBefore = await queryClaim(userCli, stakingContractAddress);

        if (claimResultBefore.pending_claims.hasOwnProperty("pending") &&
            claimResultBefore.pending_claims.pending.length > 0 &&
            claimResultBefore.pending_claims.pending[0].hasOwnProperty("withdraw")) {
            expectedBalance += Number(claimResultBefore.pending_claims.pending[0].withdraw.coins.amount);

            console.log(`claiming #${i} for user ${users[i].account}`)
            await claim(userCli, stakingContractAddress);
            console.log(`done claim`);

            let balanceResponse = await userCli.getAccount(userCli.senderAddress);

            const scrtBalanceAfter = (balanceResponse.hasOwnProperty("balance") && balanceResponse.balance.length > 0) ? balanceResponse.balance[0]: {amount: 0};
            console.log(`${JSON.stringify(scrtBalanceAfter)}`)

            if (Number(scrtBalanceAfter.amount) + 250000 !== Number(expectedBalance)) {
                console.error(`Mismatched balances: ${scrtBalanceAfter.amount} + 250000 !== ${Number(expectedBalance)}`)
            } else {
                console.log('Claimed successfully')
            }
        } else {
            console.log(`No claim found for ${userCli.senderAddress}`)
        }
    }
}


// async function test_multiple_withdraws(secretNetwork, tokenContractAddress, stakingContractAddress, NUM_OF_WITHDRAWS) {
//     let balance = await Snip20GetBalance({
//         secretjs: secretNetwork,
//         token: tokenContractAddress,
//         address: secretNetwork.senderAddress,
//         key: "yo"
//     });
//
//     console.log(`balance: ${balance}`);
//
//     for (let i = 0; i < 10; i++) {
//         console.log(`**Depositing... ${i}`)
//         await deposit(secretNetwork, 10000000, stakingContractAddress);
//
//         const tokenParams = await GetSnip20Params({
//             secretjs: secretNetwork,
//             address: tokenContractAddress,
//         });
//
//         console.log(`**token total supply: ${tokenParams.total_supply}`)
//
//         balance = await Snip20GetBalance({
//             secretjs: secretNetwork,
//             token: tokenContractAddress,
//             address: secretNetwork.senderAddress,
//             key: "yo"
//         });
//         console.log(`**balance: ${balance}`);
//     }
//
//     const tokenParams = await GetSnip20Params({
//         secretjs: secretNetwork,
//         address: tokenContractAddress,
//     });
//
//     console.log(`token total supply: ${tokenParams.total_supply}`)
//
//     balance = await Snip20GetBalance({
//         secretjs: secretNetwork,
//         token: tokenContractAddress,
//         address: secretNetwork.senderAddress,
//         key: "yo"
//     });
//
//     if (tokenParams.total_supply !== balance) {
//         console.error(`Token supply does not match current balance: ${tokenParams.total_supply} vs ${balance}`);
//     }
//
//     console.log(`token balance: ${balance}`);
//
//     const scrtBalanceBefore = (await secretNetwork.getAccount(secretNetwork.senderAddress)).balance[0];
//     console.log(`scrt balance before: ${JSON.stringify(scrtBalanceBefore)}`)
//
//     console.log(`Current time: ${Math.trunc(Date.now() / 1000)}`)
//     for (let i = 0; i < NUM_OF_WITHDRAWS; i++) {
//         console.log(`Withdrawing... ${i}`)
//
//         await withdraw(secretNetwork, Math.floor(balance / 20), stakingContractAddress, tokenContractAddress);
//     }
//
//     console.log('Done withdraws');
//
//     console.log(`Current time: ${Math.trunc(Date.now() / 1000)}`)
//     const claimResultBefore = await queryClaim(secretNetwork, stakingContractAddress);
//     console.log(`pending claims: ${JSON.stringify(claimResultBefore)}`);
//
//     let expectedBalance = Number(scrtBalanceBefore.amount);
//     for (let i = 0; i < NUM_OF_WITHDRAWS; i++) {
//         expectedBalance += Number(claimResultBefore.pending_claims.pending[i].withdraw.coins.amount);
//     }
//
//
//     await sleep(15000);
//
//     while (Math.trunc(Date.now() / 1000) < claimResultBefore.pending_claims.pending[NUM_OF_WITHDRAWS - 1].withdraw.available_time) {
//         await sleep(1000);
//     }
//
//     await claim(secretNetwork, stakingContractAddress);
//     console.log("\n\n*** CLAIMED *** \n\n");
//     const claimResultAfter = await queryClaim(secretNetwork, stakingContractAddress);
//     console.log(`after claim: ${JSON.stringify(claimResultAfter)}\n`);
//
//     const scrtBalanceAfter = (await secretNetwork.getAccount(secretNetwork.senderAddress)).balance[0];
//
//     // 250000 = 1 txs
//     if (Number(scrtBalanceAfter.amount) + (NUM_OF_WITHDRAWS + 1) * 250000 !== Number(expectedBalance)) {
//         console.error(`Mismatched balances: ${scrtBalanceAfter.amount} + ${(NUM_OF_WITHDRAWS + 1) * 250000} !== ${Number(expectedBalance)} expected withdraw: ${expectedBalance} + before: ${scrtBalanceBefore.amount}`)
//     } else {
//         console.log('Withdrawn successfully')
//     }
//     console.log(
//         "=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-="
//     );
// }

(async () => {
    const secretNetwork = await createCli();

    const NUM_OF_WITHDRAWS = 10;

    const chainId = await secretNetwork.getChainId();
    const height = await secretNetwork.getHeight();

    console.log("chainId: ", chainId);
    console.log("height: ", height);

    const validatorAddress = await getValidator(secretNetwork);

    console.log(`validator address: ${validatorAddress}`);

    const [cashContractCode, cashContractHash] = await storeCode(process.env.CASH_CONTRACT_PATH, secretNetwork);
    const [cashTokenCode, cashTokenHash] = await storeCode(process.env.CASH_TOKEN_PATH, secretNetwork);
    const [votingCode, votingHash] = await storeCode(process.env.VOTING_TOKEN_PATH, secretNetwork);

    const label = Math.random().toString(36).substring(10);

    const stakingInitMsg = {
        prng_seed: "YWE",
        token_code_id: cashTokenCode,
        token_code_hash: cashTokenHash,
        label: label,
        symbol: "",
        validator: validatorAddress
    }

    const stakingContractAddress = await Instantiate(secretNetwork, stakingInitMsg, cashContractCode);
    const [tokenContractAddress, gtokenContractAddress] = await getTokenAddresses(secretNetwork, cashTokenCode);

    await Snip20SetViewingKey({
        secretjs: secretNetwork,
        address: tokenContractAddress,
        key: "yo"
    });

    // ********** Init voting ********//

    const votingInitMsg = {
        staking_contract: stakingContractAddress,
        staking_contract_hash: cashContractHash,

        gov_token: tokenContractAddress,
        gov_token_hash: cashTokenHash,

        voting_time: 100_000,
    }
    const votingContractAddress = await Instantiate(secretNetwork, votingInitMsg, votingCode);

    await set_voting_contract(secretNetwork, stakingContractAddress, votingContractAddress, votingHash)

    await createVote(secretNetwork, 1, votingContractAddress)

    // ********** TESTS ********//
    // await test_voting(secretNetwork, tokenContractAddress, stakingContractAddress)
    // await tallyVote(secretNetwork, 1, votingContractAddress);

    //await test_killswitch(secretNetwork, tokenContractAddress, stakingContractAddress)
    try {
        while (true) {


            await test_multiple_withdraws(secretNetwork, tokenContractAddress, stakingContractAddress)
            //await test_multiple_withdraws(secretNetwork, tokenContractAddress, stakingContractAddress, NUM_OF_WITHDRAWS);
            //await test_multiple_depositors(secretNetwork, tokenContractAddress, stakingContractAddress);
            //await sleep(10000);
        }
    } catch (e) {
        console.log(e);
    }

})();