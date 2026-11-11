//! Checkpoint 6: heirs as real sub-accounts of the estate.

use near_sdk::NearToken;
use near_sdk::json_types::U128;
use near_sdk::mock::MockAction;
use near_sdk::test_utils::{get_created_receipts, get_logs};

use super::testing::*;
use super::*;

fn transfers_to(receiver: &AccountId) -> Vec<u128> {
    get_created_receipts()
        .into_iter()
        .filter(|r| &r.receiver_id == receiver)
        .flat_map(|r| r.actions)
        .filter_map(|a| match a {
            MockAction::Transfer { deposit, .. } => Some(deposit.as_yoctonear()),
            _ => None,
        })
        .collect()
}

#[test]
fn add_heir_records_a_pending_heir_and_creates_its_account() {
    let mut e = new_estate();
    set_context_paying(grantor(), 0, HEIR_DEPOSIT);
    let _ = e.add_heir("son".into(), heir_key(1), "son".into(), 6_000);

    assert_eq!(
        e.heir_of("son".into()),
        Some(HeirView {
            label: "son".into(),
            account_id: heir_id("son"),
            relationship: "son".into(),
            default_bps: 6_000,
            created: false,
        })
    );

    let receipts = get_created_receipts();
    let create = receipts
        .iter()
        .find(|r| r.receiver_id == heir_id("son"))
        .expect("a receipt to the heir's account");
    assert!(matches!(
        create.actions[0],
        MockAction::CreateAccount { .. }
    ));
    assert!(matches!(
        create.actions[1],
        MockAction::Transfer { deposit, .. } if deposit == HEIR_ACCOUNT_SEED
    ));
    match &create.actions[2] {
        MockAction::AddKeyWithFullAccess { public_key, .. } => {
            assert_eq!(public_key.to_string(), String::from(&heir_key(1)));
        }
        other => panic!("expected the heir's key, got {other:?}"),
    }
    let callback = receipts
        .iter()
        .flat_map(|r| &r.actions)
        .any(|a| matches!(a, MockAction::FunctionCallWeight { method_name, .. } if method_name == b"on_heir_created"));
    assert!(callback, "on_heir_created must be scheduled");
}

#[test]
fn the_surplus_deposit_is_refunded_to_the_grantor() {
    let mut e = new_estate();
    add_heir_at(&mut e, "son", 5_000, 0);
    let refunds = transfers_to(&grantor());
    assert_eq!(refunds.len(), 1);
    let kept = HEIR_DEPOSIT.as_yoctonear() - refunds[0];
    // The seed plus a little storage; never the whole deposit.
    assert!(kept > HEIR_ACCOUNT_SEED.as_yoctonear());
    assert!(kept < HEIR_ACCOUNT_SEED.as_yoctonear() + NearToken::from_millinear(10).as_yoctonear());
}

#[test]
#[should_panic(expected = "attach more NEAR")]
fn add_heir_needs_the_seed_attached() {
    let mut e = new_estate();
    set_context_paying(grantor(), 0, HEIR_ACCOUNT_SEED); // nothing left for storage
    let _ = e.add_heir("son".into(), heir_key(1), "son".into(), 5_000);
}

