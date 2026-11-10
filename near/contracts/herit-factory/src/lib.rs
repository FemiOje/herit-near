//! Herit factory, at `herit-near.testnet`: opens estates.
//!
//! `open_estate("alice", ..)` creates `alice.herit-near.testnet` with no access keys, attaches the
//! estate code published once as an immutable Global Contract, and initialises it with the caller
//! as grantor, all in one batch. Nobody, the grantor and this factory included, can later
//! redeploy, delete or drain an estate outside its own rules. The factory also keeps the public
//! list of estates, for the explorer and each grantor's dashboard.

use near_sdk::json_types::U64;
use near_sdk::store::{LookupMap, Vector};
use near_sdk::{
    AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise, env, is_promise_success,
    near, require,
};

/// Covers a new estate's own state and leaves it a buffer; the rest of the deposit goes to the
/// estate too.
pub const MIN_ESTATE_DEPOSIT: NearToken = NearToken::from_millinear(100);
/// Kept by the factory for its two records of the estate (about 300 bytes, 0.003 NEAR), so the
/// grantor pays for them rather than the factory's balance. Refunded if the estate isn't created.
pub const FACTORY_FEE: NearToken = NearToken::from_millinear(10);
const INIT_GAS: Gas = Gas::from_tgas(30);
const CALLBACK_GAS: Gas = Gas::from_tgas(10);
const MAX_PAGE: u32 = 100;

#[derive(BorshStorageKey)]
#[near]
enum Key {
    Estates,
    AllEstates,
    EstatesOfGrantor(AccountId),
}

#[near(serializers = [borsh, json])]
#[derive(Clone, Debug, PartialEq)]
pub struct EstateEntry {
    pub account_id: AccountId,
    pub grantor: AccountId,
    pub opened_at_ns: U64,
}

#[near(serializers = [json])]
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// base58 SHA-256 of the published estate wasm.
    pub estate_code_hash: String,
    pub attestor_pk: String,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Factory {
    estate_code_hash: [u8; 32],
    /// `ed25519:<base58>`, given to each new estate.
    attestor_pk: String,
    /// grantor -> their estates, oldest first.
    estates: LookupMap<AccountId, Vector<AccountId>>,
    /// Every estate, oldest first: the public explorer's list.
    all_estates: Vector<EstateEntry>,
}

#[near]
impl Factory {
    /// Deployed and initialised in one transaction, so nobody can initialise it first.
    #[init]
    pub fn new(estate_code_hash: String, attestor_pk: String) -> Self {
        Self {
            estate_code_hash: decode_hash(&estate_code_hash),
            attestor_pk: checked_attestor(attestor_pk),
            estates: LookupMap::new(Key::Estates),
            all_estates: Vector::new(Key::AllEstates),
        }
    }

    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Signed by the factory's own key. Only estates opened afterwards get the new attestor:
    /// existing ones are keyless and keep theirs.
    #[private]
    pub fn set_attestor(&mut self, attestor_pk: String) {
        self.attestor_pk = checked_attestor(attestor_pk);
        log_event(
            "attestor_set",
            &format!(r#"{{"attestor_pk":"{}"}}"#, self.attestor_pk),
        );
    }

    /// Opens `<label>.<this factory>` for the caller. Attach at least `MIN_ESTATE_DEPOSIT` plus
    /// `FACTORY_FEE`: the estate gets everything but the fee. If the estate can't be created (the
    /// name is taken, or the timers are out of bounds), the whole deposit comes back.
    #[payable]
    pub fn open_estate(
        &mut self,
        label: String,
        check_in_interval_sec: u32,
        grace_sec: u32,
    ) -> Promise {
        let estate = estate_account(&label);
        let grantor = env::predecessor_account_id();
        let deposit = env::attached_deposit();
        require!(
            deposit.as_yoctonear()
                >= MIN_ESTATE_DEPOSIT.as_yoctonear() + FACTORY_FEE.as_yoctonear(),
            "attach at least 0.11 NEAR: 0.1 for the estate and 0.01 for the factory's records"
        );
        let funding =
            NearToken::from_yoctonear(deposit.as_yoctonear() - FACTORY_FEE.as_yoctonear());
        let args = format!(
            r#"{{"grantor":"{grantor}","attestor_pk":"{}","check_in_interval_sec":{check_in_interval_sec},"grace_sec":{grace_sec}}}"#,
            self.attestor_pk
        );
        // No access key: the estate account is created keyless.
        Promise::new(estate.clone())
            .create_account()
            .transfer(funding)
            .use_global_contract(self.estate_code_hash)
            .function_call("new", args.into_bytes(), NearToken::from_near(0), INIT_GAS)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(CALLBACK_GAS)
                    .on_estate_created(estate, grantor, deposit),
            )
    }

