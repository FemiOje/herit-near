//! Shared helpers for the unit, fuzz and invariant tests.

use near_sdk::test_utils::VMContextBuilder;
use near_sdk::{AccountId, testing_env};

use super::Estate;

/// 32 zero bytes in base58.
pub const ATTESTOR_PK: &str = "ed25519:11111111111111111111111111111111";
pub const INTERVAL_SEC: u32 = 30 * 86_400;
pub const GRACE_SEC: u32 = 7 * 86_400;
/// An arbitrary first check-in time, well after the epoch.
pub const CHECKED_IN_AT: u64 = 1_700_000_000 * 1_000_000_000;

pub fn estate_id() -> AccountId {
    "alice.herit-near.testnet".parse().unwrap()
}
pub fn factory() -> AccountId {
    "herit-near.testnet".parse().unwrap()
}
pub fn grantor() -> AccountId {
    "alice-kaykay.testnet".parse().unwrap()
}
pub fn stranger() -> AccountId {
    "mallory.testnet".parse().unwrap()
}

/// Sets who is calling (`predecessor`) and the block time in nanoseconds.
pub fn set_context(caller: AccountId, now_ns: u64) {
    testing_env!(
        VMContextBuilder::new()
            .current_account_id(estate_id())
            .predecessor_account_id(caller)
            .block_timestamp(now_ns)
            .build()
    );
}

/// An estate opened by the factory with the default timers, clock not started.
pub fn new_estate() -> Estate {
    new_estate_with(INTERVAL_SEC, GRACE_SEC)
}

pub fn new_estate_with(interval_sec: u32, grace_sec: u32) -> Estate {
    set_context(factory(), 0);
    Estate::new(grantor(), ATTESTOR_PK.into(), interval_sec, grace_sec)
}

/// Runs `f`, and if it panics, restores `estate` to its state before the call, the way a failed
/// transaction reverts on chain. Returns whether the call succeeded.
pub fn call_reverting(estate: &mut Estate, f: impl FnOnce(&mut Estate)) -> bool {
    let before = near_sdk::borsh::to_vec(estate).unwrap();
    let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(estate))).is_ok();
    if !ok {
        *estate = near_sdk::borsh::from_slice(&before).unwrap();
    }
    ok
}
