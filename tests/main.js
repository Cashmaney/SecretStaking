const {
    SigningCosmWasmClient,
    Secp256k1Pen,
    pubkeyToAddress,
    encodeSecp256k1Pubkey,
} = require("secretjs");
const axios = require("axios");

require("dotenv").config();

const symbols = ["BTC", "DAI", "ETH"];

const customFees = {
    exec: {
        amount: [{ amount: "37500", denom: "uscrt" }],
        gas: "150000",
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

const queryClaim = async (amount) => {
    try {
        return secretNetwork.queryContractSmart(
            process.env.CASH_CONTRACT_ADDRESS,
            {
                claim: {},
            },
        );
    } catch (e) {
        console.log(`Failed to deposit ${e}`);
    }
    return null;
}

const claim = async (amount) => {
    try {
        return secretNetwork.execute(
            process.env.CASH_CONTRACT_ADDRESS,
            {
                claim: {},
            },
            "",
            [{ amount: String(amount), denom: "uscrt" }],
        );
    } catch (e) {
        console.log(`Failed to deposit ${e}`);
    }
    return null;
}

const deposit = async (amount) => {
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

const withdraw = async (amount) => {
    try {
        return secretNetwork.execute(
            process.env.CASH_TOKEN_ADDRESS,
            {
                send: {recipient: process.env.CASH_CONTRACT_ADDRESS, amount: "", msg: "eyJ3aXRoZHJhdyI6IHt9fQ"},
            },
            "",
            [{ amount: String(amount), denom: "uscrt" }],
        );
    } catch (e) {
        console.log(`Failed to withdraw ${e}`);
    }
    return null;
}

const deposit = async () => {
    try {
        const rawResults = await axios({
            method: "post",
            url: process.env.BAND_URL,
            headers: {
                "Content-Type": "application/json",
            },
            data: JSON.stringify({ symbols, min_count: 10, ask_count: 16 }),
        }).then((r) => r.data["result"]);

        let rates = [];
        let resolve_times = [];
        let request_ids = [];

        for ({ multiplier, px, request_id, resolve_time } of rawResults) {
            rates.push(px);
            resolve_times.push(Number(resolve_time));
            request_ids.push(Number(request_id));
        }

        return { symbols, rates, resolve_times, request_ids };
    } catch (e) {
        console.log(e);
        return null;
    }
};

const getCurrentRateFromProxyContract = async (secretNetwork) => {
    try {
        return secretNetwork.queryContractSmart(
            process.env.PROXY_CONTRACT_ADDRESS,
            {
                get_reference_data_bulk: {
                    base_symbols: symbols,
                    quote_symbols: Array(symbols.length).fill("USD"),
                },
            }
        );
    } catch (e) {
        console.log("Fail to get rate from proxy contract");
        console.log(e);
    }
    return null;
};

const sendRelayTx = async (secretNetwork, data) => {
    return await secretNetwork.execute(process.env.BASE_CONTRACT_ADDRESS, {
        relay: data,
    });
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

(async () => {
    const secretNetwork = await createCli();

    const chainId = await secretNetwork.getChainId();
    const height = await secretNetwork.getHeight();

    console.log("chainId: ", chainId);
    console.log("height: ", height);

    while (true) {
        try {
            const relayData = await getPricesFromBand();
            if (relayData) {
                console.log("\nrelay message: ", JSON.stringify({ relayData }));
            } else {
                throw "Fail to get prices from band";
            }

            // broadcast tx
            const resp = await sendRelayTx(secretNetwork, relayData);
            console.log("broadcast : ", resp);

            const { transactionHash } = resp;
            const txResult = await validateTx(secretNetwork, transactionHash);
            console.log("\n");
            if (!txResult) {
                throw "Fail to get result from chain";
            }

            if (!txResult.code) {
                console.log("tx successfully send!");
            } else {
                throw "Fail to send tx with result: " + JSON.stringify(txResult);
            }

            const currentRates = await getCurrentRateFromProxyContract(secretNetwork);
            if (currentRates) {
                console.log("current rates: ", JSON.stringify(currentRates));
            } else {
                throw "Fail to get current rates from proxy contract";
            }
        } catch (e) {
            console.log(e);
        }
        console.log(
            "=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-="
        );
        await sleep(10000);
    }
})();
