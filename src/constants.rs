pub const AMOUNT_OF_SHARED_WITHDRAWS: u32 = 5;
pub const AMOUNT_OF_REWARDS_TO_HANDLE: u32 = 2;
// -- 21 days + 2 minutes (buffer to make sure unbond will be matured)
pub(crate) const UNBONDING_TIME: u64 = 3600 * 24 * 21 + 120;
// pub const UNBONDING_TIME: u64 = 25;
pub const CASH_TOKEN_SYMBOL: &str = "dSCRT";