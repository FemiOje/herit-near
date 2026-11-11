//! Unit tests: the batch `open_estate` builds, the callback, and the views.

use near_sdk::json_types::U64;
use near_sdk::mock::MockAction;
use near_sdk::test_utils::{VMContextBuilder, get_created_receipts, get_logs};
use near_sdk::{AccountId, NearToken, PromiseResult, testing_env};

use super::*;

pub const HASH: [u8; 32] = [9; 32];
pub const ATTESTOR: &str = "ed25519:FzTUQSiLeJji46M3GXzoiBntYV5wbnomrF4ywomBJwfe";

pub fn factory_id() -> AccountId {
    "herit-near.testnet".parse().unwrap()
}
pub fn grantor() -> AccountId {
    "alice-kaykay.testnet".parse().unwrap()
}
pub fn other_grantor() -> AccountId {
    "bob-kaykay.testnet".parse().unwrap()
}
pub fn estate(label: &str) -> AccountId {
    format!("{label}.herit-near.testnet").parse().unwrap()
}

pub fn set_context(caller: AccountId, deposit: NearToken) {
    testing_env!(
        VMContextBuilder::new()
            .current_account_id(factory_id())
            .predecessor_account_id(caller)
            .attached_deposit(deposit)
            .block_timestamp(1_700_000_000_000_000_000)
            .build()
    );
}

pub fn set_callback(created: bool) {
    let result = if created {
        PromiseResult::Successful(vec![])
    } else {
        PromiseResult::Failed
    };
    testing_env!(
        VMContextBuilder::new()
            .current_account_id(factory_id())
            .predecessor_account_id(factory_id())
            .block_timestamp(1_700_000_000_000_000_000)
            .build(),
        near_sdk::test_vm_config(),
        near_sdk::RuntimeFeesConfig::test(),
        Default::default(),
        vec![result],
    );
}

/// A fresh factory, on empty storage.
pub fn new_factory() -> Factory {
    set_context(factory_id(), NearToken::from_near(0));
    near_sdk::mock::with_mocked_blockchain(|b| drop(b.take_storage()));
    Factory::new(near_sdk::bs58::encode(HASH).into_string(), ATTESTOR.into())
}

/// `grantor` opens `label`, and the batch succeeds.
pub fn open(f: &mut Factory, grantor: AccountId, label: &str) {
    let deposit = NearToken::from_near(1);
    set_context(grantor.clone(), deposit);
    let _ = f.open_estate(label.into(), 600, 300);
    set_callback(true);
    assert!(f.on_estate_created(estate(label), grantor, deposit));
}

#[test]
fn new_stores_the_code_hash_and_the_attestor() {
    let f = new_factory();
    assert_eq!(
        f.config(),
        Config {
            estate_code_hash: near_sdk::bs58::encode(HASH).into_string(),
            attestor_pk: ATTESTOR.into()
        }
    );
    assert_eq!(f.estates_count(), 0);
}

#[test]
#[should_panic(expected = "estate_code_hash must be 32 bytes")]
fn new_refuses_a_short_hash() {
    set_context(factory_id(), NearToken::from_near(0));
    Factory::new("abc".into(), ATTESTOR.into());
}

#[test]
#[should_panic(expected = "attestor_pk: expected ed25519:<base58>")]
fn new_refuses_a_malformed_attestor() {
    set_context(factory_id(), NearToken::from_near(0));
    Factory::new(near_sdk::bs58::encode(HASH).into_string(), "FzTU".into());
}

