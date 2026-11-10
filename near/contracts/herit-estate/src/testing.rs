//! Shared helpers for the unit, fuzz and invariant tests.

use ed25519_dalek::{Signer, SigningKey};
use near_sdk::json_types::{Base64VecU8, U64};
use near_sdk::test_utils::VMContextBuilder;
use near_sdk::{AccountId, testing_env};

use super::{
    ACTION_CHECKIN, ACTION_CLAIM, ACTION_WITHDRAW, Asset, Attestation, DOMAIN, Estate, NS_PER_SEC,
    SignedPayload, Status,
};
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::json_types::U128;
use near_sdk::{NearToken, PromiseOrValue};

pub const INTERVAL_SEC: u32 = 30 * 86_400;
pub const GRACE_SEC: u32 = 7 * 86_400;
/// An arbitrary first check-in time, well after the epoch.
pub const CHECKED_IN_AT: u64 = 1_700_000_000 * 1_000_000_000;
/// How long a test attestation lives: comfortably under the 30-minute cap.
pub const LIFETIME_NS: u64 = 15 * 60 * NS_PER_SEC;
/// Stand-ins for two different humans' World ID commitments.
pub const GRANTOR_FACE: [u8; 32] = [1; 32];
pub const OTHER_FACE: [u8; 32] = [2; 32];
const ATTESTOR_SECRET: [u8; 32] = [7; 32];
const OTHER_SECRET: [u8; 32] = [8; 32];

pub fn estate_id() -> AccountId {
    "alice.herit-near.testnet".parse().unwrap()
}
pub fn other_estate_id() -> AccountId {
    "bob.herit-near.testnet".parse().unwrap()
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

/// The test attestor's public key bytes, which `new` stores.
pub fn attestor_pk_bytes() -> [u8; 32] {
    SigningKey::from_bytes(&ATTESTOR_SECRET)
        .verifying_key()
        .to_bytes()
}

/// The test attestor's public key in NEAR's `ed25519:<base58>` form, as `new` takes it.
pub fn attestor_pk() -> String {
    format!(
        "ed25519:{}",
        near_sdk::bs58::encode(attestor_pk_bytes()).into_string()
    )
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

/// Starts from empty storage, as a new account on chain would. Mocked storage otherwise lives for
/// the whole test thread, so one fuzz case's nonces would still be "used" in the next.
pub fn new_estate_with(interval_sec: u32, grace_sec: u32) -> Estate {
    set_context(factory(), 0);
    near_sdk::mock::with_mocked_blockchain(|b| drop(b.take_storage()));
    Estate::new(grantor(), attestor_pk(), interval_sec, grace_sec)
}

/// Everything an attestation is signed over, plus the signing key. Tests change one field to
/// build a bad attestation.
#[derive(Clone, Debug)]
pub struct Att {
    pub estate: AccountId,
    pub subject: AccountId,
    pub action: u8,
    pub heir_label: Option<String>,
    pub commitment: [u8; 32],
    pub nonce: u64,
    pub expiry_ns: u64,
    pub key: [u8; 32],
}

impl Att {
    /// A valid check-in for the grantor of `estate_id()`, made at `now_ns`.
    pub fn check_in(nonce: u64, now_ns: u64) -> Self {
        Self {
            estate: estate_id(),
            subject: grantor(),
            action: ACTION_CHECKIN,
            heir_label: None,
            commitment: GRANTOR_FACE,
            nonce,
            expiry_ns: now_ns + LIFETIME_NS,
            key: ATTESTOR_SECRET,
        }
    }

    pub fn signed_by_another_key(mut self) -> Self {
        self.key = OTHER_SECRET;
        self
    }

    /// Signs the payload the way the attestor does: Borsh-encode `SignedPayload`, then ed25519.
    pub fn sign(&self) -> (Attestation, Base64VecU8) {
        let msg = near_sdk::borsh::to_vec(&SignedPayload {
            domain: DOMAIN.into(),
            estate_id: self.estate.clone(),
            subject: self.subject.clone(),
            action: self.action,
            heir_label: self.heir_label.clone(),
            commitment: self.commitment,
            nonce: self.nonce,
            expiry_ns: self.expiry_ns,
        })
        .unwrap();
        let sig = SigningKey::from_bytes(&self.key).sign(&msg).to_bytes();
        (self.attestation(), Base64VecU8(sig.to_vec()))
    }

    /// The JSON arguments the caller passes, without signing.
    pub fn attestation(&self) -> Attestation {
        Attestation {
            action: self.action,
            heir_label: self.heir_label.clone(),
            commitment: Base64VecU8(self.commitment.to_vec()),
            nonce: U64(self.nonce),
            expiry_ns: U64(self.expiry_ns),
        }
    }
}

/// The grantor checks in at `now_ns` with a valid attestation.
pub fn check_in_at(estate: &mut Estate, now_ns: u64, nonce: u64) {
    set_context(grantor(), now_ns);
    let (attestation, signature) = Att::check_in(nonce, now_ns).sign();
    estate.check_in(attestation, signature);
}

/// Runs `f`, and if it panics, restores `estate` and its storage to how they were before the call,
/// the way a failed transaction reverts on chain. Returns whether the call succeeded.
///
/// Both halves matter: the struct's plain fields live in memory, while collections such as
/// `used_nonces` write to storage as soon as they change. Any write a collection still buffers is
/// saved when it's dropped, so the estate is reloaded first, and after a panic the half-finished
/// copy is forgotten rather than dropped.
pub fn call_reverting(estate: &mut Estate, f: impl FnOnce(&mut Estate)) -> bool {
    let before = near_sdk::borsh::to_vec(estate).unwrap();
    *estate = near_sdk::borsh::from_slice(&before).unwrap();
    let storage = snapshot_storage();
    let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(estate))).is_ok();
    if !ok {
        let failed = std::mem::replace(estate, near_sdk::borsh::from_slice(&before).unwrap());
        std::mem::forget(failed);
        restore_storage(&storage);
    }
    ok
}