#[test]
fn a_successful_creation_confirms_the_heir() {
    let mut e = new_estate();
    add_heir_at(&mut e, "son", 5_000, 0);
    set_callback_context(true);
    assert!(e.on_heir_created("son".into(), U128(1)));

    assert!(e.heir_of("son".into()).unwrap().created);
    assert!(
        get_logs()
            .iter()
            .any(|l| l.contains(r#""event":"heir_added""#))
    );
    assert!(
        transfers_to(&grantor()).is_empty(),
        "nothing is refunded on success"
    );
}

#[test]
fn a_failed_creation_removes_the_heir_and_refunds_the_grantor() {
    let mut e = new_estate();
    add_heir_at(&mut e, "son", 5_000, 0);
    set_callback_context(false);
    assert!(!e.on_heir_created("son".into(), U128(123)));

    assert_eq!(e.heir_of("son".into()), None);
    assert_eq!(transfers_to(&grantor()), vec![123]);
    // The share and the slot are free again.
    add_heir_at(&mut e, "daughter", 10_000, 0);
}

// `#[private]` is enforced by the wasm entry point, which unit tests bypass: the sandbox test
// `only_the_estate_can_run_the_callback` covers it.

#[test]
#[should_panic(expected = "not the grantor")]
fn a_non_grantor_cannot_add_an_heir() {
    let mut e = new_estate();
    set_context_paying(stranger(), 0, HEIR_DEPOSIT);
    let _ = e.add_heir("son".into(), heir_key(1), "son".into(), 5_000);
}

#[test]
#[should_panic(expected = "estate unlocked")]
fn no_heir_can_be_added_once_unlocked() {
    let mut e = new_estate();
    check_in_at(&mut e, CHECKED_IN_AT, 1);
    let (_, unlock_at) = e.deadlines_of();
    add_heir_at(&mut e, "son", 5_000, unlock_at.0);
}

#[test]
fn an_heir_can_be_added_in_grace() {
    let mut e = new_estate();
    check_in_at(&mut e, CHECKED_IN_AT, 1);
    let (grace_at, _) = e.deadlines_of();
    add_heir_at(&mut e, "son", 5_000, grace_at.0);
    assert!(e.heir_of("son".into()).is_some());
}

#[test]
fn bad_labels_are_refused() {
    let too_long = "a".repeat(64);
    for label in [
        "",
        "a.b",
        "Son",
        "son!",
        "s n",
        too_long.as_str(),
        "-son",
        "son-",
        "a--b",
    ] {
        let mut e = new_estate();
        let ok = call_reverting(&mut e, |e| add_heir_at(e, label, 5_000, 0));
        assert!(!ok, "{label:?} should be refused");
        assert!(e.heirs_of().is_empty());
    }
}

#[test]
fn good_labels_are_accepted() {
    for label in ["son", "daughter-2", "a_b", "x", "0"] {
        let mut e = new_estate();
        add_heir_at(&mut e, label, 5_000, 0);
        assert_eq!(e.heir_of(label.into()).unwrap().account_id, heir_id(label));
    }
}

#[test]
fn bad_relationships_are_refused() {
    let too_long = "a".repeat(MAX_RELATIONSHIP_LEN + 1);
    for relationship in ["", "   ", r#"son","x":"y"#, "son\n", too_long.as_str()] {
        let mut e = new_estate();
        let ok = call_reverting(&mut e, |e| {
            set_context_paying(grantor(), 0, HEIR_DEPOSIT);
            let _ = e.add_heir("son".into(), heir_key(1), relationship.into(), 5_000);
        });
        assert!(!ok, "{relationship:?} should be refused");
    }
}

#[test]
#[should_panic(expected = "label taken")]
fn a_label_cannot_be_added_twice() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 1_000);
    add_heir_at(&mut e, "son", 1_000, 0);
}

#[test]
#[should_panic(expected = "label taken")]
fn a_pending_label_cannot_be_added_again() {
    let mut e = new_estate();
    add_heir_at(&mut e, "son", 1_000, 0);
    add_heir_at(&mut e, "son", 1_000, 0);
}

#[test]
#[should_panic(expected = "too many heirs")]
fn at_most_ten_heirs() {
    let mut e = new_estate();
    for i in 0..MAX_HEIRS {
        add_created_heir(&mut e, &format!("heir{i}"), 1_000);
    }
    add_heir_at(&mut e, "one-more", 0, 0);
}

#[test]
#[should_panic(expected = "shares exceed 100%")]
fn shares_cannot_exceed_100_percent() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 6_000);
    add_heir_at(&mut e, "daughter", 4_001, 0);
}

#[test]
#[should_panic(expected = "shares exceed 100%")]
fn a_pending_heirs_share_counts() {
    let mut e = new_estate();
    add_heir_at(&mut e, "son", 6_000, 0); // callback not run yet
    add_heir_at(&mut e, "daughter", 4_001, 0);
}

