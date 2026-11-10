//! Herit estate: one contract per estate, at `<label>.herit-near.testnet`.
//!
//! Checkpoint 3 adds the liveness clock: Active → Grace → Unlocked, and never back from Unlocked.
//! Attestations, heirs, the vault and claims build on this in later checkpoints.

use near_sdk::json_types::U64;
use near_sdk::{AccountId, PanicOnDefault, env, near, require};

// Same bounds as HeritRegistry on Sepolia.
const MIN_CHECK_IN_INTERVAL_SEC: u32 = 120;
const MAX_CHECK_IN_INTERVAL_SEC: u32 = 90 * 86_400;
const MIN_GRACE_SEC: u32 = 60;
const MAX_GRACE_SEC: u32 = 14 * 86_400;
const NS_PER_SEC: u64 = 1_000_000_000;

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Active,
    Grace,
    Unlocked,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Estate {
    /// Whoever called `new`: the factory that created this account.
    factory: AccountId,
    grantor: AccountId,
    /// ed25519 key of the World ID attestor. Used from Checkpoint 4.
    attestor_pk: [u8; 32],
    /// The grantor's face commitment, bound on the first check-in. Used from Checkpoint 4.
    grantor_commitment: Option<[u8; 32]>,
    check_in_interval_ns: u64,
    grace_ns: u64,
    /// 0 = no check-in yet, so the clock has not started.
    last_check_in_ns: u64,
    /// Cache only, updated by `poke_expiry`. Guards read `pending_status()`.
    status: Status,
}