type Storage = std::collections::HashMap<Vec<u8>, Vec<u8>>;

/// Copies the mocked storage. The mock can only hand storage out by emptying it, so it is
/// written straight back.
fn snapshot_storage() -> Storage {
    let storage = near_sdk::mock::with_mocked_blockchain(|b| b.take_storage());
    restore_storage(&storage);
    storage
}

fn restore_storage(storage: &Storage) {
    near_sdk::mock::with_mocked_blockchain(|b| drop(b.take_storage()));
    for (key, value) in storage {
        near_sdk::env::storage_write(key, value);
    }
}

// ---------- heirs (Checkpoint 6) ----------

/// Enough for any heir: the seed plus far more storage than one record needs.
pub const HEIR_DEPOSIT: near_sdk::NearToken = near_sdk::NearToken::from_near(1);

/// Like `set_context`, with `deposit` attached.
pub fn set_context_paying(caller: AccountId, now_ns: u64, deposit: near_sdk::NearToken) {
    testing_env!(
        VMContextBuilder::new()
            .current_account_id(estate_id())
            .predecessor_account_id(caller)
            .block_timestamp(now_ns)
            .attached_deposit(deposit)
            .build()
    );
}

/// The estate calling itself back, with the account creation's outcome as the promise result.
pub fn set_callback_context(created: bool) {
    let result = if created {
        near_sdk::PromiseResult::Successful(vec![])
    } else {
        near_sdk::PromiseResult::Failed
    };
    testing_env!(
        VMContextBuilder::new()
            .current_account_id(estate_id())
            .predecessor_account_id(estate_id())
            .build(),
        near_sdk::test_vm_config(),
        near_sdk::RuntimeFeesConfig::test(),
        Default::default(),
        vec![result],
    );
}

/// A distinct heir public key per `n`, in NEAR's `ed25519:<base58>` form.
pub fn heir_key(n: u8) -> near_sdk::PublicKey {
    let pk = SigningKey::from_bytes(&[100 + n; 32])
        .verifying_key()
        .to_bytes();
    format!("ed25519:{}", near_sdk::bs58::encode(pk).into_string())
        .parse()
        .unwrap()
}

/// `<label>.alice.herit-near.testnet`.
pub fn heir_id(label: &str) -> AccountId {
    format!("{label}.{}", estate_id()).parse().unwrap()
}

/// The grantor adds `label` with `bps`, paying `HEIR_DEPOSIT`, at `now_ns`.
pub fn add_heir_at(estate: &mut Estate, label: &str, bps: u16, now_ns: u64) {
    set_context_paying(grantor(), now_ns, HEIR_DEPOSIT);
    let _ = estate.add_heir(label.into(), heir_key(1), "son".into(), bps);
}

