//! Checkpoint 7: the vault, on the shape of `HeritVault` from the Sepolia build.

use near_sdk::NearToken;
use near_sdk::json_types::U128;
use near_sdk::test_utils::get_logs;
use proptest::prelude::*;

use super::testing::*;
use super::*;

/// What listing NEAR costs on the first deposit, measured on a fresh estate.
fn listing_cost() -> u128 {
    let mut e = new_estate();
    deposit_at(&mut e, grantor(), ONE_NEAR.as_yoctonear(), 0);
    ONE_NEAR.as_yoctonear() - e.balance_of(NEAR).0
}

// ---------- Part 1: NEAR deposits ----------

#[test]
fn a_new_estate_holds_nothing() {
    let e = new_estate();
    assert_eq!(e.balance_of(NEAR), U128(0));
    assert!(e.tokens_of().is_empty());
}

#[test]
fn the_first_deposit_lists_near_and_pays_for_that_storage() {
    let cost = listing_cost();
    // A map entry and a list entry, about 100 bytes: around 0.001 NEAR.
    assert!(
        cost > 0 && cost < NearToken::from_millinear(10).as_yoctonear(),
        "{cost}"
    );

    let mut e = new_estate();
    deposit_at(&mut e, grantor(), ONE_NEAR.as_yoctonear(), 0);
    assert_eq!(e.tokens_of(), vec![NEAR]);
    let credited = ONE_NEAR.as_yoctonear() - cost;
    assert_eq!(e.balance_of(NEAR), U128(credited));
    // The event reports what was credited, not what was attached.
    let expected =
        format!(r#""event":"deposited","data":[{{"asset":"near","amount":"{credited}"}}]"#);
    assert!(
        get_logs().iter().any(|l| l.contains(&expected)),
        "{:?}",
        get_logs()
    );
}

#[test]
fn later_deposits_are_credited_in_full_and_listed_once() {
    let mut e = new_estate();
    deposit_at(&mut e, grantor(), ONE_NEAR.as_yoctonear(), 0);
    let after_first = e.balance_of(NEAR).0;

    set_context_paying(grantor(), 0, NearToken::from_yoctonear(1));
    let before = env::storage_usage();
    e.deposit();
    assert_eq!(
        env::storage_usage(),
        before,
        "topping up writes no new storage"
    );
    assert_eq!(e.balance_of(NEAR), U128(after_first + 1));
    assert_eq!(e.tokens_of(), vec![NEAR]);
}

#[test]
#[should_panic(expected = "deposit too small to cover its storage")]
fn a_first_deposit_must_cover_its_storage() {
    let cost = listing_cost();
    let mut e = new_estate();
    deposit_at(&mut e, grantor(), cost, 0);
}

#[test]
#[should_panic(expected = "attach the NEAR to deposit")]
fn an_empty_deposit_is_refused() {
    let mut e = new_estate();
    deposit_at(&mut e, grantor(), 0, 0);
}

#[test]
#[should_panic(expected = "not the grantor")]
fn only_the_grantor_deposits() {
    let mut e = new_estate();
    deposit_at(&mut e, stranger(), ONE_NEAR.as_yoctonear(), 0);
}

#[test]
fn a_deposit_is_accepted_in_grace() {
    let mut e = new_estate();
    check_in_at(&mut e, CHECKED_IN_AT, 1);
    let (grace_at, _) = e.deadlines_of();
    deposit_at(&mut e, grantor(), ONE_NEAR.as_yoctonear(), grace_at.0);
    assert!(e.balance_of(NEAR).0 > 0);
}

#[test]
#[should_panic(expected = "estate unlocked")]
fn no_deposit_after_unlock_even_unpoked() {
    let mut e = new_estate();
    check_in_at(&mut e, CHECKED_IN_AT, 1);
    let (_, unlock_at) = e.deadlines_of();
    deposit_at(&mut e, grantor(), ONE_NEAR.as_yoctonear(), unlock_at.0);
}

proptest! {
    /// Any run of deposits, some refused: the balance is exactly the sum of the accepted ones,
    /// less the one-off listing cost on the first.
    #[test]
    fn balance_is_what_arrived_less_the_listing(
        deposits in prop::collection::vec((prop::bool::weighted(0.8), 0..=10u128.pow(25)), 1..20)
    ) {
        let cost = listing_cost();
        let mut e = new_estate();
        let mut expected = 0u128;
        let mut listed = false;
        for (by_grantor, amount) in deposits {
            let caller = if by_grantor { grantor() } else { stranger() };
            let ok = call_reverting(&mut e, |e| deposit_at(e, caller, amount, 0));
            let fee = if listed { 0 } else { cost };
            prop_assert_eq!(ok, by_grantor && amount > fee);
            if ok {
                expected += amount - fee;
                listed = true;
            }
            prop_assert_eq!(e.balance_of(NEAR), U128(expected));
            prop_assert_eq!(e.tokens_of().len(), listed as usize);
        }
    }
}

// ---------- Part 2: token deposits ----------

use near_sdk::mock::MockAction;
use near_sdk::test_utils::get_created_receipts;

#[test]
fn add_token_lists_it_and_registers_the_estate() {
    let mut e = new_estate();
    set_context_paying(grantor(), 0, NearToken::from_millinear(100));
    let _ = e.add_token(usdc());
    assert_eq!(e.tokens_of(), vec![Some(usdc())]);
    assert_eq!(e.balance_of(Some(usdc())), U128(0));

    let receipts = get_created_receipts();
    let call = receipts
        .iter()
        .find(|r| r.receiver_id == usdc())
        .expect("a call to the token");
    match &call.actions[0] {
        MockAction::FunctionCallWeight {
            method_name,
            args,
            attached_deposit,
            ..
        } => {
            assert_eq!(method_name, b"storage_deposit");
            assert_eq!(*attached_deposit, TOKEN_REGISTRATION);
            let args: near_sdk::serde_json::Value = near_sdk::serde_json::from_slice(args).unwrap();
            assert_eq!(args["account_id"], near_sdk::serde_json::json!(estate_id()));
            assert_eq!(args["registration_only"], near_sdk::serde_json::json!(true));
        }
        other => panic!("expected storage_deposit, got {other:?}"),
    }
    let callback = receipts.iter().flat_map(|r| &r.actions).any(|a| {
        matches!(a, MockAction::FunctionCallWeight { method_name, .. } if method_name == b"on_token_registered")
    });
    assert!(callback);
    // The surplus goes back to the grantor.
    let refunded: u128 = receipts
        .iter()
        .filter(|r| r.receiver_id == grantor())
        .flat_map(|r| &r.actions)
        .filter_map(|a| match a {
            MockAction::Transfer { deposit, .. } => Some(deposit.as_yoctonear()),
            _ => None,
        })
        .sum();
    let kept = NearToken::from_millinear(100).as_yoctonear() - refunded;
    assert!(
        kept > TOKEN_REGISTRATION.as_yoctonear()
            && kept < NearToken::from_millinear(10).as_yoctonear()
    );
}

#[test]
#[should_panic(expected = "attach more NEAR")]
fn add_token_needs_the_registration_fee() {
    let mut e = new_estate();
    set_context_paying(grantor(), 0, TOKEN_REGISTRATION);
    let _ = e.add_token(usdc());
}

#[test]
#[should_panic(expected = "token already added")]
fn a_token_cannot_be_added_twice() {
    let mut e = new_estate();
    add_registered_token(&mut e, usdc());
    add_token_at(&mut e, usdc(), 0);
}

#[test]
#[should_panic(expected = "not the grantor")]
fn only_the_grantor_adds_tokens() {
    let mut e = new_estate();
    set_context_paying(stranger(), 0, NearToken::from_millinear(100));
    let _ = e.add_token(usdc());
}

#[test]
#[should_panic(expected = "estate unlocked")]
fn no_token_can_be_added_once_unlocked() {
    let mut e = new_estate();
    check_in_at(&mut e, CHECKED_IN_AT, 1);
    let (_, unlock_at) = e.deadlines_of();
    add_token_at(&mut e, usdc(), unlock_at.0);
}

#[test]
#[should_panic(expected = "too many assets")]
fn near_and_nine_tokens_fill_the_list() {
    let mut e = new_estate();
    deposit_at(&mut e, grantor(), ONE_NEAR.as_yoctonear(), 0);
    for n in 0..MAX_TOKENS - 1 {
        add_registered_token(&mut e, token(n));
    }
    add_token_at(&mut e, token(99), 0);
}

#[test]
#[should_panic(expected = "too many assets")]
fn ten_tokens_leave_no_room_for_near() {
    let mut e = new_estate();
    for n in 0..MAX_TOKENS {
        add_registered_token(&mut e, token(n));
    }
    deposit_at(&mut e, grantor(), ONE_NEAR.as_yoctonear(), 0);
}

#[test]
fn a_registered_token_is_logged() {
    let mut e = new_estate();
    add_registered_token(&mut e, usdc());
    assert!(
        get_logs()
            .iter()
            .any(|l| l.contains(r#""event":"token_added""#))
    );
}

#[test]
fn a_failed_registration_unlists_the_token_and_refunds_the_grantor() {
    let mut e = new_estate();
    deposit_at(&mut e, grantor(), ONE_NEAR.as_yoctonear(), 0);
    add_registered_token(&mut e, token(1));
    add_token_at(&mut e, usdc(), 0);
    add_registered_token(&mut e, token(2));

    set_callback_context(false);
    assert!(!e.on_token_registered(usdc(), U128(777)));
    // The others keep their order.
    assert_eq!(e.tokens_of(), vec![None, Some(token(1)), Some(token(2))]);
    assert_eq!(e.balance_of(Some(usdc())), U128(0));
    let refunds: Vec<u128> = get_created_receipts()
        .into_iter()
        .filter(|r| r.receiver_id == grantor())
        .flat_map(|r| r.actions)
        .filter_map(|a| match a {
            MockAction::Transfer { deposit, .. } => Some(deposit.as_yoctonear()),
            _ => None,
        })
        .collect();
    assert_eq!(refunds, vec![777]);
    // It can be added again.
    add_token_at(&mut e, usdc(), 0);
}

#[test]
fn the_grantors_token_deposit_is_credited_in_full() {
    let mut e = new_estate();
    add_registered_token(&mut e, usdc());
    assert_eq!(
        ft_deposit_at(&mut e, usdc(), grantor(), 5_000_000, 0),
        U128(0)
    );
    assert_eq!(ft_deposit_at(&mut e, usdc(), grantor(), 1, 0), U128(0));
    assert_eq!(e.balance_of(Some(usdc())), U128(5_000_001));
    assert_eq!(e.balance_of(NEAR), U128(0));
    let expected = format!(
        r#""event":"deposited","data":[{{"asset":"{}","amount":"1"}}]"#,
        usdc()
    );
    assert!(get_logs().iter().any(|l| l.contains(&expected)));
}

#[test]
#[should_panic(expected = "token not added")]
fn a_token_that_was_not_added_is_refused() {
    let mut e = new_estate();
    ft_deposit_at(&mut e, usdc(), grantor(), 1, 0);
}

#[test]
#[should_panic(expected = "only the grantor deposits")]
fn a_strangers_token_deposit_is_refused() {
    let mut e = new_estate();
    add_registered_token(&mut e, usdc());
    ft_deposit_at(&mut e, usdc(), stranger(), 1, 0);
}

#[test]
#[should_panic(expected = "estate unlocked")]
fn no_token_deposit_after_unlock() {
    let mut e = new_estate();
    add_registered_token(&mut e, usdc());
    check_in_at(&mut e, CHECKED_IN_AT, 1);
    let (_, unlock_at) = e.deadlines_of();
    ft_deposit_at(&mut e, usdc(), grantor(), 1, unlock_at.0);
}

#[test]
#[should_panic(expected = "zero amount")]
fn a_zero_token_deposit_is_refused() {
    let mut e = new_estate();
    add_registered_token(&mut e, usdc());
    ft_deposit_at(&mut e, usdc(), grantor(), 0, 0);
}

proptest! {
    /// Random token deposits from random tokens and senders: each balance is exactly the sum
    /// of that token's accepted deposits, and nothing leaks between assets.
    #[test]
    fn each_token_balance_is_the_sum_of_its_accepted_deposits(
        deposits in prop::collection::vec((0..4u32, prop::bool::weighted(0.8), 0..=10u128.pow(24)), 1..30)
    ) {
        let mut e = new_estate();
        // token0 and token1 added; token2 and token3 never.
        add_registered_token(&mut e, token(0));
        add_registered_token(&mut e, token(1));
        let mut expected = [0u128; 4];
        for (t, by_grantor, amount) in deposits {
            let sender = if by_grantor { grantor() } else { stranger() };
            let ok = call_reverting(&mut e, |e| {
                ft_deposit_at(e, token(t), sender, amount, 0);
            });
            prop_assert_eq!(ok, t < 2 && by_grantor && amount > 0);
            if ok {
                expected[t as usize] += amount;
            }
            for n in 0..4 {
                prop_assert_eq!(e.balance_of(Some(token(n))), U128(expected[n as usize]));
            }
            prop_assert_eq!(e.balance_of(NEAR), U128(0));
        }
    }
}

// ---------- Part 3: per-asset shares ----------

/// The grantor sets `label`'s share of `asset`, attaching enough for a new override.
fn set_share_at(e: &mut Estate, label: &str, asset: Asset, bps: u16) {
    set_context_paying(grantor(), 0, NearToken::from_millinear(10));
    e.set_share(label.into(), asset, bps);
}

/// son 60%, daughter 40% by default.
fn estate_with_two_heirs() -> Estate {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 6_000);
    add_created_heir(&mut e, "daughter", 4_000);
    e
}

#[test]
fn shares_default_to_each_heirs_default() {
    let e = estate_with_two_heirs();
    assert_eq!(e.share_of("son".into(), NEAR), 6_000);
    assert_eq!(e.share_of("daughter".into(), Some(usdc())), 4_000);
    assert_eq!(e.share_of("nobody".into(), NEAR), 0);
    assert_eq!(e.total_share_of(NEAR), 10_000);
}

#[test]
fn an_override_applies_to_one_asset_only() {
    let mut e = estate_with_two_heirs();
    set_share_at(&mut e, "daughter", Some(usdc()), 1_000);
    assert_eq!(e.share_of("daughter".into(), Some(usdc())), 1_000);
    assert_eq!(e.share_of("daughter".into(), NEAR), 4_000);
    assert_eq!(e.total_share_of(Some(usdc())), 7_000);
    assert_eq!(e.total_share_of(NEAR), 10_000);
    let expected = format!(
        r#""event":"share_set","data":[{{"label":"daughter","asset":"{}","bps":1000}}]"#,
        usdc()
    );
    assert!(
        get_logs().iter().any(|l| l.contains(&expected)),
        "{:?}",
        get_logs()
    );
}

#[test]
fn the_check_is_on_the_prospective_total() {
    let mut e = estate_with_two_heirs();
    set_share_at(&mut e, "son", Some(usdc()), 5_000); // usdc: 5,000 + 4,000
    set_share_at(&mut e, "daughter", Some(usdc()), 5_000); // exactly 100%
    assert_eq!(e.total_share_of(Some(usdc())), 10_000);
    let over = call_reverting(&mut e, |e| set_share_at(e, "daughter", Some(usdc()), 5_001));
    assert!(!over, "5,000 + 5,001 is over 100%");
}

#[test]
fn a_share_can_be_lowered_or_restated_at_100_percent() {
    let mut e = estate_with_two_heirs();
    set_share_at(&mut e, "son", NEAR, 6_000); // same value: the total stays 100%
    set_share_at(&mut e, "son", NEAR, 1_000);
    assert_eq!(e.total_share_of(NEAR), 5_000);
}

#[test]
#[should_panic(expected = "shares exceed 100%")]
fn raising_a_share_past_100_percent_is_refused() {
    let mut e = estate_with_two_heirs();
    set_share_at(&mut e, "son", NEAR, 6_001);
}

#[test]
#[should_panic(expected = "no such heir")]
fn an_unknown_heir_has_no_share_to_set() {
    let mut e = estate_with_two_heirs();
    set_share_at(&mut e, "nobody", NEAR, 0);
}

#[test]
#[should_panic(expected = "not the grantor")]
fn only_the_grantor_sets_shares() {
    let mut e = estate_with_two_heirs();
    set_context_paying(stranger(), 0, NearToken::from_millinear(10));
    e.set_share("son".into(), NEAR, 1);
}

#[test]
#[should_panic(expected = "estate unlocked")]
fn no_share_can_change_once_unlocked() {
    let mut e = estate_with_two_heirs();
    check_in_at(&mut e, CHECKED_IN_AT, 1);
    let (_, unlock_at) = e.deadlines_of();
    set_context_paying(grantor(), unlock_at.0, NearToken::from_millinear(10));
    e.set_share("son".into(), NEAR, 1);
}

#[test]
#[should_panic(expected = "attach more NEAR")]
fn a_new_override_pays_for_its_storage() {
    let mut e = estate_with_two_heirs();
    set_context(grantor(), 0); // nothing attached
    e.set_share("son".into(), NEAR, 1_000);
}

#[test]
fn changing_an_existing_override_is_free_and_the_surplus_is_refunded() {
    let mut e = estate_with_two_heirs();
    set_share_at(&mut e, "son", NEAR, 1_000);
    let refunded: u128 = get_created_receipts()
        .into_iter()
        .filter(|r| r.receiver_id == grantor())
        .flat_map(|r| r.actions)
        .filter_map(|a| match a {
            MockAction::Transfer { deposit, .. } => Some(deposit.as_yoctonear()),
            _ => None,
        })
        .sum();
    let kept = NearToken::from_millinear(10).as_yoctonear() - refunded;
    assert!(
        kept > 0 && kept < NearToken::from_millinear(5).as_yoctonear(),
        "{kept}"
    );

    set_context(grantor(), 0); // nothing attached
    e.set_share("son".into(), NEAR, 2_000);
    assert_eq!(e.share_of("son".into(), NEAR), 2_000);
}

#[test]
#[should_panic(expected = "shares exceed 100%")]
fn a_new_heirs_default_is_checked_against_overridden_assets() {
    // Defaults total 50%, but usdc is overridden to 100% for son.
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 5_000);
    set_share_at(&mut e, "son", Some(usdc()), 10_000);
    add_heir_at(&mut e, "daughter", 1, 0);
}

#[test]
#[should_panic(expected = "too many assets with overrides")]
fn at_most_ten_assets_carry_overrides() {
    let mut e = estate_with_two_heirs();
    for n in 0..MAX_OVERRIDE_TOKENS {
        set_share_at(&mut e, "son", Some(token(n)), 0);
    }
    set_share_at(&mut e, "son", NEAR, 0);
}

#[test]
fn removing_an_heir_drops_their_overrides() {
    let mut e = estate_with_two_heirs();
    set_share_at(&mut e, "son", Some(usdc()), 1_000);
    set_context(grantor(), 0);
    e.remove_heir("son".into());
    assert_eq!(e.total_share_of(Some(usdc())), 4_000);
    // A new heir under the same label starts from their default, not the old override.
    add_created_heir(&mut e, "son", 6_000);
    assert_eq!(e.share_of("son".into(), Some(usdc())), 6_000);
}

#[test]
fn a_failed_heir_creation_drops_its_overrides() {
    let mut e = new_estate();
    add_heir_at(&mut e, "son", 5_000, 0);
    set_share_at(&mut e, "son", Some(usdc()), 1_000); // set while the account is pending
    set_callback_context(false);
    e.on_heir_created("son".into(), U128(0));
    add_created_heir(&mut e, "son", 5_000);
    assert_eq!(e.share_of("son".into(), Some(usdc())), 5_000);
}

#[derive(Debug, Clone)]
enum ShareOp {
    Add(usize, u16),
    Remove(usize),
    Set(usize, usize, u16),
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Random adds, removes and share changes: every asset's total stays at or under 100%,
    /// equals the sum of the heirs' shares, and a refused call changes nothing.
    #[test]
    fn every_assets_shares_stay_within_100_percent(
        ops in prop::collection::vec(prop_oneof![
            (0..4usize, 0..=6_000u16).prop_map(|(l, b)| ShareOp::Add(l, b)),
            (0..4usize).prop_map(ShareOp::Remove),
            (0..4usize, 0..3usize, 0..=6_000u16).prop_map(|(l, a, b)| ShareOp::Set(l, a, b)),
        ], 1..40)
    ) {
        let labels = ["son", "daughter", "c", "d"];
        let assets: [Asset; 3] = [None, Some(usdc()), Some(token(1))];
        let mut e = new_estate();
        for op in ops {
            let before: Vec<Vec<u16>> = assets.iter().map(|a| {
                labels.iter().map(|l| e.share_of((*l).into(), a.clone())).collect()
            }).collect();
            let ok = call_reverting(&mut e, |e| match &op {
                ShareOp::Add(l, b) => add_created_heir(e, labels[*l], *b),
                ShareOp::Remove(l) => {
                    set_context(grantor(), 0);
                    e.remove_heir(labels[*l].into());
                }
                ShareOp::Set(l, a, b) => set_share_at(e, labels[*l], assets[*a].clone(), *b),
            });
            for (i, asset) in assets.iter().enumerate() {
                let shares: Vec<u16> = labels.iter().map(|l| e.share_of((*l).into(), asset.clone())).collect();
                let sum: u32 = shares.iter().map(|s| *s as u32).sum();
                prop_assert!(sum <= BPS, "{:?} totals {}", asset, sum);
                prop_assert_eq!(e.total_share_of(asset.clone()) as u32, sum);
                if !ok {
                    prop_assert_eq!(&shares, &before[i], "a refused {:?} changed shares", op);
                }
            }
        }
    }
}

// ---------- Part 4: withdrawals with a fresh Selfie Check ----------

/// Checked in (face bound, nonce 1) with 3 NEAR and 5 mUSDC deposited.
fn funded_estate() -> Estate {
    let mut e = new_estate();
    check_in_at(&mut e, CHECKED_IN_AT, 1);
    deposit_at(
        &mut e,
        grantor(),
        3 * ONE_NEAR.as_yoctonear(),
        CHECKED_IN_AT,
    );
    add_registered_token(&mut e, usdc());
    ft_deposit_at(&mut e, usdc(), grantor(), 5_000_000, CHECKED_IN_AT);
    e
}

fn near_held(e: &Estate) -> u128 {
    e.balance_of(NEAR).0
}

#[test]
fn a_near_withdrawal_is_debited_first_and_sent_to_the_grantor() {
    let mut e = funded_estate();
    let held = near_held(&e);
    withdraw_with(
        &mut e,
        NEAR,
        ONE_NEAR.as_yoctonear(),
        &withdraw_att(2, CHECKED_IN_AT),
        CHECKED_IN_AT,
    );

    assert_eq!(
        near_held(&e),
        held - ONE_NEAR.as_yoctonear(),
        "debited before the transfer runs"
    );
    let receipts = get_created_receipts();
    let sent = receipts
        .iter()
        .filter(|r| r.receiver_id == grantor())
        .flat_map(|r| &r.actions)
        .any(|a| matches!(a, MockAction::Transfer { deposit, .. } if *deposit == ONE_NEAR));
    assert!(sent);
    let callback = receipts.iter().flat_map(|r| &r.actions).any(|a| {
        matches!(a, MockAction::FunctionCallWeight { method_name, .. } if method_name == b"on_withdrawn")
    });
    assert!(callback);
}

#[test]
fn a_token_withdrawal_calls_ft_transfer_to_the_grantor() {
    let mut e = funded_estate();
    withdraw_with(
        &mut e,
        Some(usdc()),
        2_000_000,
        &withdraw_att(2, CHECKED_IN_AT),
        CHECKED_IN_AT,
    );
    assert_eq!(e.balance_of(Some(usdc())), U128(3_000_000));

    let call = get_created_receipts()
        .into_iter()
        .find(|r| r.receiver_id == usdc())
        .expect("a call to the token");
    match &call.actions[0] {
        MockAction::FunctionCallWeight {
            method_name,
            args,
            attached_deposit,
            ..
        } => {
            assert_eq!(method_name, b"ft_transfer");
            assert_eq!(*attached_deposit, ONE_YOCTO);
            let args: near_sdk::serde_json::Value = near_sdk::serde_json::from_slice(args).unwrap();
            assert_eq!(args["receiver_id"], near_sdk::serde_json::json!(grantor()));
            assert_eq!(args["amount"], near_sdk::serde_json::json!("2000000"));
        }
        other => panic!("expected ft_transfer, got {other:?}"),
    }
}

#[test]
fn a_successful_transfer_is_logged() {
    let mut e = funded_estate();
    withdraw_with(
        &mut e,
        NEAR,
        1,
        &withdraw_att(2, CHECKED_IN_AT),
        CHECKED_IN_AT,
    );
    let held = near_held(&e);
    set_callback_context(true);
    assert!(e.on_withdrawn(NEAR, U128(1)));
    assert_eq!(near_held(&e), held);
    assert!(
        get_logs()
            .iter()
            .any(|l| l.contains(r#""event":"withdrawn","data":[{"asset":"near","amount":"1"}]"#))
    );
}

#[test]
fn a_failed_transfer_is_credited_back() {
    let mut e = funded_estate();
    withdraw_with(
        &mut e,
        Some(usdc()),
        2_000_000,
        &withdraw_att(2, CHECKED_IN_AT),
        CHECKED_IN_AT,
    );
    set_callback_context(false);
    assert!(!e.on_withdrawn(Some(usdc()), U128(2_000_000)));
    assert_eq!(e.balance_of(Some(usdc())), U128(5_000_000));
    assert!(
        get_logs()
            .iter()
            .any(|l| l.contains(r#""event":"withdraw_failed""#))
    );
}

#[test]
fn a_withdrawal_is_allowed_in_grace() {
    let mut e = funded_estate();
    let (grace_at, _) = e.deadlines_of();
    withdraw_with(&mut e, NEAR, 1, &withdraw_att(2, grace_at.0), grace_at.0);
}

#[test]
fn everything_can_be_withdrawn() {
    let mut e = funded_estate();
    let all = near_held(&e);
    withdraw_with(
        &mut e,
        NEAR,
        all,
        &withdraw_att(2, CHECKED_IN_AT),
        CHECKED_IN_AT,
    );
    assert_eq!(near_held(&e), 0);
}

#[test]
fn refused_withdrawals() {
    let now = CHECKED_IN_AT;
    let (_, unlock_at) = funded_estate().deadlines_of();
    let other_face = {
        let mut a = withdraw_att(2, now);
        a.commitment = OTHER_FACE;
        a
    };
    let check_in_action = Att::check_in(2, now);
    let with_label = {
        let mut a = withdraw_att(2, now);
        a.heir_label = Some("son".into());
        a
    };
    let by_stranger = {
        let mut a = withdraw_att(2, now);
        a.subject = stranger();
        a
    };
    let stolen_key = withdraw_att(2, now).signed_by_another_key();
    let replayed = withdraw_att(1, now); // nonce 1 went to the check-in
    let cases: Vec<(&str, Att, Asset, u128, u64)> = vec![
        ("wrong human", other_face, NEAR, 1, now),
        ("a check-in attestation", check_in_action, NEAR, 1, now),
        ("an heir label", with_label, NEAR, 1, now),
        ("not the grantor", by_stranger, NEAR, 1, now),
        ("signed by another key", stolen_key, NEAR, 1, now),
        ("a replayed nonce", replayed, NEAR, 1, now),
        ("zero", withdraw_att(2, now), NEAR, 0, now),
        (
            "more than held",
            withdraw_att(2, now),
            Some(usdc()),
            5_000_001,
            now,
        ),
        (
            "an asset not held",
            withdraw_att(2, now),
            Some(token(7)),
            1,
            now,
        ),
        (
            "after unlock",
            withdraw_att(2, unlock_at.0),
            NEAR,
            1,
            unlock_at.0,
        ),
    ];
    for (why, att, asset, amount, at) in cases {
        let mut e = funded_estate();
        let before = (e.balance_of(NEAR), e.balance_of(Some(usdc())));
        let ok = call_reverting(&mut e, |e| {
            withdraw_with(e, asset.clone(), amount, &att, at)
        });
        assert!(!ok, "{why} should be refused");
        let after = (e.balance_of(NEAR), e.balance_of(Some(usdc())));
        assert_eq!(after, before, "{why} changed a balance");
    }
}

#[test]
#[should_panic(expected = "check in first")]
fn no_withdrawal_before_a_face_is_bound() {
    let mut e = new_estate();
    deposit_at(&mut e, grantor(), ONE_NEAR.as_yoctonear(), 0);
    withdraw_with(&mut e, NEAR, 1, &withdraw_att(1, 0), 0);
}

#[test]
#[should_panic(expected = "exactly 1 yoctoNEAR")]
fn a_withdrawal_needs_exactly_one_yocto() {
    let mut e = funded_estate();
    set_context(grantor(), CHECKED_IN_AT);
    let (attestation, signature) = withdraw_att(2, CHECKED_IN_AT).sign();
    let _ = e.withdraw(NEAR, U128(1), attestation, signature);
}

#[derive(Debug, Clone)]
enum VaultOp {
    Deposit(u128),
    Withdraw(u128),
    /// Settle the oldest in-flight withdrawal, as its transfer succeeds or fails.
    Settle(bool),
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Deposits, withdrawals and transfers that succeed or fail, in any order: the balance is
    /// always deposits less withdrawals that are in flight or went through. A failed transfer
    /// gives its amount back; a refused withdrawal changes nothing.
    #[test]
    fn the_books_follow_the_money(
        ops in prop::collection::vec(prop_oneof![
            (1..=10u128.pow(24)).prop_map(VaultOp::Deposit),
            (0..=2 * 10u128.pow(24)).prop_map(VaultOp::Withdraw),
            any::<bool>().prop_map(VaultOp::Settle),
        ], 1..40)
    ) {
        let mut e = new_estate();
        check_in_at(&mut e, CHECKED_IN_AT, 1);
        deposit_at(&mut e, grantor(), ONE_NEAR.as_yoctonear(), CHECKED_IN_AT);
        let mut expected = near_held(&e);
        let mut in_flight: std::collections::VecDeque<u128> = Default::default();
        let mut nonce = 1;
        for op in ops {
            match op {
                VaultOp::Deposit(amount) => {
                    deposit_at(&mut e, grantor(), amount, CHECKED_IN_AT);
                    expected += amount;
                }
                VaultOp::Withdraw(amount) => {
                    nonce += 1;
                    let att = withdraw_att(nonce, CHECKED_IN_AT);
                    let ok = call_reverting(&mut e, |e| withdraw_with(e, NEAR, amount, &att, CHECKED_IN_AT));
                    prop_assert_eq!(ok, amount > 0 && amount <= expected);
                    if ok {
                        expected -= amount;
                        in_flight.push_back(amount);
                    }
                }
                VaultOp::Settle(sent) => {
                    if let Some(amount) = in_flight.pop_front() {
                        set_callback_context(sent);
                        prop_assert_eq!(e.on_withdrawn(NEAR, U128(amount)), sent);
                        if !sent {
                            expected += amount;
                        }
                    }
                }
            }
            prop_assert_eq!(near_held(&e), expected);
        }
    }
}