#[near]
impl Estate {
    /// Called once by the factory, in the same batch that creates this account.
    #[init]
    pub fn new(
        grantor: AccountId,
        attestor_pk: String,
        check_in_interval_sec: u32,
        grace_sec: u32,
    ) -> Self {
        validate_timers(check_in_interval_sec, grace_sec);
        log_event("estate_opened", &format!(r#"{{"grantor":"{grantor}"}}"#));
        Self {
            factory: env::predecessor_account_id(),
            grantor,
            attestor_pk: decode_ed25519(&attestor_pk),
            grantor_commitment: None,
            check_in_interval_ns: check_in_interval_sec as u64 * NS_PER_SEC,
            grace_ns: grace_sec as u64 * NS_PER_SEC,
            last_check_in_ns: 0, // the first check_in starts the clock, as on Sepolia
            status: Status::Active,
        }
    }

    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    // ---------- liveness ----------

    /// Permissionless, like `pokeExpiry()`. Stores the computed status. Idempotent.
    pub fn poke_expiry(&mut self) -> Status {
        let s = self.pending_status();
        if s == self.status {
            return s;
        }
        let now = env::block_timestamp();
        match s {
            Status::Grace => log_event("entered_grace", &format!(r#"{{"at":"{now}"}}"#)),
            // Checkpoint 8 adds the escrow snapshot here.
            Status::Unlocked => log_event("unlocked", &format!(r#"{{"at":"{now}"}}"#)),
            Status::Active => {}
        }
        self.status = s;
        s
    }

    // ---------- grantor setup ----------

    /// Retunes the timers. Active only: from Grace, a longer interval would recompute back to
    /// Active, which is recovery without a Selfie Check. Never moves `last_check_in_ns`.
    pub fn configure(&mut self, check_in_interval_sec: u32, grace_sec: u32) {
        self.only_grantor();
        require!(self.pending_status() == Status::Active, "estate not active");
        validate_timers(check_in_interval_sec, grace_sec);
        self.check_in_interval_ns = check_in_interval_sec as u64 * NS_PER_SEC;
        self.grace_ns = grace_sec as u64 * NS_PER_SEC;
        // A shorter interval moves the deadline back, maybe into the past. Never out of Active.
        require!(
            self.pending_status() == Status::Active,
            "timers would end Active"
        );
        log_event(
            "configured",
            &format!(r#"{{"interval_sec":{check_in_interval_sec},"grace_sec":{grace_sec}}}"#),
        );
    }

    // ---------- views ----------

    pub fn status_of(&self) -> Status {
        self.pending_status()
    }

    /// `(grace starts, unlock)` in nanoseconds. `(0, 0)` until the first check-in.
    pub fn deadlines_of(&self) -> (U64, U64) {
        if self.last_check_in_ns == 0 {
            return (U64(0), U64(0));
        }
        let grace_at = self.last_check_in_ns + self.check_in_interval_ns;
        (U64(grace_at), U64(grace_at + self.grace_ns))
    }

    // ---------- internals ----------

    /// The status the clock says right now, whatever the cache holds.
    fn pending_status(&self) -> Status {
        if self.status == Status::Unlocked {
            return Status::Unlocked; // one-way door
        }
        if self.last_check_in_ns == 0 {
            return Status::Active; // not started: must never read as lapsed
        }
        let now = env::block_timestamp();
        let grace_at = self.last_check_in_ns + self.check_in_interval_ns;
        if now >= grace_at + self.grace_ns {
            Status::Unlocked
        } else if now >= grace_at {
            Status::Grace
        } else {
            Status::Active
        }
    }

    fn only_grantor(&self) {
        require!(
            env::predecessor_account_id() == self.grantor,
            "not the grantor"
        );
    }
}

fn validate_timers(check_in_interval_sec: u32, grace_sec: u32) {
    require!(
        (MIN_CHECK_IN_INTERVAL_SEC..=MAX_CHECK_IN_INTERVAL_SEC).contains(&check_in_interval_sec),
        "bad interval"
    );
    require!(
        (MIN_GRACE_SEC..=MAX_GRACE_SEC).contains(&grace_sec),
        "bad grace"
    );
}

fn decode_ed25519(s: &str) -> [u8; 32] {
    let b58 = s
        .strip_prefix("ed25519:")
        .expect("expected ed25519:<base58>");
    near_sdk::bs58::decode(b58)
        .into_vec()
        .expect("bad base58")
        .try_into()
        .expect("ed25519 key must be 32 bytes")
}

/// NEP-297 event, readable by indexers and the activity feed.
fn log_event(event: &str, data: &str) {
    env::log_str(&format!(
        r#"EVENT_JSON:{{"standard":"herit","version":"1.0.0","event":"{event}","data":[{data}]}}"#
    ));
}

#[cfg(test)]
mod testing;

#[cfg(test)]
mod fuzz;

#[cfg(test)]
mod invariant;

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn new_stores_the_setup_and_starts_active() {
        let e = new_estate();
        assert_eq!(e.factory, factory());
        assert_eq!(e.grantor, grantor());
        assert_eq!(e.attestor_pk, [0u8; 32]);
        assert_eq!(e.check_in_interval_ns, INTERVAL_SEC as u64 * NS_PER_SEC);
        assert_eq!(e.grace_ns, GRACE_SEC as u64 * NS_PER_SEC);
        assert_eq!(e.status_of(), Status::Active);
        assert_eq!(e.deadlines_of(), (U64(0), U64(0)));
    }

    #[test]
    #[should_panic(expected = "bad interval")]
    fn new_refuses_an_interval_below_the_minimum() {
        set_context(factory(), 0);
        Estate::new(
            grantor(),
            ATTESTOR_PK.into(),
            MIN_CHECK_IN_INTERVAL_SEC - 1,
            GRACE_SEC,
        );
    }

    #[test]
    #[should_panic(expected = "bad interval")]
    fn new_refuses_an_interval_above_the_maximum() {
        set_context(factory(), 0);
        Estate::new(
            grantor(),
            ATTESTOR_PK.into(),
            MAX_CHECK_IN_INTERVAL_SEC + 1,
            GRACE_SEC,
        );
    }

    #[test]
    #[should_panic(expected = "bad grace")]
    fn new_refuses_grace_below_the_minimum() {
        set_context(factory(), 0);
        Estate::new(
            grantor(),
            ATTESTOR_PK.into(),
            INTERVAL_SEC,
            MIN_GRACE_SEC - 1,
        );
    }

    #[test]
    #[should_panic(expected = "bad grace")]
    fn new_refuses_grace_above_the_maximum() {
        set_context(factory(), 0);
        Estate::new(
            grantor(),
            ATTESTOR_PK.into(),
            INTERVAL_SEC,
            MAX_GRACE_SEC + 1,
        );
    }

    #[test]
    #[should_panic(expected = "ed25519 key must be 32 bytes")]
    fn new_refuses_a_malformed_attestor_key() {
        set_context(factory(), 0);
        Estate::new(grantor(), "ed25519:abc".into(), INTERVAL_SEC, GRACE_SEC);
    }

    #[test]
    fn no_check_in_never_lapses() {
        let e = new_estate();
        set_context(stranger(), 10 * 365 * 86_400 * NS_PER_SEC);
        assert_eq!(e.status_of(), Status::Active);
    }

    #[test]
    fn grace_and_unlock_start_exactly_at_their_deadlines() {
        let mut e = new_estate();
        e.last_check_in_ns = CHECKED_IN_AT;
        let (grace_at, unlock_at) = e.deadlines_of();

        set_context(stranger(), grace_at.0 - 1);
        assert_eq!(e.status_of(), Status::Active);
        set_context(stranger(), grace_at.0);
        assert_eq!(e.status_of(), Status::Grace);
        set_context(stranger(), unlock_at.0 - 1);
        assert_eq!(e.status_of(), Status::Grace);
        set_context(stranger(), unlock_at.0);
        assert_eq!(e.status_of(), Status::Unlocked);
    }

    #[test]
    fn status_follows_the_clock_and_poke_caches_it() {
        let mut e = new_estate();
        e.last_check_in_ns = CHECKED_IN_AT;
        let (grace_at, _) = e.deadlines_of();

        set_context(stranger(), grace_at.0);
        assert_eq!(e.status_of(), Status::Grace); // computed, no poke needed
        assert_eq!(e.status, Status::Active); // cache not yet updated

        assert_eq!(e.poke_expiry(), Status::Grace);
        assert_eq!(e.status, Status::Grace);
        assert_eq!(e.poke_expiry(), Status::Grace); // idempotent
        assert_eq!(e.status, Status::Grace);
    }

    #[test]
    fn unlocked_is_irreversible() {
        let mut e = new_estate();
        e.last_check_in_ns = CHECKED_IN_AT;
        let (_, unlock_at) = e.deadlines_of();

        set_context(stranger(), unlock_at.0);
        assert_eq!(e.poke_expiry(), Status::Unlocked);

        set_context(stranger(), CHECKED_IN_AT); // clock back before the deadline
        assert_eq!(e.status_of(), Status::Unlocked);
        assert_eq!(e.poke_expiry(), Status::Unlocked);
    }

    #[test]
    #[should_panic(expected = "not the grantor")]
    fn configure_refuses_a_non_grantor() {
        let mut e = new_estate();
        set_context(stranger(), 0);
        e.configure(INTERVAL_SEC, GRACE_SEC);
    }

    #[test]
    #[should_panic(expected = "estate not active")]
    fn configure_is_refused_in_grace() {
        let mut e = new_estate();
        e.last_check_in_ns = CHECKED_IN_AT;
        let (grace_at, _) = e.deadlines_of();
        set_context(grantor(), grace_at.0);
        e.configure(MAX_CHECK_IN_INTERVAL_SEC, GRACE_SEC); // would recompute back to Active
    }

    #[test]
    #[should_panic(expected = "estate not active")]
    fn configure_is_refused_when_unlocked() {
        let mut e = new_estate();
        e.last_check_in_ns = CHECKED_IN_AT;
        let (_, unlock_at) = e.deadlines_of();
        set_context(grantor(), unlock_at.0);
        e.configure(INTERVAL_SEC, GRACE_SEC);
    }

    #[test]
    #[should_panic(expected = "timers would end Active")]
    fn configure_cannot_push_the_estate_past_its_deadline() {
        let mut e = new_estate();
        e.last_check_in_ns = CHECKED_IN_AT;
        // Still Active under the current interval, but past the minimum interval's deadline.
        set_context(
            grantor(),
            CHECKED_IN_AT + (MIN_CHECK_IN_INTERVAL_SEC as u64 + 1) * NS_PER_SEC,
        );
        e.configure(MIN_CHECK_IN_INTERVAL_SEC, GRACE_SEC);
    }

    #[test]
    fn configure_moves_the_deadlines_but_not_the_last_check_in() {
        let mut e = new_estate();
        e.last_check_in_ns = CHECKED_IN_AT;
        set_context(grantor(), CHECKED_IN_AT + NS_PER_SEC);

        let new_interval = INTERVAL_SEC * 2;
        let new_grace = GRACE_SEC * 2;
        e.configure(new_interval, new_grace);

        assert_eq!(e.last_check_in_ns, CHECKED_IN_AT);
        let grace_at = CHECKED_IN_AT + new_interval as u64 * NS_PER_SEC;
        let unlock_at = grace_at + new_grace as u64 * NS_PER_SEC;
        assert_eq!(e.deadlines_of(), (U64(grace_at), U64(unlock_at)));
    }
}
