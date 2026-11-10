//! Herit estate: one contract per estate, at `<label>.herit-near.testnet`.
//!
//! Checkpoint 3: the liveness clock, Active → Grace → Unlocked, and never back from Unlocked.
//! Checkpoint 4: `check_in` with a World ID attestation, which restarts the clock.
//! Heirs, the vault and claims build on this in later checkpoints.

use near_sdk::json_types::{Base64VecU8, U64};
use near_sdk::store::LookupSet;
use near_sdk::{AccountId, BorshStorageKey, PanicOnDefault, env, near, require};

pub const ACTION_CHECKIN: u8 = 0;
const DOMAIN: &str = "herit-near:v1";
// Same bounds as HeritRegistry on Sepolia.
const MIN_CHECK_IN_INTERVAL_SEC: u32 = 120;
const MAX_CHECK_IN_INTERVAL_SEC: u32 = 90 * 86_400;
const MIN_GRACE_SEC: u32 = 60;
const MAX_GRACE_SEC: u32 = 14 * 86_400;
const NS_PER_SEC: u64 = 1_000_000_000;
/// Same 30-minute cap as Sepolia.
const MAX_ATTESTATION_LIFETIME_NS: u64 = 30 * 60 * NS_PER_SEC;

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Active,
    Grace,
    Unlocked,
}

/// What the attestor signs. Borsh is positional: this field order and these types must match
/// the signers exactly (the OutLayer worker, §7.1, and the JS dev signer, §7.5). The golden test
/// in `src/golden.rs` pins the bytes.
///
/// `estate_id` and `subject` are not arguments: `verify` fills them from the runtime, so a
/// signature only verifies for this estate and this caller.
#[near(serializers = [borsh])]
struct SignedPayload {
    domain: String,
    estate_id: AccountId,
    subject: AccountId,
    action: u8,
    heir_label: Option<String>,
    commitment: [u8; 32],
    nonce: u64,
    expiry_ns: u64,
}

/// The part of the signed payload the caller passes in, as JSON.
#[near(serializers = [json])]
#[derive(Clone, Debug)]
pub struct Attestation {
    pub action: u8,
    pub heir_label: Option<String>,
    /// 32 bytes identifying the human (from their World ID nullifier), base64.
    pub commitment: Base64VecU8,
    pub nonce: U64,
    pub expiry_ns: U64,
}

