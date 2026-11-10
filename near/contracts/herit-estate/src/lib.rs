//! Herit estate: one contract per estate, at `<label>.herit-near.testnet`.
//!
//! Checkpoint 3: the liveness clock, Active → Grace → Unlocked, and never back from Unlocked.
//! Checkpoint 4: `check_in` with a World ID attestation, which restarts the clock.
//! Checkpoint 6: heirs, each a real account `<label>.<estate>` that only the heir holds a key to.
//! Checkpoint 7: the vault: NEAR and NEP-141 deposits, each heir's share of each asset, and
//! withdrawals that need a fresh Selfie Check.
//! Checkpoint 8: claims: the snapshot at unlock, who may claim, and payouts.
//! Checkpoint 9: each heir's soulbound, dynamic NEP-171 credential (`nft.rs`).
//! Claims build on this in later checkpoints.

use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::json_types::{Base64VecU8, U64, U128};
use near_sdk::store::{IterableMap, LookupMap, LookupSet, Vector};
use near_sdk::{
    AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise, PromiseOrValue, PublicKey,
    assert_one_yocto, env, ext_contract, is_promise_success, near, require,
};

mod nft;

pub const ACTION_CHECKIN: u8 = 0;
/// A claim: the heir's own Selfie Check, naming their label.
pub const ACTION_CLAIM: u8 = 1;
/// A withdrawal: the grantor's own Selfie Check, so a stolen key alone can't move funds.
pub const ACTION_WITHDRAW: u8 = 2;
const DOMAIN: &str = "herit-near:v1";
// Same bounds as HeritRegistry on Sepolia.
const MIN_CHECK_IN_INTERVAL_SEC: u32 = 120;
const MAX_CHECK_IN_INTERVAL_SEC: u32 = 90 * 86_400;
const MIN_GRACE_SEC: u32 = 60;
const MAX_GRACE_SEC: u32 = 14 * 86_400;
const NS_PER_SEC: u64 = 1_000_000_000;
/// Same 30-minute cap as Sepolia.
const MAX_ATTESTATION_LIFETIME_NS: u64 = 30 * 60 * NS_PER_SEC;
/// 100% in basis points.
const BPS: u32 = 10_000;
/// Keeps every loop over heirs, and a claim's payouts, well inside the gas limit.
const MAX_HEIRS: u32 = 10;
/// Sent to each new heir account: its storage, plus gas for the heir's own claim later.
const HEIR_ACCOUNT_SEED: NearToken = NearToken::from_millinear(100);
const MAX_RELATIONSHIP_LEN: usize = 32;
const HEIR_CALLBACK_GAS: Gas = Gas::from_tgas(10);
/// Ceiling on distinct assets per estate, counting NEAR, as `MAX_TOKENS` in `HeritVault`. The
/// unlock snapshot and every claim loop over the whole list, so it must stay small.
pub const MAX_TOKENS: u32 = 10;
/// Ceiling on distinct assets with at least one share override, as in `HeritRegistry`.
/// `add_heir` walks the list, so it must stay small.
pub const MAX_OVERRIDE_TOKENS: u32 = 10;

/// What a NEP-145 token charges to register an account; 0.00125 NEAR covers the standard
/// implementation. Any excess is refunded by the token to the estate.
const TOKEN_REGISTRATION: NearToken = NearToken::from_micronear(1_250);
const TOKEN_CALL_GAS: Gas = Gas::from_tgas(10);
/// A payout callback only updates a record and logs. Kept small so a whole claim fits in what the
/// heir's 0.1 NEAR seed can prepay.
const PAYOUT_CALLBACK_GAS: Gas = Gas::from_tgas(5);

/// An asset the vault holds: `None` is native NEAR (`NATIVE` in `HeritVault`), `Some(token)` a
/// NEP-141 token contract.
pub type Asset = Option<AccountId>;

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

/// One heir, stored under its label: `son` for `son.<estate>`.
#[near(serializers = [borsh])]
#[derive(Clone, Debug, PartialEq)]
pub struct Heir {
    pub account_id: AccountId,
    /// Free text the grantor chooses, such as "son". Shown on the heir's credential.
    pub relationship: String,
    /// The heir's share of each asset in basis points, unless overridden per asset (Checkpoint 7).
    pub default_bps: u16,
    /// False until the heir's account exists. Until then the heir can't be removed, so the
    /// creation callback always finds the record it was made for.
    pub created: bool,
}

