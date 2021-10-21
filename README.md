## Staking derivatives (dSCRT)

### High level description

dSCRT (working title) is a staking derivative for the Secret Network. The purpose is to provide a liquid, fungible token for a user, representing his stake in the network. This token will accrue value, in a similar fashion to PoS staking which will unlock farming, and yield compounding opportunities. 
The goal is not to optimize APYs for users, but rather:
1.	Automatic compounding of rewards 
2.	Compounding of rewards without triggering a taxable event (if relevant)
3.	Minimizing validator risk by spreading delegations out to multiple validators
4.	Creating a mechanism for private governance voting
5.	Creating a fungible token which allows double-dipping in DeFi applications while still accruing network-level staking rewards
6.	Creating an asset class that accrues network-level staking rewards while still remaining liquid
7.	Allow “private staking”, whereby you can buy dSCRT on the market in a privacy preserving way without leaving a traceable transaction

### The main functions of dSCRT

#### Staking
1.	Stake SCRT, get dSCRT
2.	Withdraw (1 step). Unbond -> wait 21 days(*)
3.	Withdraw (2 steps). Unbond -> wait 21 days -> Claim

(*) if automatic claiming is enabled by the contract

#### Voting 
Secret voting is done by interacting with the dSCRT contract.
The contract admin opens voting for an on-chain proposal. The contract can be queried for open-proposals to see which proposals are in the voting period.
While the voting period is open, all holders of dSCRT gov token can place their votes. At the end of the voting period, the contract admin will call the tally function, and the votes will be tallied. The option with the highest vote count (each person has voting power based on their token holdings, similar to the on-chain voting) gets voted by the contract, with all it’s voting power. 

#### Trading
dSCRT is a SNIP-20 token, and can be sent or transferred to contracts or other users.

#### Fees
There is a 0.5%-1% (optional) developer fee on each deposit. There is a small deposit fee that helps incentivize users to trigger finalizing withdrawal windows (optional)

#### Withdrawal Windows
Cosmos chains (by default) have a limit of 7 active unbondings per address. This means that to be able to support many users dSCRT must group up the unbonds in withdrawal windows. These are 3 day sliding windows (customizable) during which all withdraws are aggregated. At the end of such a window, an unbond action is triggered, and the 21-day unbond counter is started. At the end of this time the released funds are distributed to users (user may also manually claim his SCRT if the contract has not yet been triggered).
This means the actual unbonding time is actually 21-24 days, depending on the withdrawal window

#### User stories and main functions

1.	A holder of SCRT wants to deposit his coins and receive dSCRT so that he can earn interest while remaining liquid
    *	User clicks stake button and selects an amount
    *	A confirmation screen is displayed, with the amount of SCRT deposited, and the number of tokens that will be received
    *	User clicks confirm
    *	Secret contract stake function is called
2.	A holder of dSCRT wants to withdraw his tokens so he can trade his SCRT
    *	User clicks the unbond button and selects an amount
    *	A confirmation screen is displayed, with the date the tokens will be available, as well as a warning that he will have to take further actions to claim his SCRT
    *	Secret contract unbond function is called
    *	The unbond is displayed for the user, including time to release and amount of SCRT tied to the unbond
    *	Withdraws should be automatically claimed for the user by the contract. However, in some cases a manual claim might be required. In such a case:
    *	When matured, the user can click a claim button, which claims all matured deposits
3.	A user wants to trigger withdrawal window finalization
    *	The amount of accumulated SCRT that will be rewarded to the caller is displayed
    *	The time till the withdrawal window closes is displayed
    *	After the window closes, a user may trigger a function call that closes the transfer window and awards him the SCRT in the reward pool
4.	A holder of dSCRT wants to see his current balance in tokens and in SCRT, so that he can track his position in a simple way
    *	There should be a display of current SCRT balance
    *	There should be a display of current dSCRT balance
5.	A holder of SCRT wants to see the current exchange rate, so that he can track the value of his dSCRT
    *	There should be a display of the exchange rate between SCRT<->dSCRT (this is available from the exchange_rate contract query)
6.	A holder of SCRT wants to be able to see some explanation about staking derivatives, and how it works
    *	A brief explanation in a landing page
    *	A more in-depth FAQ page

#### Gov token
1.	Optional governance token for dSCRT
2.	Governance token holders will share the fees generated by the deposit fee
3.	Governance token holders can vote on on-chain proposals (TBD – not sure if this is a good idea)
4.	Governance token holders will vote on new validators added to the pool, removed from the pool, or weight changes in the pool

#### Stakononics
1.	Validators can charge whatever commission they want, up to 10% (15%?) – anything over this amount and they will be removed from the validator set.
2.	Stake is split between validators according to weight. Validators can be weighted differently, but will start out with equal weights, unless otherwise specified
3.	The algorithm to balance stake between validators is: 
    *	On new deposit – Stake 100% to the validator that has the lowest amount to stake
    *	On new unbond – Unbond 100% from the validator that has the most stake. If there is remainder, unbond the remainder from the validator that has the next most stake. Repeat until the unbond has been fulfilled.
4.	Contract admin will charge a deposit fee of 1% (TBD). In the future, this fee may be repurposed for governance usages

#### Governance Tokenonics
1.	During initial distribution, governance token is distributed in a way that mirrors the distribution of dSCRT – users and holder of the staking derivative will be issues a matching amount of governance token
2.	After a period of time, the window for initial distribution (aka airdrop) will close, and further governance token distribution will continue according to a distribution plan (TBD)