    /// Records the estate, or, if the batch failed, sends the whole deposit back to the grantor:
    /// the funding has come back to this factory, and the fee wasn't earned.
    #[private]
    pub fn on_estate_created(
        &mut self,
        estate: AccountId,
        grantor: AccountId,
        deposit: NearToken,
    ) -> bool {
        if is_promise_success() {
            self.estates
                .entry(grantor.clone())
                .or_insert_with(|| Vector::new(Key::EstatesOfGrantor(grantor.clone())))
                .push(estate.clone());
            self.all_estates.push(EstateEntry {
                account_id: estate.clone(),
                grantor: grantor.clone(),
                opened_at_ns: U64(env::block_timestamp()),
            });
            log_event(
                "estate_opened",
                &format!(r#"{{"estate":"{estate}","grantor":"{grantor}"}}"#),
            );
            true
        } else {
            Promise::new(grantor).transfer(deposit).detach();
            log_event("estate_failed", &format!(r#"{{"estate":"{estate}"}}"#));
            false
        }
    }

    // ---------- views ----------

    /// A grantor's estates, oldest first.
    pub fn estates_of(&self, grantor: AccountId) -> Vec<AccountId> {
        self.estates
            .get(&grantor)
            .map(|v| v.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Every estate, oldest first, at most 100 a page. The frontend reads `estates_count` and pages
    /// back from the end for newest first.
    pub fn estates(&self, from_index: Option<u32>, limit: Option<u32>) -> Vec<EstateEntry> {
        self.all_estates
            .iter()
            .skip(from_index.unwrap_or(0) as usize)
            .take(limit.unwrap_or(MAX_PAGE).min(MAX_PAGE) as usize)
            .cloned()
            .collect()
    }

    pub fn estates_count(&self) -> u32 {
        self.all_estates.len()
    }

    pub fn config(&self) -> Config {
        Config {
            estate_code_hash: near_sdk::bs58::encode(self.estate_code_hash).into_string(),
            attestor_pk: self.attestor_pk.clone(),
        }
    }
}

/// `<label>.<this factory>`. The same label rule as an estate's heirs: one lowercase segment of a
/// NEAR account id.
fn estate_account(label: &str) -> AccountId {
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

fn decode_hash(s: &str) -> [u8; 32] {
    near_sdk::bs58::decode(s)
        .into_vec()
        .unwrap_or_else(|_| env::panic_str("estate_code_hash: bad base58"))
        .try_into()
        .unwrap_or_else(|_| env::panic_str("estate_code_hash must be 32 bytes"))
}

/// The estate decodes it at `new`; checked here too, so a bad key fails at the factory rather than
/// in every estate it opens.
fn checked_attestor(pk: String) -> String {
    let b58 = pk
        .strip_prefix("ed25519:")
        .unwrap_or_else(|| env::panic_str("attestor_pk: expected ed25519:<base58>"));
    let bytes = near_sdk::bs58::decode(b58)
        .into_vec()
        .unwrap_or_else(|_| env::panic_str("attestor_pk: bad base58"));
    require!(bytes.len() == 32, "attestor_pk must be 32 bytes");
    pk
}

/// NEP-297 event, as the estate logs them.
fn log_event(event: &str, data: &str) {
    env::log_str(&format!(
        r#"EVENT_JSON:{{"standard":"herit","version":"1.0.0","event":"{event}","data":[{data}]}}"#
    ));
}

#[cfg(test)]
mod fuzz;

#[cfg(test)]
mod tests;