#[test]
fn shares_of_exactly_100_percent_are_accepted() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 6_000);
    add_created_heir(&mut e, "daughter", 4_000);
    assert_eq!(e.total_default_bps(), BPS);
}

#[test]
fn remove_heir_frees_the_label_slot_and_share() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 6_000);
    set_context(grantor(), 0);
    e.remove_heir("son".into());

    assert_eq!(e.heir_of("son".into()), None);
    assert!(
        get_logs()
            .iter()
            .any(|l| l.contains(r#""event":"heir_removed""#))
    );
    add_heir_at(&mut e, "daughter", 10_000, 0);
}

#[test]
#[should_panic(expected = "heir account still being created")]
fn a_pending_heir_cannot_be_removed() {
    let mut e = new_estate();
    add_heir_at(&mut e, "son", 5_000, 0);
    set_context(grantor(), 0);
    e.remove_heir("son".into());
}

#[test]
#[should_panic(expected = "no such heir")]
fn removing_an_unknown_heir_is_refused() {
    let mut e = new_estate();
    set_context(grantor(), 0);
    e.remove_heir("son".into());
}

#[test]
#[should_panic(expected = "not the grantor")]
fn a_non_grantor_cannot_remove_an_heir() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 5_000);
    set_context(stranger(), 0);
    e.remove_heir("son".into());
}

#[test]
#[should_panic(expected = "estate unlocked")]
fn no_heir_can_be_removed_once_unlocked() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 5_000);
    check_in_at(&mut e, CHECKED_IN_AT, 1);
    let (_, unlock_at) = e.deadlines_of();
    set_context(grantor(), unlock_at.0);
    e.remove_heir("son".into());
}

#[test]
fn heirs_of_lists_every_heir() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 6_000);
    add_heir_at(&mut e, "daughter", 4_000, 0);
    let mut labels: Vec<(String, bool)> = e
        .heirs_of()
        .into_iter()
        .map(|h| (h.label, h.created))
        .collect();
    labels.sort();
    assert_eq!(
        labels,
        vec![("daughter".into(), false), ("son".into(), true)]
    );
}

// ---------- named heirs: an account the heir already has ----------