/// Adds `label` and runs the creation callback as a success.
pub fn add_created_heir(estate: &mut Estate, label: &str, bps: u16) {
    add_heir_at(estate, label, bps, 0);
    set_callback_context(true);
    assert!(estate.on_heir_created(label.into(), near_sdk::json_types::U128(0)));
}

// ---------- vault (Checkpoint 7) ----------

pub const ONE_NEAR: near_sdk::NearToken = near_sdk::NearToken::from_near(1);
pub const ONE_YOCTO: near_sdk::NearToken = near_sdk::NearToken::from_yoctonear(1);
pub const NEAR: super::Asset = None;

pub fn deposit_at(e: &mut Estate, caller: AccountId, amount: u128, now: u64) {
    set_context_paying(caller, now, NearToken::from_yoctonear(amount));
    e.deposit();
}

pub fn token(n: u32) -> AccountId {
    format!("token{n}.testnet").parse().unwrap()
}

pub fn usdc() -> AccountId {
    token(0)
}

/// The grantor adds `t`, paying enough.
pub fn add_token_at(e: &mut Estate, t: AccountId, now: u64) {
    set_context_paying(grantor(), now, NearToken::from_millinear(100));
    let _ = e.add_token(t);
}

/// Adds `t` and runs the registration callback as a success.
pub fn add_registered_token(e: &mut Estate, t: AccountId) {
    add_token_at(e, t.clone(), 0);
    set_callback_context(true);
    assert!(e.on_token_registered(t, U128(0)));
}

/// `token` delivers `amount` from `sender`, as `ft_transfer_call` does.
pub fn ft_deposit_at(
    e: &mut Estate,
    token: AccountId,
    sender: AccountId,
    amount: u128,
    now: u64,
) -> U128 {
    set_context(token, now);
    match e.ft_on_transfer(sender, U128(amount), String::new()) {
        PromiseOrValue::Value(unused) => unused,
        PromiseOrValue::Promise(_) => panic!("ft_on_transfer must answer at once"),
    }
}

/// A valid withdrawal attestation for the grantor's face, made at `now`.
pub fn withdraw_att(nonce: u64, now: u64) -> Att {
    let mut att = Att::check_in(nonce, now);
    att.action = ACTION_WITHDRAW;
    att
}

/// The grantor withdraws with `att`, attaching 1 yoctoNEAR.
pub fn withdraw_with(e: &mut Estate, asset: Asset, amount: u128, att: &Att, now: u64) {
    set_context_paying(att.subject.clone(), now, ONE_YOCTO);
    let (attestation, signature) = att.sign();
    let _ = e.withdraw(asset, U128(amount), attestation, signature);
}

// ---------- claims (Checkpoint 8) ----------

/// What a claim deposit covers: the commitment record's storage, with room to spare.
pub const CLAIM_DEPOSIT: NearToken = NearToken::from_millinear(10);
pub const SON_FACE: [u8; 32] = [3; 32];
pub const DAUGHTER_FACE: [u8; 32] = [4; 32];

/// Checked in, two heirs, 3 NEAR and 5 mUSDC deposited. Returns it with its unlock time.
pub fn funded_estate() -> (Estate, u64) {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 6_000);
    add_created_heir(&mut e, "daughter", 4_000);
    check_in_at(&mut e, CHECKED_IN_AT, 1);
    deposit_at(
        &mut e,
        grantor(),
        3 * ONE_NEAR.as_yoctonear(),
        CHECKED_IN_AT,
    );
    add_registered_token(&mut e, usdc());
    ft_deposit_at(&mut e, usdc(), grantor(), 5_000_000, CHECKED_IN_AT);
    let (_, unlock_at) = e.deadlines_of();
    (e, unlock_at.0)
}

pub fn poke_at(e: &mut Estate, now: u64) -> Status {
    set_context(stranger(), now);
    e.poke_expiry()
}

/// A valid claim for `label`, by `<label>.<estate>`, with `face`, made at `now`.
pub fn claim_att(label: &str, face: [u8; 32], nonce: u64, now: u64) -> Att {
    let mut att = Att::check_in(nonce, now);
    att.subject = heir_id(label);
    att.action = ACTION_CLAIM;
    att.heir_label = Some(label.into());
    att.commitment = face;
    att
}

/// `att.subject` claims `label` at `now`, attaching `deposit`.
pub fn claim_with(e: &mut Estate, label: &str, att: &Att, now: u64, deposit: NearToken) {
    set_context_paying(att.subject.clone(), now, deposit);
    let (attestation, signature) = att.sign();
    e.claim(label.into(), attestation, signature);
}