#[test]
fn open_estate_creates_a_keyless_account_running_the_published_code() {
    let mut f = new_factory();
    set_context(grantor(), NearToken::from_near(1));
    let _ = f.open_estate("alice".into(), 600, 300);

    let receipts = get_created_receipts();
    let batch = receipts
        .iter()
        .find(|r| r.receiver_id == estate("alice"))
        .expect("a batch to the estate");
    assert!(matches!(batch.actions[0], MockAction::CreateAccount { .. }));
    match &batch.actions[1] {
        MockAction::Transfer { deposit, .. } => {
            assert_eq!(
                deposit.as_yoctonear(),
                NearToken::from_near(1).as_yoctonear() - FACTORY_FEE.as_yoctonear()
            )
        }
        other => panic!("expected the funding, got {other:?}"),
    }
    // The exact hash is checked in `the_published_code_is_the_one_from_init`.
    assert!(
        matches!(&batch.actions[2], MockAction::UseGlobalContract { .. }),
        "{:?}",
        batch.actions[2]
    );
    match &batch.actions[3] {
        MockAction::FunctionCallWeight {
            method_name, args, ..
        } => {
            assert_eq!(method_name, b"new");
            let args: near_sdk::serde_json::Value = near_sdk::serde_json::from_slice(args).unwrap();
            assert_eq!(args["grantor"], near_sdk::serde_json::json!(grantor()));
            assert_eq!(args["attestor_pk"], ATTESTOR);
            assert_eq!(args["check_in_interval_sec"], 600);
            assert_eq!(args["grace_sec"], 300);
        }
        other => panic!("expected new, got {other:?}"),
    }
    let keys = batch.actions.iter().any(|a| {
        matches!(
            a,
            MockAction::AddKeyWithFullAccess { .. } | MockAction::AddKeyWithFunctionCall { .. }
        )
    });
    assert!(!keys, "the estate must be created keyless");
    let callback = receipts.iter().flat_map(|r| &r.actions).any(|a| {
        matches!(a, MockAction::FunctionCallWeight { method_name, .. } if method_name == b"on_estate_created")
    });
    assert!(callback);
}

#[test]
fn the_published_code_is_the_one_from_init() {
    let mut f = new_factory();
    set_context(grantor(), NearToken::from_near(1));
    let _ = f.open_estate("alice".into(), 600, 300);
    let used = get_created_receipts()
        .into_iter()
        .flat_map(|r| r.actions)
        .find_map(|a| match a {
            MockAction::UseGlobalContract { contract_id, .. } => Some(format!("{contract_id:?}")),
            _ => None,
        })
        .expect("use_global_contract");
    let expected = near_sdk::bs58::encode(HASH).into_string();
    assert!(used.contains(&expected), "{used} should name {expected}");
}

#[test]
#[should_panic(expected = "attach at least 0.11 NEAR")]
fn open_estate_needs_the_minimum_deposit() {
    let mut f = new_factory();
    set_context(grantor(), NearToken::from_millinear(109));
    let _ = f.open_estate("alice".into(), 600, 300);
}

#[test]
fn bad_labels_are_refused() {
    for label in ["", "Alice", "a.b", "al ice", "-alice", &"a".repeat(64)] {
        let mut f = new_factory();
        set_context(grantor(), NearToken::from_near(1));
        let refused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = f.open_estate(label.into(), 600, 300);
        }))
        .is_err();
        assert!(refused, "{label:?}");
    }
}