#[test]
fn a_named_heir_is_confirmed_at_once_with_its_credential() {
    let mut e = new_estate();
    add_named_heir_at(&mut e, "kay", named_id(2), 5_000, 0);

    assert_eq!(
        e.heir_of("kay".into()),
        Some(HeirView {
            label: "kay".into(),
            account_id: named_id(2),
            relationship: "son".into(),
            default_bps: 5_000,
            created: true,
        })
    );
    assert_eq!(e.nft_token("kay".into()).unwrap().owner_id, named_id(2));
    let logs = get_logs();
    assert!(
        logs.iter()
            .any(|l| l.contains(r#""event":"heir_added""#) && l.contains(named_id(2).as_str())),
        "{logs:?}"
    );
    assert!(
        logs.iter().any(|l| l.contains(r#""event":"nft_mint""#)),
        "{logs:?}"
    );
    // Nothing is created: the only receipt is the grantor's refund.
    let receipts = get_created_receipts();
    assert!(
        receipts.iter().all(|r| r.receiver_id == grantor()),
        "{receipts:?}"
    );
}

#[test]
fn a_named_heir_costs_storage_only() {
    let mut e = new_estate();
    add_named_heir_at(&mut e, "kay", named_id(2), 5_000, 0);
    let kept = NAMED_HEIR_DEPOSIT.as_yoctonear() - transfers_to(&grantor()).iter().sum::<u128>();
    assert!(kept > 0);
    assert!(
        kept < NearToken::from_millinear(5).as_yoctonear(),
        "kept {kept}"
    );
}

#[test]
#[should_panic(expected = "attach more NEAR")]
fn a_named_heir_needs_its_storage_paid() {
    let mut e = new_estate();
    set_context(grantor(), 0);
    e.add_heir_account("kay".into(), named_id(2), "son".into(), 5_000);
}

#[test]
fn accounts_that_cannot_be_named_heirs_are_refused() {
    let estate_sub = heir_id("son");
    let deep_sub: AccountId = format!("a.b.{}", estate_id()).parse().unwrap();
    for (account, why) in [
        (estate_id(), "under this estate"),
        (estate_sub, "under this estate"),
        (deep_sub, "under this estate"),
        (grantor(), "the grantor cannot"),
    ] {
        let mut e = new_estate();
        set_context_paying(grantor(), 0, NAMED_HEIR_DEPOSIT);
        let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            e.add_heir_account("kay".into(), account.clone(), "son".into(), 5_000)
        }))
        .expect_err(&format!("{account} should be refused"));
        let msg = err
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| err.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        assert!(msg.contains(why), "{account}: {msg}");
    }
}

#[test]
fn an_account_on_another_estate_or_a_parent_is_fine() {
    // Only this estate's own sub-accounts are refused, not lookalikes.
    for account in [
        "son.bob.herit-near.testnet",
        "herit-near.testnet",
        "xalice.herit-near.testnet",
    ] {
        let mut e = new_estate();
        add_named_heir_at(&mut e, "kay", account.parse().unwrap(), 5_000, 0);
        assert!(e.heir_of("kay".into()).is_some(), "{account}");
    }
}

#[test]
#[should_panic(expected = "account is already an heir")]
fn one_account_holds_one_heir_slot() {
    let mut e = new_estate();
    add_named_heir_at(&mut e, "kay", named_id(2), 3_000, 0);
    add_named_heir_at(&mut e, "kay-again", named_id(2), 3_000, 0);
}

#[test]
#[should_panic(expected = "not the grantor")]
fn a_non_grantor_cannot_name_an_heir() {
    let mut e = new_estate();
    set_context_paying(stranger(), 0, NAMED_HEIR_DEPOSIT);
    e.add_heir_account("kay".into(), named_id(2), "son".into(), 5_000);
}

#[test]
#[should_panic(expected = "estate unlocked")]
fn no_heir_can_be_named_once_unlocked() {
    let mut e = new_estate();
    check_in_at(&mut e, CHECKED_IN_AT, 1);
    let (_, unlock_at) = e.deadlines_of();
    add_named_heir_at(&mut e, "kay", named_id(2), 5_000, unlock_at.0);
}

#[test]
fn named_heir_labels_follow_the_sub_account_rules() {
    for label in ["", "a.b", "Son", "a--b", "-son"] {
        let mut e = new_estate();
        let ok = call_reverting(&mut e, |e| {
            add_named_heir_at(e, label, named_id(2), 5_000, 0)
        });
        assert!(!ok, "{label:?} should be refused");
    }
}

#[test]
#[should_panic(expected = "label taken")]
fn both_kinds_share_one_set_of_labels() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 1_000);
    add_named_heir_at(&mut e, "son", named_id(2), 1_000, 0);
}

#[test]
#[should_panic(expected = "shares exceed 100%")]
fn both_kinds_share_one_100_percent() {
    let mut e = new_estate();
    add_named_heir_at(&mut e, "kay", named_id(2), 6_000, 0);
    add_heir_at(&mut e, "son", 4_001, 0);
}

#[test]
#[should_panic(expected = "too many heirs")]
fn both_kinds_share_the_ten_slots() {
    let mut e = new_estate();
    for i in 0..MAX_HEIRS as u8 {
        add_named_heir_at(&mut e, &format!("heir{i}"), named_id(10 + i), 1_000, 0);
    }
    add_heir_at(&mut e, "one-more", 0, 0);
}

#[test]
fn a_removed_named_heir_frees_its_label_for_either_kind() {
    let mut e = new_estate();
    add_named_heir_at(&mut e, "kay", named_id(2), 5_000, 0);
    set_context(grantor(), 0);
    e.remove_heir("kay".into());
    assert!(e.nft_token("kay".into()).is_none());
    // The account can be named again, and the label can become a sub-account.
    add_named_heir_at(&mut e, "kay2", named_id(2), 5_000, 0);
    add_heir_at(&mut e, "kay", 5_000, 0);
    assert_eq!(e.heir_of("kay".into()).unwrap().account_id, heir_id("kay"));
}
