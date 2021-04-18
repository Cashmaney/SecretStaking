// const { CosmWasmClient, ExecuteResult, SigningCosmWasmClient } = require('secretjs');
// const { StdFee } = require('secretjs/types/types');

const Snip20SwapHash = ({ tx_id, address }) => {
    return `${tx_id}|${address}`;
};

//
// export interface Snip20TokenInfo {
//     name: string;
//     symbol: string;
//     decimals: number;
//     total_supply?: string;
// }

const GetSnip20Params = async ({
    secretjs,
    address,
}) => {

    try {
        const paramsResponse = await secretjs.queryContractSmart(address, { token_info: {} });

        return {
            name: paramsResponse.token_info.name,
            symbol: paramsResponse.token_info.symbol,
            decimals: paramsResponse.token_info.decimals,
            total_supply: paramsResponse.token_info.total_supply,
        };
    } catch (e) {
        throw Error('Failed to get info');
    }
};

const Snip20GetBalance = async ({
    secretjs,
    token,
    address,
    key,
}) => {

    let balanceResponse;
    try {
        balanceResponse = await secretjs.queryContractSmart(token, {
            balance: {
                address: address,
                key,
            },
        });
    } catch (e) {
        console.log(e);
        return 'Unlock';
    }

    if (balanceResponse.viewing_key_error) {
        return 'Fix Unlock';
    }

    if (Number(balanceResponse.balance.amount) === 0) {
        return '0';
    }
    return balanceResponse.balance.amount;
};

const Snip20SetViewingKey = async ({
                                     secretjs,
                                     address,
                                     key,
                                 }) => {

    return await secretjs.execute(
        address,
        {
            set_viewing_key: {
                key
            },
        }
    );
};

const Snip20Send = async ({
    secretjs,
    address,
    amount,
    msg,
    recipient,
    fee,
}) => {
    //const { secretjs, address, amount, msg, recipient, fee } = params;

    return await secretjs.execute(
        address,
        {
            send: {
                amount,
                recipient,
                msg,
            },
        },
        '',
        [],
        fee,
    );
};

const GetContractCodeHash = async ({
    secretjs,
    address,
}) => {
    return await secretjs.getCodeHashByContractAddr(address);
};

module.exports = {Snip20SetViewingKey, GetContractCodeHash, GetSnip20Params, Snip20GetBalance, Snip20Send};