#[test]
fn a_created_estate_is_recorded() {
    let mut f = new_factory();
    open(&mut f, grantor(), "alice");
    assert_eq!(f.estates_of(grantor()), vec![estate("alice")]);
    assert_eq!(f.estates_count(), 1);
    assert_eq!(
        f.estates(None, None),
        vec![EstateEntry {
            account_id: estate("alice"),
            grantor: grantor(),
            opened_at_ns: U64(1_700_000_000_000_000_000)
        }]
    );
    assert!(
        get_logs()
            .iter()
            .any(|l| l.contains(r#""event":"estate_opened""#))
    );
}

#[test]
fn a_failed_estate_refunds_the_whole_deposit_and_records_nothing() {
    let mut f = new_factory();
    let deposit = NearToken::from_near(1);
    set_context(grantor(), deposit);
    let _ = f.open_estate("alice".into(), 600, 300);
    set_callback(false);
    assert!(!f.on_estate_created(estate("alice"), grantor(), deposit));

    let refunds: Vec<u128> = get_created_receipts()
        .into_iter()
        .filter(|r| r.receiver_id == grantor())
        .flat_map(|r| r.actions)
        .filter_map(|a| match a {
            MockAction::Transfer { deposit, .. } => Some(deposit.as_yoctonear()),
            _ => None,
        })
        .collect();
    assert_eq!(
        refunds,
        vec![deposit.as_yoctonear()],
        "funding and fee both come back"
    );
    assert_eq!(f.estates_count(), 0);
    assert!(f.estates_of(grantor()).is_empty());
}

#[test]
fn estates_are_listed_per_grantor_and_in_pages() {
    let mut f = new_factory();
    open(&mut f, grantor(), "alice");
    open(&mut f, other_grantor(), "bob");
    open(&mut f, grantor(), "alice-2");
    assert_eq!(
        f.estates_of(grantor()),
        vec![estate("alice"), estate("alice-2")]
    );
    assert_eq!(f.estates_of(other_grantor()), vec![estate("bob")]);
    assert!(f.estates_of(estate("nobody")).is_empty());
    let page: Vec<AccountId> = f
        .estates(Some(1), Some(1))
        .into_iter()
        .map(|e| e.account_id)
        .collect();
    assert_eq!(page, vec![estate("bob")]);
    assert_eq!(f.estates(Some(3), None), vec![]);
    assert_eq!(
        f.estates(None, Some(1_000)).len(),
        3,
        "capped at 100, here all 3"
    );
}

#[test]
fn set_attestor_changes_the_key_for_new_estates() {
    let mut f = new_factory();
    set_context(factory_id(), NearToken::from_near(0));
    let other = "ed25519:6E8sCci9badyRkXb3JoRpBj5p8C6Tw41ELDZoiihKEtp";
    f.set_attestor(other.into());
    assert_eq!(f.config().attestor_pk, other);
    set_context(grantor(), NearToken::from_near(1));
    let _ = f.open_estate("alice".into(), 600, 300);
    let args = get_created_receipts()
        .into_iter()
        .flat_map(|r| r.actions)
        .find_map(|a| match a {
            MockAction::FunctionCallWeight {
                method_name, args, ..
            } if method_name == b"new" => Some(args),
            _ => None,
        })
        .unwrap();
    assert!(String::from_utf8(args).unwrap().contains(other));
}

#[test]
#[should_panic(expected = "attestor_pk must be 32 bytes")]
fn set_attestor_refuses_a_bad_key() {
    let mut f = new_factory();
    set_context(factory_id(), NearToken::from_near(0));
    f.set_attestor("ed25519:abc".into());
}

#[test]
fn set_estate_code_changes_the_code_for_new_estates() {
    let mut f = new_factory();
    set_context(factory_id(), NearToken::from_near(0));
    let other = near_sdk::bs58::encode([9u8; 32]).into_string();
    f.set_estate_code(other.clone());
    assert_eq!(f.config().estate_code_hash, other);
    assert!(
        near_sdk::test_utils::get_logs()
            .iter()
            .any(|l| l.contains(r#""event":"estate_code_set""#) && l.contains(&other))
    );
    set_context(grantor(), NearToken::from_near(1));
    let _ = f.open_estate("alice".into(), 600, 300);
    let used = get_created_receipts()
        .into_iter()
        .flat_map(|r| r.actions)
        .find_map(|a| match a {
            MockAction::UseGlobalContract { contract_id, .. } => Some(format!("{contract_id:?}")),
            _ => None,
        })
        .expect("use_global_contract");
    assert!(used.contains(&other), "{used} should name {other}");
}

#[test]
#[should_panic(expected = "estate_code_hash must be 32 bytes")]
fn set_estate_code_refuses_a_bad_hash() {
    let mut f = new_factory();
    set_context(factory_id(), NearToken::from_near(0));
    f.set_estate_code(near_sdk::bs58::encode([9u8; 31]).into_string());
}