/// Storage prefixes. A variant's position is its prefix, so only ever append.
#[derive(BorshStorageKey)]
#[near]
enum Key {
    Nonces,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Estate {
    /// Whoever called `new`: the factory that created this account.
    factory: AccountId,
    grantor: AccountId,
    /// ed25519 key of the World ID attestor.
    attestor_pk: [u8; 32],
    /// The grantor's face commitment, bound on the first check-in.
    grantor_commitment: Option<[u8; 32]>,
    check_in_interval_ns: u64,
    grace_ns: u64,
    /// 0 = no check-in yet, so the clock has not started.
    last_check_in_ns: u64,
    /// Cache only, updated by `poke_expiry` and `check_in`. Guards read `pending_status()`.
    status: Status,
    /// Every attestation nonce accepted so far, for replay protection.
    used_nonces: LookupSet<u64>,
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
            used_nonces: LookupSet::new(Key::Nonces),
        }
    }

    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    // ---------- liveness ----------

    /// The grantor proves they're alive with a Selfie Check attestation. The first check-in binds
    /// their face commitment; later ones must match it. Allowed in Grace, which is the recovery
    /// path; refused once the clock says Unlocked, poked or not.
    pub fn check_in(&mut self, attestation: Attestation, signature: Base64VecU8) {
        self.only_grantor();
        self.not_unlocked();
        let c = self.verify(&attestation, &signature, ACTION_CHECKIN);
        match self.grantor_commitment {
            None => self.grantor_commitment = Some(c), // bind to the first face seen
            Some(bound) => require!(bound == c, "wrong human"), // = LivenessAttestor__WrongHuman
        }
        self.last_check_in_ns = env::block_timestamp();
        self.status = Status::Active;
        log_event(
            "checked_in",
            &format!(r#"{{"at":"{}"}}"#, self.last_check_in_ns),
        );
    }

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

    pub fn nonce_used(&self, nonce: U64) -> bool {
        self.used_nonces.contains(&nonce.0)
    }

    /// The grantor's bound face commitment, base64. `None` until the first check-in.
    pub fn commitment_of(&self) -> Option<Base64VecU8> {
        self.grantor_commitment.map(|c| Base64VecU8(c.to_vec()))
    }

    // ---------- internals ----------

    /// Checks an attestation for `action`, signed for this estate and this caller, and burns its
    /// nonce. Returns the commitment (who the human is).
    fn verify(&mut self, a: &Attestation, sig: &Base64VecU8, action: u8) -> [u8; 32] {
        require!(a.action == action, "wrong action");
        let now = env::block_timestamp();
        require!(a.expiry_ns.0 > now, "attestation expired");
        require!(
            a.expiry_ns.0 <= now + MAX_ATTESTATION_LIFETIME_NS,
            "attestation lives too long"
        );
        require!(!self.used_nonces.contains(&a.nonce.0), "nonce used");
        let commitment: [u8; 32] = a
            .commitment
            .0
            .as_slice()
            .try_into()
            .expect("commitment != 32 bytes");
        require!(commitment != [0u8; 32], "zero commitment");
        let msg = near_sdk::borsh::to_vec(&SignedPayload {
            domain: DOMAIN.into(),
            estate_id: env::current_account_id(),
            subject: env::predecessor_account_id(),
            action,
            heir_label: a.heir_label.clone(),
            commitment,
            nonce: a.nonce.0,
            expiry_ns: a.expiry_ns.0,
        })
        .unwrap();
        let sig: [u8; 64] = sig.0.as_slice().try_into().expect("signature != 64 bytes");
        require!(
            env::ed25519_verify(&sig, &msg, &self.attestor_pk),
            "invalid signature"
        );
        self.used_nonces.insert(a.nonce.0);
        commitment
    }

    fn not_unlocked(&self) {
        require!(self.pending_status() != Status::Unlocked, "estate unlocked");
    }

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
mod golden;

/// Checkpoint 4: check-in and attestations.
#[cfg(test)]
mod check_in_tests {
    use super::testing::*;
    use super::*;

    fn b64(bytes: [u8; 32]) -> Base64VecU8 {
        Base64VecU8(bytes.to_vec())
    }

    /// Submits `att` as the caller `att.subject` at `now`.
    fn submit(e: &mut Estate, att: &Att, now: u64) {
        set_context(att.subject.clone(), now);
        let (attestation, signature) = att.sign();
        e.check_in(attestation, signature);
    }

    #[test]
    fn first_check_in_binds_the_face_and_starts_the_clock() {
        let mut e = new_estate();
        check_in_at(&mut e, CHECKED_IN_AT, 1);

        assert_eq!(e.commitment_of(), Some(b64(GRANTOR_FACE)));
        assert!(e.nonce_used(U64(1)));
        assert!(!e.nonce_used(U64(2)));
        let grace_at = CHECKED_IN_AT + INTERVAL_SEC as u64 * NS_PER_SEC;
        let unlock_at = grace_at + GRACE_SEC as u64 * NS_PER_SEC;
        assert_eq!(e.deadlines_of(), (U64(grace_at), U64(unlock_at)));
        assert_eq!(e.status_of(), Status::Active);
    }

    #[test]
    fn a_second_check_in_from_the_same_face_moves_the_deadlines() {
        let mut e = new_estate();
        check_in_at(&mut e, CHECKED_IN_AT, 1);
        let later = CHECKED_IN_AT + 10 * 86_400 * NS_PER_SEC;
        check_in_at(&mut e, later, 2);

        assert_eq!(e.last_check_in_ns, later);
        assert_eq!(
            e.deadlines_of().0,
            U64(later + INTERVAL_SEC as u64 * NS_PER_SEC)
        );
        assert_eq!(e.commitment_of(), Some(b64(GRANTOR_FACE)));
    }

    #[test]
    #[should_panic(expected = "wrong human")]
    fn a_different_face_is_refused() {
        let mut e = new_estate();
        check_in_at(&mut e, CHECKED_IN_AT, 1);
        let mut att = Att::check_in(2, CHECKED_IN_AT);
        att.commitment = OTHER_FACE;
        submit(&mut e, &att, CHECKED_IN_AT);
    }

    #[test]
    #[should_panic(expected = "nonce used")]
    fn a_nonce_cannot_be_used_twice() {
        let mut e = new_estate();
        check_in_at(&mut e, CHECKED_IN_AT, 1);
        check_in_at(&mut e, CHECKED_IN_AT + NS_PER_SEC, 1);
    }

    #[test]
    #[should_panic(expected = "attestation expired")]
    fn an_expired_attestation_is_refused() {
        let mut e = new_estate();
        let mut att = Att::check_in(1, CHECKED_IN_AT);
        att.expiry_ns = CHECKED_IN_AT; // expiry must be strictly in the future
        submit(&mut e, &att, CHECKED_IN_AT);
    }

    #[test]
    #[should_panic(expected = "attestation lives too long")]
    fn an_attestation_over_30_minutes_is_refused() {
        let mut e = new_estate();
        let mut att = Att::check_in(1, CHECKED_IN_AT);
        att.expiry_ns = CHECKED_IN_AT + MAX_ATTESTATION_LIFETIME_NS + 1;
        submit(&mut e, &att, CHECKED_IN_AT);
    }

    #[test]
    fn an_attestation_of_exactly_30_minutes_is_accepted() {
        let mut e = new_estate();
        let mut att = Att::check_in(1, CHECKED_IN_AT);
        att.expiry_ns = CHECKED_IN_AT + MAX_ATTESTATION_LIFETIME_NS;
        submit(&mut e, &att, CHECKED_IN_AT);
        assert_eq!(e.commitment_of(), Some(b64(GRANTOR_FACE)));
    }

    #[test]
    #[should_panic(expected = "wrong action")]
    fn the_wrong_action_is_refused() {
        let mut e = new_estate();
        let mut att = Att::check_in(1, CHECKED_IN_AT);
        att.action = 1; // a claim attestation
        submit(&mut e, &att, CHECKED_IN_AT);
    }

    #[test]
    #[should_panic(expected = "invalid signature")]
    fn a_signature_for_another_estate_is_refused() {
        let mut e = new_estate();
        let mut att = Att::check_in(1, CHECKED_IN_AT);
        att.estate = other_estate_id();
        submit(&mut e, &att, CHECKED_IN_AT);
    }

    #[test]
    #[should_panic(expected = "invalid signature")]
    fn a_signature_for_another_caller_is_refused() {
        let mut e = new_estate();
        let mut att = Att::check_in(1, CHECKED_IN_AT);
        att.subject = stranger(); // signed for mallory...
        set_context(grantor(), CHECKED_IN_AT); // ...but submitted by the grantor
        let (attestation, signature) = att.sign();
        e.check_in(attestation, signature);
    }

    #[test]
    #[should_panic(expected = "invalid signature")]
    fn a_signature_from_another_key_is_refused() {
        let mut e = new_estate();
        let att = Att::check_in(1, CHECKED_IN_AT).signed_by_another_key();
        submit(&mut e, &att, CHECKED_IN_AT);
    }

    #[test]
    #[should_panic(expected = "not the grantor")]
    fn a_non_grantor_is_refused() {
        let mut e = new_estate();
        let mut att = Att::check_in(1, CHECKED_IN_AT);
        att.subject = stranger();
        submit(&mut e, &att, CHECKED_IN_AT);
    }

    #[test]
    fn a_check_in_in_grace_brings_the_estate_back_to_active() {
        let mut e = new_estate();
        check_in_at(&mut e, CHECKED_IN_AT, 1);
        let (grace_at, _) = e.deadlines_of();
        set_context(stranger(), grace_at.0);
        assert_eq!(e.poke_expiry(), Status::Grace);

        check_in_at(&mut e, grace_at.0, 2);
        assert_eq!(e.status_of(), Status::Active);
        assert_eq!(e.status, Status::Active);
        assert_eq!(e.last_check_in_ns, grace_at.0);
    }

    #[test]
    #[should_panic(expected = "estate unlocked")]
    fn a_check_in_after_an_unpoked_lapse_is_refused() {
        let mut e = new_estate();
        check_in_at(&mut e, CHECKED_IN_AT, 1);
        let (_, unlock_at) = e.deadlines_of();
        assert_eq!(e.status, Status::Active); // nobody poked
        check_in_at(&mut e, unlock_at.0, 2);
    }

    #[test]
    #[should_panic(expected = "zero commitment")]
    fn an_all_zero_commitment_is_refused() {
        let mut e = new_estate();
        let mut att = Att::check_in(1, CHECKED_IN_AT);
        att.commitment = [0; 32];
        submit(&mut e, &att, CHECKED_IN_AT);
    }

    #[test]
    #[should_panic(expected = "signature != 64 bytes")]
    fn a_short_signature_is_refused() {
        let mut e = new_estate();
        set_context(grantor(), CHECKED_IN_AT);
        let (attestation, mut signature) = Att::check_in(1, CHECKED_IN_AT).sign();
        signature.0.pop();
        e.check_in(attestation, signature);
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn new_stores_the_setup_and_starts_active() {
        let e = new_estate();
        assert_eq!(e.factory, factory());
        assert_eq!(e.grantor, grantor());
        assert_eq!(e.attestor_pk, attestor_pk_bytes());
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
            attestor_pk(),
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
            attestor_pk(),
            MAX_CHECK_IN_INTERVAL_SEC + 1,
            GRACE_SEC,
        );
    }

    #[test]
    #[should_panic(expected = "bad grace")]
    fn new_refuses_grace_below_the_minimum() {
        set_context(factory(), 0);
        Estate::new(grantor(), attestor_pk(), INTERVAL_SEC, MIN_GRACE_SEC - 1);
    }

    #[test]
    #[should_panic(expected = "bad grace")]
    fn new_refuses_grace_above_the_maximum() {
        set_context(factory(), 0);
        Estate::new(grantor(), attestor_pk(), INTERVAL_SEC, MAX_GRACE_SEC + 1);
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
