const {
    SigningCosmWasmClient,
    Secp256k1Pen,
    pubkeyToAddress,
    encodeSecp256k1Pubkey,
} = require("secretjs");

const {
    Snip20GetBalance,
    Snip20SetViewingKey,
    GetSnip20Params
} = require("./snip20");

const axios = require("axios");

require("dotenv").config();

const customFees = {
    exec: {
        amount: [{ amount: "250000", denom: "uscrt" }],
        gas: "1000000",
    },
};

const sleep = async (ms) => new Promise((r) => setTimeout(r, ms));

const createCli = async () => {
    const signingPen = await Secp256k1Pen.fromMnemonic(process.env.MNEMONIC);
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

const queryClaim = async (secretNetwork) => {
    try {
        return secretNetwork.queryContractSmart(
            process.env.CASH_CONTRACT_ADDRESS,
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

const claim = async (secretNetwork) => {
    try {
        return secretNetwork.execute(
            process.env.CASH_CONTRACT_ADDRESS,
            {
                claim: {},
            }
        );
    } catch (e) {
        console.log(`Failed to claim ${e}`);
    }
    return null;
}

const deposit = async (secretNetwork, amount) => {
    try {
        return secretNetwork.execute(
            process.env.CASH_CONTRACT_ADDRESS,
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

const withdraw = async (secretNetwork, amount) => {
    try {
        return secretNetwork.execute(
            process.env.CASH_TOKEN_ADDRESS,
            {
                send: {recipient: process.env.CASH_CONTRACT_ADDRESS, amount: String(amount), msg: "eyJ3aXRoZHJhdyI6IHt9fQ"},
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

// (async () => {
//     const secretNetwork = await createCli();
//
//     const chainId = await secretNetwork.getChainId();
//     const height = await secretNetwork.getHeight();
//
//     console.log("chainId: ", chainId);
//     console.log("height: ", height);
//
//     await Snip20SetViewingKey({
//         secretjs: secretNetwork,
//         address: process.env.CASH_TOKEN_ADDRESS,
//         key: "yo"
//     });
//
//     try {
//         while (true) {
//
//             let balance = await Snip20GetBalance({
//                 secretjs: secretNetwork,
//                 token: process.env.CASH_TOKEN_ADDRESS,
//                 address: secretNetwork.senderAddress,
//                 key: "yo"
//             });
//
//             console.log(`balance: ${balance}`);
//
//             console.log('Depositing...')
//             await deposit(secretNetwork, 1000000);
//             console.log('waiting for 1 block');
//
//             const tokenParams = await GetSnip20Params({secretjs: secretNetwork,
//                 address: process.env.CASH_TOKEN_ADDRESS,
//             });
//
//             console.log(`token total supply: ${tokenParams.total_supply}`)
//
//
//
//             await sleep(6000);
//
//             balance = await Snip20GetBalance({secretjs: secretNetwork,
//                 token: process.env.CASH_TOKEN_ADDRESS,
//                 address: secretNetwork.senderAddress,
//                 key: "yo"
//             });
//
//             if (tokenParams.total_supply !== balance) {
//                 console.error(`Token supply does not match current balance: ${tokenParams.total_supply} vs ${balance}`);
//             }
//
//             console.log(`balance: ${balance}`);
//
//             const scrtBalanceBefore = (await secretNetwork.getAccount(secretNetwork.senderAddress)).balance[0];
//             console.log(`balance before: ${JSON.stringify(scrtBalanceBefore)}`)
//             console.log('Withdrawing...')
//             await withdraw(secretNetwork, balance);
//             console.log('Done withdraw');
//             console.log(`Current time: ${Math.trunc( Date.now() / 1000)}`)
//             const claimResultBefore = await queryClaim(secretNetwork);
//             console.log(`claim result: ${JSON.stringify(claimResultBefore)}`);
//             const expectedBalance = claimResultBefore.pending_claims.pending[0].withdraw.coins.amount;
//
//             await sleep(15000);
//
//             while(Math.trunc( Date.now() / 1000) < claimResultBefore.pending_claims.pending[0].withdraw.available_time) {
//                 await sleep(1000);
//             }
//
//             await claim(secretNetwork);
//
//             const claimResultAfter = await queryClaim(secretNetwork);
//             console.log(`after claim: ${JSON.stringify(claimResultAfter)}`);
//
//             const scrtBalanceAfter = (await secretNetwork.getAccount(secretNetwork.senderAddress)).balance[0];
//
//             // - 500000 = 2 txs
//             if (Number(scrtBalanceAfter.amount) + 500000 !== Number(expectedBalance) + Number(scrtBalanceBefore.amount)) {
//                 console.error(`Mismatched balances: ${scrtBalanceAfter.amount} !== ${Number(expectedBalance) + Number(scrtBalanceBefore.amount)} expected withdraw: ${expectedBalance} + before: ${scrtBalanceBefore.amount}`)
//             } else {
//                 console.log('Withdrawn successfully')
//             }
//             console.log(
//                 "=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-="
//             );
//             await sleep(10000);
//         }
// } catch (e) {
//         console.log(e);
//     }
//
// })();

(async () => {
    const secretNetwork = await createCli();

    const chainId = await secretNetwork.getChainId();
    const height = await secretNetwork.getHeight();

    console.log("chainId: ", chainId);
    console.log("height: ", height);

    await Snip20SetViewingKey({
        secretjs: secretNetwork,
        address: process.env.CASH_TOKEN_ADDRESS,
        key: "yo"
    });

    try {
        while (true) {

            let balance = await Snip20GetBalance({
                secretjs: secretNetwork,
                token: process.env.CASH_TOKEN_ADDRESS,
                address: secretNetwork.senderAddress,
                key: "yo"
            });

            console.log(`balance: ${balance}`);

            for (let i = 0; i < 10; i++) {
                console.log(`**Depositing... ${i}`)
                await deposit(secretNetwork, 1000000);

                const tokenParams = await GetSnip20Params({secretjs: secretNetwork,
                    address: process.env.CASH_TOKEN_ADDRESS,
                });

                console.log(`**token total supply: ${tokenParams.total_supply}`)

                balance = await Snip20GetBalance({secretjs: secretNetwork,
                    token: process.env.CASH_TOKEN_ADDRESS,
                    address: secretNetwork.senderAddress,
                    key: "yo"
                });
                console.log(`**balance: ${balance}`);
            }

            const tokenParams = await GetSnip20Params({secretjs: secretNetwork,
                address: process.env.CASH_TOKEN_ADDRESS,
            });

            console.log(`token total supply: ${tokenParams.total_supply}`)

            balance = await Snip20GetBalance({secretjs: secretNetwork,
                token: process.env.CASH_TOKEN_ADDRESS,
                address: secretNetwork.senderAddress,
                key: "yo"
            });

            if (tokenParams.total_supply !== balance) {
                console.error(`Token supply does not match current balance: ${tokenParams.total_supply} vs ${balance}`);
            }

            console.log(`token balance: ${balance}`);

            const scrtBalanceBefore = (await secretNetwork.getAccount(secretNetwork.senderAddress)).balance[0];
            console.log(`scrt balance before: ${JSON.stringify(scrtBalanceBefore)}`)

            console.log(`Current time: ${Math.trunc( Date.now() / 1000)}`)
            let temp_balance = balance;
            for (let i = 0; i < 1000; i++) {
                console.log(`Withdrawing... ${i}`)

                await withdraw(secretNetwork, Math.floor(temp_balance / 2));
                temp_balance = temp_balance / 2;
            }

            console.log('Done withdraws');

            console.log(`Current time: ${Math.trunc( Date.now() / 1000)}`)
            const claimResultBefore = await queryClaim(secretNetwork);
            console.log(`pending claims: ${JSON.stringify(claimResultBefore)}`);
            const expectedBalance = claimResultBefore.pending_claims.pending[0].withdraw.coins.amount;

            await sleep(15000);

            while(Math.trunc( Date.now() / 1000) < claimResultBefore.pending_claims.pending[0].withdraw.available_time) {
                await sleep(1000);
            }

            await claim(secretNetwork);

            const claimResultAfter = await queryClaim(secretNetwork);
            console.log(`after claim: ${JSON.stringify(claimResultAfter)}`);

            const scrtBalanceAfter = (await secretNetwork.getAccount(secretNetwork.senderAddress)).balance[0];

            // - 500000 = 2 txs
            if (Number(scrtBalanceAfter.amount) + 500000 !== Number(expectedBalance) + Number(scrtBalanceBefore.amount)) {
                console.error(`Mismatched balances: ${scrtBalanceAfter.amount} !== ${Number(expectedBalance) + Number(scrtBalanceBefore.amount)} expected withdraw: ${expectedBalance} + before: ${scrtBalanceBefore.amount}`)
            } else {
                console.log('Withdrawn successfully')
            }
            console.log(
                "=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-="
            );
            await sleep(10000);
        }
    } catch (e) {
        console.log(e);
    }

})();