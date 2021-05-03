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
                pending_claims: {
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
                deposit: {},
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


const getExchangeRate = async (secretNetwork) => {
    try {
        return secretNetwork.queryContractSmart(
            process.env.CASH_CONTRACT_ADDRESS,
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


async function test_multiple_withdraws(secretNetwork, tokenContractAddress, stakingContractAddress, NUM_OF_WITHDRAWS) {
    let balance = await Snip20GetBalance({
        secretjs: secretNetwork,
        token: tokenContractAddress,
        address: secretNetwork.senderAddress,
        key: "yo"
    });

    console.log(`balance: ${balance}`);

    for (let i = 0; i < 10; i++) {
        console.log(`**Depositing... ${i}`)
        await deposit(secretNetwork, 10000000, stakingContractAddress);

        const tokenParams = await GetSnip20Params({
            secretjs: secretNetwork,
            address: tokenContractAddress,
        });

        console.log(`**token total supply: ${tokenParams.total_supply}`)

        balance = await Snip20GetBalance({
            secretjs: secretNetwork,
            token: tokenContractAddress,
            address: secretNetwork.senderAddress,
            key: "yo"
        });
        console.log(`**balance: ${balance}`);
    }

    const tokenParams = await GetSnip20Params({
        secretjs: secretNetwork,
        address: tokenContractAddress,
    });

    console.log(`token total supply: ${tokenParams.total_supply}`)

    balance = await Snip20GetBalance({
        secretjs: secretNetwork,
        token: tokenContractAddress,
        address: secretNetwork.senderAddress,
        key: "yo"
    });

    if (tokenParams.total_supply !== balance) {
        console.error(`Token supply does not match current balance: ${tokenParams.total_supply} vs ${balance}`);
    }

    console.log(`token balance: ${balance}`);

    const scrtBalanceBefore = (await secretNetwork.getAccount(secretNetwork.senderAddress)).balance[0];
    console.log(`scrt balance before: ${JSON.stringify(scrtBalanceBefore)}`)

    console.log(`Current time: ${Math.trunc(Date.now() / 1000)}`)
    for (let i = 0; i < NUM_OF_WITHDRAWS; i++) {
        console.log(`Withdrawing... ${i}`)

        await withdraw(secretNetwork, Math.floor(balance / 20), stakingContractAddress, tokenContractAddress);
    }

    console.log('Done withdraws');

    console.log(`Current time: ${Math.trunc(Date.now() / 1000)}`)
    const claimResultBefore = await queryClaim(secretNetwork, stakingContractAddress);
    console.log(`pending claims: ${JSON.stringify(claimResultBefore)}`);

    let expectedBalance = Number(scrtBalanceBefore.amount);
    for (let i = 0; i < NUM_OF_WITHDRAWS; i++) {
        expectedBalance += Number(claimResultBefore.pending_claims.pending[i].withdraw.coins.amount);
    }


    await sleep(15000);

    while (Math.trunc(Date.now() / 1000) < claimResultBefore.pending_claims.pending[NUM_OF_WITHDRAWS - 1].withdraw.available_time) {
        await sleep(1000);
    }

    await claim(secretNetwork, stakingContractAddress);
    console.log("\n\n*** CLAIMED *** \n\n");
    const claimResultAfter = await queryClaim(secretNetwork, stakingContractAddress);
    console.log(`after claim: ${JSON.stringify(claimResultAfter)}\n`);

    const scrtBalanceAfter = (await secretNetwork.getAccount(secretNetwork.senderAddress)).balance[0];

    // 250000 = 1 txs
    if (Number(scrtBalanceAfter.amount) + (NUM_OF_WITHDRAWS + 1) * 250000 !== Number(expectedBalance)) {
        console.error(`Mismatched balances: ${scrtBalanceAfter.amount} + ${(NUM_OF_WITHDRAWS + 1) * 250000} !== ${Number(expectedBalance)} expected withdraw: ${expectedBalance} + before: ${scrtBalanceBefore.amount}`)
    } else {
        console.log('Withdrawn successfully')
    }
    console.log(
        "=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-="
    );
}

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

    try {
        while (true) {

            //await test_multiple_withdraws(secretNetwork, tokenContractAddress, stakingContractAddress, NUM_OF_WITHDRAWS);
            await test_multiple_depositors(secretNetwork, tokenContractAddress, stakingContractAddress);
            await sleep(10000);
        }
    } catch (e) {
        console.log(e);
    }

})();