/// An heir as the views return it.
#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq)]
pub struct HeirView {
    pub label: String,
    pub account_id: AccountId,
    pub relationship: String,
    pub default_bps: u16,
    pub created: bool,
}

/// Storage prefixes. A variant's position is its prefix, so only ever append.
#[derive(BorshStorageKey)]
#[near]
enum Key {
    Nonces,
    Heirs,
    Balances,
    Tokens,
    Shares,
    OverrideAssets,
    Snapshots,
    ClaimCommitments,
    Paid,
}

/// The NEP-141 and NEP-145 calls the estate makes on a token contract.
#[ext_contract(ext_ft)]
#[allow(dead_code)]
trait FungibleToken {
    fn storage_deposit(&mut self, account_id: Option<AccountId>, registration_only: Option<bool>);
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
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
    /// label -> heir. At most `MAX_HEIRS`.
    heirs: IterableMap<String, Heir>,
    /// What the estate holds of each asset (`s_balances` in `HeritVault`). Counted here, never
    /// read from the account balance, which also pays for storage and collects gas rewards.
    balances: LookupMap<Asset, u128>,
    /// Every asset the estate has held, in the order first deposited (`s_tokens`). Never pruned:
    /// an asset withdrawn to zero stays listed. At most `MAX_TOKENS`.
    tokens: Vector<Asset>,
    /// (label, asset) -> bps: an heir's share of one asset, overriding their `default_bps`
    /// (`s_share` + `s_hasOverride` in `HeritRegistry`: an entry exists exactly when overridden).
    shares: LookupMap<(String, Asset), u16>,
    /// Every asset with at least one override (`s_overriddenTokens`), so `add_heir` can check a new
    /// heir's default against each asset's own total. At most `MAX_OVERRIDE_TOKENS`.
    override_assets: Vector<Asset>,
    /// What each balance was at unlock (`s_snapshots` in `HeritVault`). Every share is a percentage
    /// of this, so heirs claiming at different times split the same total. An entry is created,
    /// at 0, when its asset is listed, so taking the snapshot writes no new storage.
    snapshots: LookupMap<Asset, u128>,
    /// `s_snapshotTaken`: the balances are frozen. Set once, at unlock.
    snapshot_taken: bool,
    /// commitment -> the label it claimed for (`s_claimUsed` in `LivenessAttestor`). One human,
    /// one heir slot; the same human may claim again for the same label, to retry a failed payout.
    claim_commitments: LookupMap<[u8; 32], String>,
    /// (label, asset) pairs already paid (`s_paid` in `ClaimManager`). A pair is unmarked again if
    /// its transfer fails, so the heir can retry.
    paid: LookupSet<(String, Asset)>,
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
            heirs: IterableMap::new(Key::Heirs),
            balances: LookupMap::new(Key::Balances),
            tokens: Vector::new(Key::Tokens),
            shares: LookupMap::new(Key::Shares),
            override_assets: Vector::new(Key::OverrideAssets),
            snapshots: LookupMap::new(Key::Snapshots),
            snapshot_taken: false,
            claim_commitments: LookupMap::new(Key::ClaimCommitments),
            paid: LookupSet::new(Key::Paid),
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
            // `_unlock`: freeze the balances before any heir can claim.
            Status::Unlocked => {
                self.snapshot();
                log_event(
                    "unlocked",
                    &format!(r#"{{"at":"{now}","heirs":{}}}"#, self.heirs.len()),
                );
            }
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

    // ---------- vault ----------

    /// The grantor puts NEAR into the estate (`depositETH`). Allowed until unlock, Grace included.
    ///
    /// Credits what arrives, as `HeritVault` does: the first deposit lists NEAR, and the storage
    /// that listing takes comes out of the amount credited. So the books never promise the heirs
    /// NEAR the estate has spent on storage.
    #[payable]
    pub fn deposit(&mut self) {
        self.only_grantor();
        self.not_unlocked();
        let amount = env::attached_deposit().as_yoctonear();
        require!(amount > 0, "attach the NEAR to deposit");
        let storage_cost = self.list_asset(&None);
        require!(
            amount > storage_cost,
            "deposit too small to cover its storage"
        );
        let credited = amount - storage_cost;
        self.credit(&None, credited);
        log_event(
            "deposited",
            &format!(r#"{{"asset":"near","amount":"{credited}"}}"#),
        );
    }

    /// Lists a NEP-141 token and registers the estate with it, so the grantor can deposit it.
    ///
    /// The NEAR-only step before `depositERC20`'s first deposit: an account must be registered with
    /// a token contract (and pay that storage) before it can receive the token, and a token
    /// transfer can't carry the NEAR for it. Attach the listing's storage plus
    /// `TOKEN_REGISTRATION`; the surplus is refunded. Counts towards `MAX_TOKENS`. If registration
    /// fails, `on_token_registered` unlists the token and refunds the grantor.
    #[payable]
    pub fn add_token(&mut self, token: AccountId) -> Promise {
        self.only_grantor();
        self.not_unlocked();
        let asset = Some(token.clone());
        require!(!self.is_listed(&asset), "token already added");
        let storage_cost = self.list_asset(&asset);
        let registration = TOKEN_REGISTRATION.as_yoctonear();
        let paid = env::attached_deposit().as_yoctonear();
        require!(
            paid >= storage_cost + registration,
            "attach more NEAR for storage and the token registration"
        );
        if paid > storage_cost + registration {
            Promise::new(env::predecessor_account_id())
                .transfer(NearToken::from_yoctonear(
                    paid - storage_cost - registration,
                ))
                .detach();
        }
        ext_ft::ext(token.clone())
            .with_attached_deposit(TOKEN_REGISTRATION)
            .with_static_gas(TOKEN_CALL_GAS)
            .storage_deposit(Some(env::current_account_id()), Some(true))
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(TOKEN_CALL_GAS)
                    .on_token_registered(token, U128(storage_cost + registration)),
            )
    }

    /// Runs after the registration. On failure the registration fee has come back to this estate,
    /// so the token is unlisted and `refund` goes to the grantor.
    #[private]
    pub fn on_token_registered(&mut self, token: AccountId, refund: U128) -> bool {
        if is_promise_success() {
            log_event("token_added", &format!(r#"{{"token":"{token}"}}"#));
            true
        } else {
            self.unlist_asset(&Some(token.clone()));
            Promise::new(self.grantor.clone())
                .transfer(NearToken::from_yoctonear(refund.0))
                .detach();
            log_event("token_failed", &format!(r#"{{"token":"{token}"}}"#));
            false
        }
    }

    /// The grantor takes `amount` of `asset` (`null` for NEAR) back out (`withdraw` in
    /// `HeritVault`). Grantor only, before unlock, Grace included: the way out for a grantor who
    /// returns after a missed check-in.
    ///
    /// New on NEAR: it also needs a fresh `ACTION_WITHDRAW` Selfie Check from the grantor's bound
    /// face, so a stolen key alone can move nothing. Attach exactly 1 yoctoNEAR, which only a
    /// full-access key can do and which pays the token transfer's own 1 yoctoNEAR.
    ///
    /// The balance is debited before the money moves (`_debit` before `_send`). The transfer is
    /// asynchronous, so if it fails `on_withdrawn` credits the amount back.
    #[payable]
    pub fn withdraw(
        &mut self,
        asset: Asset,
        amount: U128,
        attestation: Attestation,
        signature: Base64VecU8,
    ) -> Promise {
        assert_one_yocto();
        self.only_grantor();
        self.not_unlocked();
        require!(
            attestation.heir_label.is_none(),
            "a withdrawal names no heir"
        );
        let face = self.verify(&attestation, &signature, ACTION_WITHDRAW);
        match self.grantor_commitment {
            None => env::panic_str("check in first: no face is bound yet"),
            Some(bound) => require!(bound == face, "wrong human"),
        }
        require!(amount.0 > 0, "zero amount");
        let available = self.balances.get(&asset).copied().unwrap_or(0);
        require!(amount.0 <= available, "insufficient balance");
        self.balances.insert(asset.clone(), available - amount.0);

        let grantor = self.grantor.clone();
        let send = match &asset {
            None => Promise::new(grantor).transfer(NearToken::from_yoctonear(amount.0)),
            Some(token) => ext_ft::ext(token.clone())
                .with_attached_deposit(NearToken::from_yoctonear(1))
                .with_static_gas(TOKEN_CALL_GAS)
                .ft_transfer(grantor, amount, Some("herit withdrawal".into())),
        };
        send.then(
            Self::ext(env::current_account_id())
                .with_static_gas(TOKEN_CALL_GAS)
                .on_withdrawn(asset, amount),
        )
    }

    /// Runs after the transfer. Logs `withdrawn`, or credits the amount back and logs
    /// `withdraw_failed`.
    #[private]
    pub fn on_withdrawn(&mut self, asset: Asset, amount: U128) -> bool {
        let asset_name = asset.clone().map_or("near".to_string(), |t| t.to_string());
        let data = format!(r#"{{"asset":"{asset_name}","amount":"{}"}}"#, amount.0);
        if is_promise_success() {
            log_event("withdrawn", &data);
            true
        } else {
            self.credit(&asset, amount.0);
            // Sent before unlock, failed after it: the amount is back in the estate, so it joins
            // the snapshot and the heirs still to claim share it rather than it being stranded.
            if self.snapshot_taken {
                *self.snapshots.get_mut(&asset).expect("asset not listed") += amount.0;
            }
            log_event("withdraw_failed", &data);
            false
        }
    }

    // ---------- claims ----------

    /// An heir claims their share (`LivenessAttestor.claim` + `ClaimManager.claim`). Sent from the
    /// heir's own account, `<label>.<estate>`, with their own `ACTION_CLAIM` Selfie Check naming
    /// `label`. Pokes the estate first, so the first claim after the deadline unlocks it and takes
    /// the snapshot.
    ///
    /// Then pays every asset this heir hasn't been paid yet: their share of the snapshot, rounded
    /// down (`ClaimManager.claim` + `HeritVault.payOut`). An asset with a 0% share is skipped; one
    /// whose share rounds to nothing is marked paid. Refused with "nothing to claim" if no transfer
    /// results. Each transfer has its own callback, which credits the amount back and unmarks the
    /// asset if the transfer failed, so the heir can claim again.
    ///
    /// `#[payable]`: the heir pays for the records this writes (their commitment the first time,
    /// one paid mark per asset) and, per token, their registration with the token (0.00125 NEAR)
    /// and the 1 yoctoNEAR `ft_transfer` needs, out of their account's seed, never out of the
    /// estate. The surplus is refunded. Returns how many
    /// transfers were sent.
    #[payable]
    pub fn claim(
        &mut self,
        label: String,
        attestation: Attestation,
        signature: Base64VecU8,
    ) -> u32 {
        require!(
            self.poke_expiry() == Status::Unlocked,
            "estate not unlocked"
        );
        let heir = self.heirs.get(&label).expect("no such heir");
        require!(
            env::predecessor_account_id() == heir.account_id,
            "not entitled: claim from the heir's own account"
        );
        require!(
            attestation.heir_label.as_deref() == Some(label.as_str()),
            "attestation is for another label"
        );
        let face = self.verify(&attestation, &signature, ACTION_CLAIM);
        require!(
            Some(face) != self.grantor_commitment,
            "wrong human: the grantor cannot claim"
        );

        let before = env::storage_usage();
        match self.claim_commitments.get(&face) {
            Some(claimed) => require!(
                claimed == &label,
                "commitment used: one human, one heir slot"
            ),
            None => {
                self.claim_commitments.insert(face, label.clone());
                self.claim_commitments.flush(); // so storage_usage() sees the write
            }
        }
        log_event(
            "claim_attested",
            &format!(r#"{{"label":"{label}","nonce":"{}"}}"#, attestation.nonce.0),
        );

        let heir_account = env::predecessor_account_id();
        let assets: Vec<Asset> = self.tokens.iter().cloned().collect();
        let (mut sent, mut token_transfers) = (0u32, 0u128);
        for asset in assets {
            let key = (label.clone(), asset.clone());
            if self.paid.contains(&key) {
                continue;
            }
            let bps = self.share(&label, &asset);
            if bps == 0 {
                continue; // no share of this asset, so nothing to record
            }
            self.paid.insert(key);
            let amount = self.payout_amount(&asset, bps);
            if amount == 0 {
                continue; // the share rounded down to nothing
            }
            let available = self.balances.get(&asset).copied().unwrap_or(0);
            require!(amount <= available, "insufficient balance"); // can't happen: shares <= 100%
            self.balances.insert(asset.clone(), available - amount);
            let transfer = match &asset {
                None => {
                    Promise::new(heir_account.clone()).transfer(NearToken::from_yoctonear(amount))
                }
                Some(token) => {
                    token_transfers += 1;
                    // A new heir account isn't registered with the token yet, so register it first.
                    // `then` runs the transfer whether or not that succeeds, so an heir who was
                    // already registered is still paid.
                    ext_ft::ext(token.clone())
                        .with_attached_deposit(TOKEN_REGISTRATION)
                        .with_static_gas(TOKEN_CALL_GAS)
                        .storage_deposit(Some(heir_account.clone()), Some(true))
                        .then(
                            ext_ft::ext(token.clone())
                                .with_attached_deposit(NearToken::from_yoctonear(1))
                                .with_static_gas(TOKEN_CALL_GAS)
                                .ft_transfer(
                                    heir_account.clone(),
                                    U128(amount),
                                    Some("herit claim".into()),
                                ),
                        )
                }
            };
            transfer
                .then(
                    Self::ext(env::current_account_id())
                        .with_static_gas(PAYOUT_CALLBACK_GAS)
                        .on_paid(label.clone(), asset, U128(amount)),
                )
                .detach();
            sent += 1;
        }
        require!(sent > 0, "nothing to claim");
        // `paid` is a LookupSet, which writes at once, so storage_usage() already counts its marks.
        // Per token payout: the heir's registration with the token, and 1 yocto for `ft_transfer`.
        let per_token = TOKEN_REGISTRATION.as_yoctonear() + 1;
        charge_storage(before, token_transfers * per_token);
        log_event(
            "claim_settled",
            &format!(r#"{{"label":"{label}","transfers":{sent}}}"#),
        );
        sent
    }

    /// Runs after each payout. Logs `claimed`, or credits the amount back, unmarks the asset so
    /// the heir can claim it again, and logs `claim_failed`.
    #[private]
    pub fn on_paid(&mut self, label: String, asset: Asset, amount: U128) -> bool {
        let asset_name = asset.clone().map_or("near".to_string(), |t| t.to_string());
        let data = format!(
            r#"{{"label":"{label}","asset":"{asset_name}","amount":"{}"}}"#,
            amount.0
        );
        if is_promise_success() {
            log_event("claimed", &data);
            true
        } else {
            self.credit(&asset, amount.0);
            self.paid.remove(&(label, asset));
            log_event("claim_failed", &data);
            false
        }
    }

    // ---------- heirs ----------

    /// Creates `<label>.<this estate>`, such as `son.alice.herit-near.testnet`, as a real account
    /// holding only `public_key`. The heir generates that key themselves, so the grantor never holds
    /// the private half. Only this estate can create the account: NEAR lets an account create only
    /// its own sub-accounts.
    ///
    /// Attach NEAR for the heir record's storage plus `HEIR_ACCOUNT_SEED`; the surplus is refunded.
    /// If the account can't be created (the name already exists), `on_heir_created` undoes the
    /// record and refunds the grantor.
    #[payable]
    pub fn add_heir(
        &mut self,
        label: String,
        public_key: PublicKey,
        relationship: String,
        default_bps: u16,
    ) -> Promise {
        self.only_grantor();
        self.not_unlocked();
        let account_id = heir_account(&label);
        validate_relationship(&relationship);
        require!(!self.heirs.contains_key(&label), "label taken");
        require!(self.heirs.len() < MAX_HEIRS, "too many heirs");
        require!(
            self.total_default_bps() + default_bps as u32 <= BPS,
            "shares exceed 100%"
        );
        // The defaults' total doesn't see overrides: an asset with overrides has its own total,
        // and this heir's default lands on top of it (as in `recordHeir`).
        for asset in self.override_assets.iter() {
            require!(
                self.total_bps(asset) + default_bps as u32 <= BPS,
                "shares exceed 100%"
            );
        }

        let before = env::storage_usage();
        self.heirs.insert(
            label.clone(),
            Heir {
                account_id: account_id.clone(),
                relationship,
                default_bps,
                created: false,
            },
        );
        self.heirs.flush(); // so storage_usage() sees the write
        let seed = HEIR_ACCOUNT_SEED.as_yoctonear();
        let storage_cost = charge_storage(before, seed);

        Promise::new(account_id)
            .create_account()
            .transfer(HEIR_ACCOUNT_SEED)
            .add_full_access_key(public_key)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(HEIR_CALLBACK_GAS)
                    .on_heir_created(label, U128(storage_cost + seed)),
            )
    }

    /// Runs after the account creation. On success the heir is confirmed. On failure the seed has
    /// come back to this estate, so the record is removed and `refund` goes to the grantor.
    #[private]
    pub fn on_heir_created(&mut self, label: String, refund: U128) -> bool {
        if is_promise_success() {
            let heir = self.heirs.get_mut(&label).expect("no such heir");
            heir.created = true;
            let account = heir.account_id.clone();
            nft::emit_mint(&account, &label); // the credential exists from now on
            log_event(
                "heir_added",
                &format!(r#"{{"label":"{label}","account":"{account}"}}"#),
            );
            true
        } else {
            self.heirs.remove(&label);
            self.clear_shares(&label);
            Promise::new(self.grantor.clone())
                .transfer(NearToken::from_yoctonear(refund.0))
                .detach();
            log_event("heir_failed", &format!(r#"{{"label":"{label}"}}"#));
            false
        }
    }

    /// Drops an heir before unlock. Their account survives, because this estate holds no key to
    /// it, so the same label can't be added again: creating the account would fail and roll back.
    pub fn remove_heir(&mut self, label: String) {
        self.only_grantor();
        self.not_unlocked();
        let heir = self.heirs.get(&label).expect("no such heir");
        require!(heir.created, "heir account still being created");
        let account = heir.account_id.clone();
        self.heirs.remove(&label);
        self.clear_shares(&label);
        nft::emit_burn(&account, &label);
        log_event("heir_removed", &format!(r#"{{"label":"{label}"}}"#));
    }

    /// Sets `label`'s share of `asset` (`null` for NEAR) to `bps`, overriding their default for that
    /// asset only (`setShare`). The asset's total after the change must stay at or under 100%.
    ///
    /// `#[payable]`: a new override is new storage, paid by the grantor, never out of the heirs'
    /// NEAR. Changing an existing override costs nothing. The surplus is refunded.
    #[payable]
    pub fn set_share(&mut self, label: String, asset: Asset, bps: u16) {
        self.only_grantor();
        self.not_unlocked();
        require!(self.heirs.contains_key(&label), "no such heir");
        // The prospective total: swap this heir's current share of the asset for `bps`.
        let prospective = self.total_bps(&asset) - self.share(&label, &asset) as u32 + bps as u32;
        require!(prospective <= BPS, "shares exceed 100%");

        let before = env::storage_usage();
        if !self.override_assets.iter().any(|a| a == &asset) {
            require!(
                self.override_assets.len() < MAX_OVERRIDE_TOKENS,
                "too many assets with overrides"
            );
            self.override_assets.push(asset.clone());
        }
        self.shares.insert((label.clone(), asset.clone()), bps);
        self.override_assets.flush();
        self.shares.flush(); // so storage_usage() sees both writes
        charge_storage(before, 0);

        let asset_name = asset.map_or("near".to_string(), |t| t.to_string());
        log_event(
            "share_set",
            &format!(r#"{{"label":"{label}","asset":"{asset_name}","bps":{bps}}}"#),
        );
    }

    // ---------- views ----------

    /// `label`'s effective share of `asset` in bps: the override if set, else the default.
    pub fn share_of(&self, label: String, asset: Asset) -> u16 {
        self.share(&label, &asset)
    }

    /// The sum of every heir's effective share of `asset`, in bps. At most 10,000.
    pub fn total_share_of(&self, asset: Asset) -> u16 {
        self.total_bps(&asset) as u16
    }

    pub fn heirs_of(&self) -> Vec<HeirView> {
        self.heirs.iter().map(|(l, h)| heir_view(l, h)).collect()
    }

    /// What the estate holds of `asset` (`null` for NEAR, in yoctoNEAR). Not the account balance:
    /// that also pays for storage.
    pub fn balance_of(&self, asset: Asset) -> U128 {
        U128(self.balances.get(&asset).copied().unwrap_or(0))
    }

    /// The label a face commitment (base64) has claimed for, if any (`claimCommitmentUsed`).
    pub fn claim_commitment_used(&self, commitment: Base64VecU8) -> Option<String> {
        let c: [u8; 32] = commitment.0.as_slice().try_into().ok()?;
        self.claim_commitments.get(&c).cloned()
    }

    /// What `label` would receive of `asset` if they claimed now (`claimableOf`): their share of
    /// the snapshot, rounded down, or 0 if already paid. Once the clock says Unlocked it already
    /// shows the figure the snapshot will give, poke or no poke.
    pub fn claimable_of(&self, label: String, asset: Asset) -> U128 {
        U128(self.claimable(&label, &asset))
    }

    /// Every asset with what `label` would receive of it (`claimableAll`).
    pub fn claimable_all(&self, label: String) -> Vec<(Asset, U128)> {
        self.tokens
            .iter()
            .map(|a| (a.clone(), U128(self.claimable(&label, a))))
            .collect()
    }

    /// Whether `label` has been paid `asset` (`hasClaimed`). False again if the transfer failed.
    pub fn has_claimed(&self, label: String, asset: Asset) -> bool {
        self.paid.contains(&(label, asset))
    }

    /// What the estate held of `asset` at unlock. 0 before then.
    pub fn snapshot_of(&self, asset: Asset) -> U128 {
        U128(self.snapshots.get(&asset).copied().unwrap_or(0))
    }

    /// Whether the balances have been frozen.
    pub fn snapshot_taken(&self) -> bool {
        self.snapshot_taken
    }

    /// Every asset the estate has held, in the order first deposited. `null` is NEAR.
    pub fn tokens_of(&self) -> Vec<Asset> {
        self.tokens.iter().cloned().collect()
    }

    pub fn heir_of(&self, label: String) -> Option<HeirView> {
        self.heirs.get(&label).map(|h| heir_view(&label, h))
    }

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
            .unwrap_or_else(|_| env::panic_str("commitment != 32 bytes"));
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
        .unwrap_or_else(|_| env::abort());
        let sig: [u8; 64] = sig
            .0
            .as_slice()
            .try_into()
            .unwrap_or_else(|_| env::panic_str("signature != 64 bytes"));
        require!(
            env::ed25519_verify(&sig, &msg, &self.attestor_pk),
            "invalid signature"
        );
        self.used_nonces.insert(a.nonce.0);
        commitment
    }

    /// Lists `asset` the first time it's seen (`_listToken`) and returns the storage that took, in
    /// yoctoNEAR. The cap is on the list, not the money: topping up a listed asset always works.
    fn list_asset(&mut self, asset: &Asset) -> u128 {
        if self.is_listed(asset) {
            return 0;
        }
        require!(self.tokens.len() < MAX_TOKENS, "too many assets");
        let before = env::storage_usage();
        self.tokens.push(asset.clone());
        self.balances.insert(asset.clone(), 0);
        self.snapshots.insert(asset.clone(), 0);
        self.tokens.flush();
        self.balances.flush();
        self.snapshots.flush(); // so storage_usage() sees all three writes
        (env::storage_usage() - before) as u128 * env::storage_byte_cost().as_yoctonear()
    }

    /// `_claimable`.
    fn claimable(&self, label: &str, asset: &Asset) -> u128 {
        if !self.heirs.contains_key(label)
            || self.paid.contains(&(label.to_string(), asset.clone()))
        {
            return 0;
        }
        self.payout_amount(asset, self.share(label, asset))
    }

    /// `snapshot × bps / 10,000`, rounded down: dust beats paying out more than was there. Before
    /// the snapshot is taken it reads the balance, which is what the snapshot will be once the
    /// clock says Unlocked, and 0 before that.
    fn payout_amount(&self, asset: &Asset, bps: u16) -> u128 {
        let ruler = if self.snapshot_taken {
            self.snapshots.get(asset).copied().unwrap_or(0)
        } else if self.pending_status() == Status::Unlocked {
            self.balances.get(asset).copied().unwrap_or(0)
        } else {
            0
        };
        ruler * bps as u128 / BPS as u128
    }

    /// Freezes every listed balance (`snapshot` in `HeritVault`). A no-op the second time, as
    /// there: re-running it after a payout would shrink the ruler.
    fn snapshot(&mut self) {
        if self.snapshot_taken {
            return;
        }
        self.snapshot_taken = true;
        for asset in self.tokens.iter() {
            let balance = self.balances.get(asset).copied().unwrap_or(0);
            self.snapshots.insert(asset.clone(), balance);
        }
        log_event(
            "snapshotted",
            &format!(r#"{{"assets":{}}}"#, self.tokens.len()),
        );
    }

    fn is_listed(&self, asset: &Asset) -> bool {
        self.tokens.iter().any(|a| a == asset)
    }

    /// Undoes `list_asset` for a token whose registration failed. Keeps the others in order.
    fn unlist_asset(&mut self, asset: &Asset) {
        let kept: Vec<Asset> = self
            .tokens
            .iter()
            .filter(|a| *a != asset)
            .cloned()
            .collect();
        self.tokens.clear();
        self.tokens.extend(kept);
        self.balances.remove(asset);
        self.snapshots.remove(asset);
    }

    fn credit(&mut self, asset: &Asset, amount: u128) {
        *self.balances.get_mut(asset).expect("asset not listed") += amount;
    }

    /// `_effectiveShare`: the override if one is set, else the heir's default. 0 for no such heir.
    fn share(&self, label: &str, asset: &Asset) -> u16 {
        match self.shares.get(&(label.to_string(), asset.clone())) {
            Some(bps) => *bps,
            None => self.heirs.get(label).map_or(0, |h| h.default_bps),
        }
    }

    /// `_totalBps`: every heir's effective share of `asset`, pending heirs included.
    fn total_bps(&self, asset: &Asset) -> u32 {
        self.heirs.keys().map(|l| self.share(l, asset) as u32).sum()
    }

    /// Drops every override `label` had, when the heir goes away.
    fn clear_shares(&mut self, label: &str) {
        for asset in self.override_assets.iter() {
            self.shares.remove(&(label.to_string(), asset.clone()));
        }
    }

    /// Counts heirs whose accounts are still being created too, so a pending heir's share is
    /// never handed out twice.
    fn total_default_bps(&self) -> u32 {
        self.heirs.values().map(|h| h.default_bps as u32).sum()
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

/// NEP-141 deposits (`depositERC20`): the grantor calls `ft_transfer_call` on the token with this
/// estate as the receiver. Refusing panics, and the token returns the whole amount to the sender.
#[near]
impl FungibleTokenReceiver for Estate {
    fn ft_on_transfer(
        &mut self,
        sender_id: AccountId,
        amount: U128,
        msg: String,
    ) -> PromiseOrValue<U128> {
        let _ = msg;
        let token = env::predecessor_account_id();
        let asset = Some(token.clone());
        require!(self.is_listed(&asset), "token not added");
        require!(sender_id == self.grantor, "only the grantor deposits");
        self.not_unlocked();
        require!(amount.0 > 0, "zero amount");
        // NEP-141 transfers exactly `amount`, so what arrives is what's credited.
        self.credit(&asset, amount.0);
        log_event(
            "deposited",
            &format!(r#"{{"asset":"{token}","amount":"{}"}}"#, amount.0),
        );
        PromiseOrValue::Value(U128(0)) // keep all of it
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

/// `<label>.<this estate>`. A label is one lowercase segment of a NEAR account id: `son`,
/// `daughter-2`. The account id parser enforces the rest, such as the 64-character limit.
fn heir_account(label: &str) -> AccountId {
    require!(
        !label.is_empty()
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_'),
        "label must be a single lowercase segment"
    );
    format!("{label}.{}", env::current_account_id())
        .parse()
        .unwrap_or_else(|_| env::panic_str("label is not a valid account name"))
}

/// Letters, digits, spaces and hyphens, so it can go into JSON logs and NFT metadata as is.
fn validate_relationship(r: &str) {
    require!(
        !r.trim().is_empty()
            && r.len() <= MAX_RELATIONSHIP_LEN
            && r.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b' ' || b == b'-'),
        "relationship must be 1-32 letters, digits, spaces or hyphens"
    );
}

fn heir_view(label: &str, h: &Heir) -> HeirView {
    HeirView {
        label: label.to_string(),
        account_id: h.account_id.clone(),
        relationship: h.relationship.clone(),
        default_bps: h.default_bps,
        created: h.created,
    }
}

/// Charges the caller for the storage written since `before`, plus `reserved` (NEAR this call
/// sends on), and refunds the rest of the attached deposit. Returns the storage cost.
fn charge_storage(before: u64, reserved: u128) -> u128 {
    let used = env::storage_usage().saturating_sub(before) as u128;
    let storage_cost = env::storage_byte_cost().as_yoctonear() * used;
    let cost = storage_cost + reserved;
    let paid = env::attached_deposit().as_yoctonear();
    require!(
        paid >= cost,
        "attach more NEAR for storage and the heir's seed"
    );
    if paid > cost {
        Promise::new(env::predecessor_account_id())
            .transfer(NearToken::from_yoctonear(paid - cost))
            .detach();
    }
    storage_cost
}

fn decode_ed25519(s: &str) -> [u8; 32] {
    let b58 = s
        .strip_prefix("ed25519:")
        .expect("expected ed25519:<base58>");
    near_sdk::bs58::decode(b58)
        .into_vec()
        .unwrap_or_else(|_| env::panic_str("bad base58"))
        .try_into()
        .unwrap_or_else(|_| env::panic_str("ed25519 key must be 32 bytes"))
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

#[cfg(test)]
mod heir_tests;

#[cfg(test)]
mod heir_invariant;

#[cfg(test)]
mod vault_tests;

#[cfg(test)]
mod claim_tests;

#[cfg(test)]
mod nft_tests;

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
