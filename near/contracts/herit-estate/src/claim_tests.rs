//! Checkpoint 8: claims, on the shape of `ClaimManager` and `HeritVault.snapshot` from Sepolia.

use near_sdk::json_types::U128;
use near_sdk::test_utils::get_logs;
use proptest::prelude::*;

use super::testing::*;
use super::*;

// ---------- Part 1: the snapshot at unlock ----------

#[test]
fn there_is_no_snapshot_before_unlock() {
    let (mut e, unlock_at) = funded_estate();
    assert!(!e.snapshot_taken());
    assert_eq!(e.snapshot_of(NEAR), U128(0));
    // Grace doesn't take one either.
    let (grace_at, _) = e.deadlines_of();
    assert_eq!(poke_at(&mut e, grace_at.0), Status::Grace);
    assert!(!e.snapshot_taken());
    assert!(unlock_at > grace_at.0);
}

#[test]
fn unlocking_freezes_every_balance() {
    let (mut e, unlock_at) = funded_estate();
    assert_eq!(poke_at(&mut e, unlock_at), Status::Unlocked);

    assert!(e.snapshot_taken());
    assert_eq!(e.snapshot_of(NEAR), e.balance_of(NEAR));
    assert_eq!(e.snapshot_of(Some(usdc())), U128(5_000_000));
    assert_eq!(
        e.snapshot_of(Some(token(9))),
        U128(0),
        "an asset never held"
    );
    let logs = get_logs();
    assert!(
        logs.iter()
            .any(|l| l.contains(r#""event":"snapshotted","data":[{"assets":2}]"#)),
        "{logs:?}"
    );
    assert!(
        logs.iter()
            .any(|l| l.contains(r#""event":"unlocked""#) && l.contains(r#""heirs":2"#)),
        "{logs:?}"
    );
}

#[test]
fn an_unpoked_unlock_has_no_snapshot_yet() {
    // The clock says Unlocked, but nobody has poked: the snapshot waits for the first poke, which
    // a claim does itself (Part 2).
    let (e, unlock_at) = funded_estate();
    set_context(stranger(), unlock_at);
    assert_eq!(e.status_of(), Status::Unlocked);
    assert!(!e.snapshot_taken());
}

#[test]
fn the_snapshot_is_taken_once() {
    let (mut e, unlock_at) = funded_estate();
    poke_at(&mut e, unlock_at);
    let frozen = e.snapshot_of(NEAR);
    // A payout lowers the balance (Part 3 does it for real); the ruler must not move.
    *e.balances.get_mut(&NEAR).unwrap() -= 1;
    poke_at(&mut e, unlock_at + NS_PER_SEC);
    e.snapshot();
    assert_eq!(e.snapshot_of(NEAR), frozen);
}

#[test]
fn taking_the_snapshot_writes_no_new_storage() {
    let (mut e, unlock_at) = funded_estate();
    e.snapshots.flush();
    set_context(stranger(), unlock_at);
    let before = env::storage_usage();
    e.poke_expiry();
    e.snapshots.flush();
    assert_eq!(
        env::storage_usage(),
        before,
        "listing already paid for every entry"
    );
}

#[test]
fn a_withdrawal_that_fails_after_unlock_joins_the_snapshot() {
    let (mut e, unlock_at) = funded_estate();
    let (grace_at, _) = e.deadlines_of();
    // Sent in Grace...
    withdraw_with(
        &mut e,
        Some(usdc()),
        2_000_000,
        &withdraw_att(2, grace_at.0),
        grace_at.0,
    );
    // ...unlocked while in flight: the snapshot sees 3 mUSDC...
    poke_at(&mut e, unlock_at);
    assert_eq!(e.snapshot_of(Some(usdc())), U128(3_000_000));
    // ...then the transfer fails: the 2 mUSDC is back, and the heirs share it.
    set_callback_context(false);
    assert!(!e.on_withdrawn(Some(usdc()), U128(2_000_000)));
    assert_eq!(e.balance_of(Some(usdc())), U128(5_000_000));
    assert_eq!(e.snapshot_of(Some(usdc())), U128(5_000_000));
}

#[test]
fn a_withdrawal_that_fails_before_unlock_leaves_no_snapshot() {
    let (mut e, _) = funded_estate();
    withdraw_with(
        &mut e,
        Some(usdc()),
        2_000_000,
        &withdraw_att(2, CHECKED_IN_AT),
        CHECKED_IN_AT,
    );
    set_callback_context(false);
    e.on_withdrawn(Some(usdc()), U128(2_000_000));
    assert!(!e.snapshot_taken());
    assert_eq!(e.snapshot_of(Some(usdc())), U128(0));
}

proptest! {
    /// Any mix of deposits across NEAR and two tokens: the snapshot at unlock equals every
    /// balance exactly.
    #[test]
    fn the_snapshot_equals_the_balances_at_unlock(
        deposits in prop::collection::vec((0..3u32, 10u128.pow(22)..=10u128.pow(24)), 1..15)
    ) {
        let mut e = new_estate();
        check_in_at(&mut e, CHECKED_IN_AT, 1);
        add_registered_token(&mut e, token(1));
        add_registered_token(&mut e, token(2));
        for (asset, amount) in deposits {
            if asset == 0 {
                deposit_at(&mut e, grantor(), amount, CHECKED_IN_AT);
            } else {
                ft_deposit_at(&mut e, token(asset), grantor(), amount, CHECKED_IN_AT);
            }
        }
        let (_, unlock_at) = e.deadlines_of();
        poke_at(&mut e, unlock_at.0);
        for asset in [NEAR, Some(token(1)), Some(token(2))] {
            prop_assert_eq!(e.snapshot_of(asset.clone()), e.balance_of(asset));
        }
    }
}

// ---------- Part 2: who may claim ----------

use near_sdk::NearToken;
use near_sdk::json_types::Base64VecU8;

fn face_b64(face: [u8; 32]) -> Base64VecU8 {
    Base64VecU8(face.to_vec())
}

#[test]
fn an_heir_claims_after_the_deadline_and_the_claim_unlocks_the_estate() {
    let (mut e, unlock_at) = funded_estate();
    assert_eq!(e.status, Status::Active, "nobody poked");
    claim_with(
        &mut e,
        "son",
        &claim_att("son", SON_FACE, 10, unlock_at),
        unlock_at,
        CLAIM_DEPOSIT,
    );

    assert_eq!(e.status, Status::Unlocked, "the claim poked");
    assert!(e.snapshot_taken());
    assert_eq!(
        e.claim_commitment_used(face_b64(SON_FACE)),
        Some("son".into())
    );
    assert!(
        get_logs().iter().any(
            |l| l.contains(r#""event":"claim_attested","data":[{"label":"son","nonce":"10"}]"#)
        )
    );
}

#[test]
fn no_claim_before_unlock() {
    let (mut e, unlock_at) = funded_estate();
    let (grace_at, _) = e.deadlines_of();
    for now in [CHECKED_IN_AT, grace_at.0, unlock_at - 1] {
        let att = claim_att("son", SON_FACE, 10, now);
        let ok = call_reverting(&mut e, |e| claim_with(e, "son", &att, now, CLAIM_DEPOSIT));
        assert!(!ok, "claimed at {now}");
        assert_eq!(
            e.status,
            Status::Active,
            "a refused claim's poke is rolled back too"
        );
    }
}

#[test]
fn refused_claims() {
    let (_, unlock_at) = funded_estate();
    let now = unlock_at;
    let mut by_grantor_account = claim_att("son", SON_FACE, 10, now);
    by_grantor_account.subject = grantor();
    let mut by_sibling = claim_att("son", SON_FACE, 10, now);
    by_sibling.subject = heir_id("daughter");
    let mut wrong_action = claim_att("son", SON_FACE, 10, now);
    wrong_action.action = ACTION_CHECKIN;
    let mut no_label = claim_att("son", SON_FACE, 10, now);
    no_label.heir_label = None;
    let cases: Vec<(&str, &str, Att, &str)> = vec![
        (
            "no such heir",
            "nobody",
            claim_att("nobody", SON_FACE, 10, now),
            "no such heir",
        ),
        (
            "from the grantor's account",
            "son",
            by_grantor_account,
            "not entitled",
        ),
        (
            "from a sibling's account",
            "son",
            by_sibling,
            "not entitled",
        ),
        (
            "signed for the daughter",
            "son",
            claim_att("daughter", SON_FACE, 10, now),
            "not entitled",
        ),
        (
            "no label in the attestation",
            "son",
            no_label,
            "another label",
        ),
        (
            "the grantor's face",
            "son",
            claim_att("son", GRANTOR_FACE, 10, now),
            "wrong human",
        ),
        (
            "a check-in attestation",
            "son",
            wrong_action,
            "wrong action",
        ),
        (
            "another key",
            "son",
            claim_att("son", SON_FACE, 10, now).signed_by_another_key(),
            "invalid signature",
        ),
        (
            "a used nonce",
            "son",
            claim_att("son", SON_FACE, 1, now),
            "nonce used",
        ),
    ];
    for (why, label, att, expected) in cases {
        let (mut e, _) = funded_estate();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            claim_with(&mut e, label, &att, now, CLAIM_DEPOSIT)
        }));
        let message = result
            .expect_err(why)
            .downcast_ref::<String>()
            .cloned()
            .unwrap_or_default();
        assert!(
            message.contains(expected),
            "{why}: expected {expected:?}, got {message:?}"
        );
    }
}

#[test]
#[should_panic(expected = "commitment used: one human, one heir slot")]
fn one_human_cannot_claim_two_slots() {
    let (mut e, unlock_at) = funded_estate();
    claim_with(
        &mut e,
        "son",
        &claim_att("son", SON_FACE, 10, unlock_at),
        unlock_at,
        CLAIM_DEPOSIT,
    );
    // The same face, now claiming the daughter's slot from her account.
    claim_with(
        &mut e,
        "daughter",
        &claim_att("daughter", SON_FACE, 11, unlock_at),
        unlock_at,
        CLAIM_DEPOSIT,
    );
}

#[test]
fn the_same_human_may_retry_their_own_slot_after_a_failed_payout() {
    let (mut e, unlock_at) = funded_estate();
    let att = claim_att("son", SON_FACE, 10, unlock_at);
    claim_with(&mut e, "son", &att, unlock_at, CLAIM_DEPOSIT);
    // The NEAR transfer fails (say the account was deleted); the mUSDC one went through.
    set_callback_context(false);
    let near_share = e.payout_amount(&NEAR, 6_000);
    assert!(!e.on_paid("son".into(), NEAR, U128(near_share)));
    // The same face retries: only the NEAR is sent again.
    set_context_paying(heir_id("son"), unlock_at, CLAIM_DEPOSIT);
    let (attestation, signature) = claim_att("son", SON_FACE, 11, unlock_at).sign();
    assert_eq!(e.claim("son".into(), attestation, signature), 1);
    assert_eq!(
        e.claim_commitment_used(face_b64(SON_FACE)),
        Some("son".into())
    );
}

#[test]
fn two_heirs_with_their_own_faces_both_claim() {
    let (mut e, unlock_at) = funded_estate();
    claim_with(
        &mut e,
        "son",
        &claim_att("son", SON_FACE, 10, unlock_at),
        unlock_at,
        CLAIM_DEPOSIT,
    );
    claim_with(
        &mut e,
        "daughter",
        &claim_att("daughter", DAUGHTER_FACE, 11, unlock_at),
        unlock_at,
        CLAIM_DEPOSIT,
    );
    assert_eq!(
        e.claim_commitment_used(face_b64(DAUGHTER_FACE)),
        Some("daughter".into())
    );
}

#[test]
#[should_panic(expected = "attach more NEAR")]
fn the_first_claim_pays_for_its_record() {
    let (mut e, unlock_at) = funded_estate();
    claim_with(
        &mut e,
        "son",
        &claim_att("son", SON_FACE, 10, unlock_at),
        unlock_at,
        NearToken::from_yoctonear(0),
    );
}

#[test]
fn an_unknown_commitment_has_claimed_nothing() {
    let (e, _) = funded_estate();
    assert_eq!(e.claim_commitment_used(face_b64(SON_FACE)), None);
    assert_eq!(e.claim_commitment_used(Base64VecU8(vec![1, 2, 3])), None);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Random claims: any label, any face (the grantor's included), from the right account or a
    /// sibling's. A claim goes through exactly when the rules allow it; no face ever holds two
    /// labels; the grantor's face is never recorded.
    #[test]
    fn one_human_one_slot_holds_and_each_heir_is_paid_once(
        claims in prop::collection::vec((0..3usize, 0..4usize, prop::bool::weighted(0.85)), 1..25)
    ) {
        let labels = ["son", "daughter", "nobody"];
        let faces = [GRANTOR_FACE, SON_FACE, DAUGHTER_FACE, [5; 32]];
        let (mut e, unlock_at) = funded_estate();
        let mut model: std::collections::HashMap<[u8; 32], &str> = Default::default();
        // Labels already paid in full: claiming again finds nothing to claim.
        let mut paid: std::collections::HashSet<&str> = Default::default();
        for (n, (l, f, own_account)) in claims.into_iter().enumerate() {
            let (label, face) = (labels[l], faces[f]);
            let mut att = claim_att(label, face, 100 + n as u64, unlock_at);
            if !own_account {
                att.subject = heir_id(labels[(l + 1) % 2]);
            }
            let ok = call_reverting(&mut e, |e| claim_with(e, label, &att, unlock_at, CLAIM_DEPOSIT));
            let expected = label != "nobody"
                && att.subject == heir_id(label)
                && face != GRANTOR_FACE
                && model.get(&face).is_none_or(|claimed| *claimed == label)
                && !paid.contains(label);
            prop_assert_eq!(ok, expected, "{} with face {} from {}", label, f, att.subject);
            if ok {
                model.insert(face, label);
                paid.insert(label);
            }
            for face in faces {
                prop_assert_eq!(
                    e.claim_commitment_used(face_b64(face)),
                    model.get(&face).map(|l| l.to_string())
                );
            }
        }
    }
}

// ---------- Part 3: payouts ----------

use near_sdk::mock::MockAction;
use near_sdk::test_utils::get_created_receipts;

/// Every payout the last call scheduled, as (receiver, asset, amount): the receipts an `on_paid`
/// callback waits on. (A refund of the heir's surplus deposit is a transfer too, but nothing
/// waits on it.)
fn payouts() -> Vec<(AccountId, Asset, u128)> {
    let receipts = get_created_receipts();
    let awaited: Vec<u64> = receipts
        .iter()
        .filter(|r| {
            r.actions.iter().any(|a| {
                matches!(a, MockAction::FunctionCallWeight { method_name, .. } if method_name == b"on_paid")
            })
        })
        .flat_map(|r| r.receipt_indices.clone())
        .collect();
    let mut out = vec![];
    for r in &receipts {
        for a in &r.actions {
            if !a.receipt_index().is_some_and(|i| awaited.contains(&i)) {
                continue;
            }
            match a {
                MockAction::Transfer { deposit, .. } => {
                    out.push((r.receiver_id.clone(), NEAR, deposit.as_yoctonear()))
                }
                MockAction::FunctionCallWeight {
                    method_name, args, ..
                } if method_name == b"ft_transfer" => {
                    let args: near_sdk::serde_json::Value =
                        near_sdk::serde_json::from_slice(args).unwrap();
                    let to: AccountId = args["receiver_id"].as_str().unwrap().parse().unwrap();
                    let amount: u128 = args["amount"].as_str().unwrap().parse().unwrap();
                    out.push((to, Some(r.receiver_id.clone()), amount));
                }
                _ => {}
            }
        }
    }
    out
}

fn son_claims(e: &mut Estate, nonce: u64, now: u64) -> u32 {
    set_context_paying(heir_id("son"), now, CLAIM_DEPOSIT);
    let (attestation, signature) = claim_att("son", SON_FACE, nonce, now).sign();
    e.claim("son".into(), attestation, signature)
}

fn daughter_claims(e: &mut Estate, nonce: u64, now: u64) -> u32 {
    set_context_paying(heir_id("daughter"), now, CLAIM_DEPOSIT);
    let (attestation, signature) = claim_att("daughter", DAUGHTER_FACE, nonce, now).sign();
    e.claim("daughter".into(), attestation, signature)
}

#[test]
fn a_claim_pays_the_heirs_share_of_every_asset() {
    let (mut e, unlock_at) = funded_estate();
    let near = e.balance_of(NEAR).0;
    assert_eq!(son_claims(&mut e, 10, unlock_at), 2);

    let son_near = near * 6_000 / 10_000;
    let mut sent = payouts();
    sent.sort_by_key(|p| p.1.is_some());
    assert_eq!(
        sent,
        vec![
            (heir_id("son"), NEAR, son_near),
            (heir_id("son"), Some(usdc()), 3_000_000)
        ]
    );
    // Debited before the transfers run.
    assert_eq!(e.balance_of(NEAR), U128(near - son_near));
    assert_eq!(e.balance_of(Some(usdc())), U128(2_000_000));
    assert!(e.has_claimed("son".into(), NEAR) && e.has_claimed("son".into(), Some(usdc())));
    assert!(
        get_logs().iter().any(
            |l| l.contains(r#""event":"claim_settled","data":[{"label":"son","transfers":2}]"#)
        )
    );
    let callbacks = get_created_receipts()
        .iter()
        .flat_map(|r| r.actions.clone())
        .filter(|a| matches!(a, MockAction::FunctionCallWeight { method_name, .. } if method_name == b"on_paid"))
        .count();
    assert_eq!(callbacks, 2, "one callback per transfer");
}

#[test]
fn heirs_claiming_at_different_times_split_the_same_total() {
    let (mut e, unlock_at) = funded_estate();
    son_claims(&mut e, 10, unlock_at);
    // A week later the balance is lower, but the daughter's 40% is still of the snapshot.
    let later = unlock_at + 7 * 86_400 * NS_PER_SEC;
    daughter_claims(&mut e, 11, later);
    assert_eq!(
        payouts().into_iter().find(|p| p.1 == Some(usdc())),
        Some((heir_id("daughter"), Some(usdc()), 2_000_000))
    );
    assert_eq!(e.balance_of(Some(usdc())), U128(0), "60% + 40% of 5 mUSDC");
}

#[test]
fn shares_round_down_and_the_dust_stays() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 6_000);
    add_created_heir(&mut e, "daughter", 4_000);
    check_in_at(&mut e, CHECKED_IN_AT, 1);
    add_registered_token(&mut e, usdc());
    ft_deposit_at(&mut e, usdc(), grantor(), 3, CHECKED_IN_AT);
    let (_, unlock_at) = e.deadlines_of();

    son_claims(&mut e, 10, unlock_at.0); // 60% of 3 = 1.8 -> 1
    daughter_claims(&mut e, 11, unlock_at.0); // 40% of 3 = 1.2 -> 1
    assert_eq!(
        e.balance_of(Some(usdc())),
        U128(1),
        "1 unit of dust, never an overpayment"
    );
}

#[test]
fn a_share_that_rounds_to_nothing_is_marked_paid_but_not_sent() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 6_000);
    check_in_at(&mut e, CHECKED_IN_AT, 1);
    deposit_at(&mut e, grantor(), ONE_NEAR.as_yoctonear(), CHECKED_IN_AT);
    add_registered_token(&mut e, usdc());
    ft_deposit_at(&mut e, usdc(), grantor(), 1, CHECKED_IN_AT); // 60% of 1 -> 0
    let (_, unlock_at) = e.deadlines_of();

    assert_eq!(
        son_claims(&mut e, 10, unlock_at.0),
        1,
        "only the NEAR is sent"
    );
    assert!(e.has_claimed("son".into(), Some(usdc())));
    assert_eq!(e.balance_of(Some(usdc())), U128(1));
}

#[test]
fn an_asset_with_a_zero_share_is_skipped_and_not_marked() {
    let (mut e, unlock_at) = funded_estate();
    set_context_paying(grantor(), CHECKED_IN_AT, CLAIM_DEPOSIT);
    e.set_share("son".into(), Some(usdc()), 0);
    assert_eq!(son_claims(&mut e, 10, unlock_at), 1);
    assert!(!e.has_claimed("son".into(), Some(usdc())));
    assert_eq!(e.balance_of(Some(usdc())), U128(5_000_000));
}

#[test]
fn overrides_are_paid_as_set() {
    let (mut e, unlock_at) = funded_estate();
    set_context_paying(grantor(), CHECKED_IN_AT, CLAIM_DEPOSIT);
    e.set_share("son".into(), Some(usdc()), 2_000);
    son_claims(&mut e, 10, unlock_at);
    assert!(payouts().contains(&(heir_id("son"), Some(usdc()), 1_000_000)));
}

#[test]
#[should_panic(expected = "nothing to claim")]
fn a_paid_heir_has_nothing_left_to_claim() {
    let (mut e, unlock_at) = funded_estate();
    son_claims(&mut e, 10, unlock_at);
    son_claims(&mut e, 11, unlock_at);
}

#[test]
#[should_panic(expected = "nothing to claim")]
fn an_heir_with_no_share_has_nothing_to_claim() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 0);
    check_in_at(&mut e, CHECKED_IN_AT, 1);
    deposit_at(&mut e, grantor(), ONE_NEAR.as_yoctonear(), CHECKED_IN_AT);
    let (_, unlock_at) = e.deadlines_of();
    son_claims(&mut e, 10, unlock_at.0);
}

#[test]
fn a_failed_payout_is_credited_back_and_can_be_claimed_again() {
    let (mut e, unlock_at) = funded_estate();
    son_claims(&mut e, 10, unlock_at);
    set_callback_context(false);
    assert!(!e.on_paid("son".into(), Some(usdc()), U128(3_000_000)));

    assert_eq!(e.balance_of(Some(usdc())), U128(5_000_000));
    assert!(!e.has_claimed("son".into(), Some(usdc())));
    assert_eq!(e.claimable_of("son".into(), Some(usdc())), U128(3_000_000));
    assert!(
        get_logs()
            .iter()
            .any(|l| l.contains(r#""event":"claim_failed""#))
    );
    // The retry sends only the mUSDC.
    assert_eq!(son_claims(&mut e, 11, unlock_at), 1);
    assert_eq!(payouts(), vec![(heir_id("son"), Some(usdc()), 3_000_000)]);
}

#[test]
fn a_successful_payout_is_logged() {
    let (mut e, unlock_at) = funded_estate();
    son_claims(&mut e, 10, unlock_at);
    set_callback_context(true);
    assert!(e.on_paid("son".into(), Some(usdc()), U128(3_000_000)));
    let expected = format!(
        r#""event":"claimed","data":[{{"label":"son","asset":"{}","amount":"3000000"}}]"#,
        usdc()
    );
    assert!(get_logs().iter().any(|l| l.contains(&expected)));
}

#[test]
fn claimable_shows_what_a_claim_would_pay() {
    let (mut e, unlock_at) = funded_estate();
    set_context(stranger(), CHECKED_IN_AT);
    assert_eq!(
        e.claimable_of("son".into(), Some(usdc())),
        U128(0),
        "nothing before unlock"
    );

    set_context(stranger(), unlock_at);
    assert_eq!(
        e.claimable_of("son".into(), Some(usdc())),
        U128(3_000_000),
        "unlocked, unpoked"
    );
    assert_eq!(e.claimable_of("nobody".into(), Some(usdc())), U128(0));
    let near = e.balance_of(NEAR).0;
    assert_eq!(
        e.claimable_all("daughter".into()),
        vec![
            (NEAR, U128(near * 4_000 / 10_000)),
            (Some(usdc()), U128(2_000_000))
        ]
    );

    son_claims(&mut e, 10, unlock_at);
    assert_eq!(e.claimable_of("son".into(), NEAR), U128(0), "paid");
    assert_eq!(
        e.claimable_of("daughter".into(), Some(usdc())),
        U128(2_000_000),
        "still of the snapshot"
    );
}

#[test]
#[should_panic(expected = "attach more NEAR")]
fn the_heir_pays_for_the_records_and_the_token_transfers() {
    let (mut e, unlock_at) = funded_estate();
    set_context_paying(heir_id("son"), unlock_at, NearToken::from_yoctonear(1));
    let (attestation, signature) = claim_att("son", SON_FACE, 10, unlock_at).sign();
    e.claim("son".into(), attestation, signature);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    /// Random shares, balances and payout failures; every heir claims, retrying after failures,
    /// in random order. In the end every asset's payouts are exactly each heir's rounded-down
    /// share of the snapshot, never more than the snapshot, and the books hold the rest.
    #[test]
    fn every_heir_gets_exactly_their_share_of_the_snapshot(
        defaults in prop::collection::vec(0..=5_000u16, 3),
        usdc_override in prop::option::of(0..=5_000u16),
        near in 10u128.pow(22)..=10u128.pow(25),
        usdc_amount in 1..=10u128.pow(12),
        order in Just(vec![0usize, 1, 2]).prop_shuffle(),
        failures in prop::collection::vec(any::<bool>(), 64),
    ) {
        let labels = ["son", "daughter", "kid"];
        let faces = [SON_FACE, DAUGHTER_FACE, [5; 32]];
        let mut defaults = defaults;
        defaults[2] = defaults[2].min(10_000 - defaults[0] - defaults[1]);
        let mut e = new_estate();
        for (l, d) in labels.iter().zip(&defaults) {
            add_created_heir(&mut e, l, *d);
        }
        if let Some(bps) = usdc_override {
            // Keep usdc within 100%: the son's override replaces his default for usdc only.
            let bps = bps.min(10_000 - defaults[1] - defaults[2]);
            set_context_paying(grantor(), 0, CLAIM_DEPOSIT);
            e.set_share("son".into(), Some(usdc()), bps);
        }
        check_in_at(&mut e, CHECKED_IN_AT, 1);
        deposit_at(&mut e, grantor(), near, CHECKED_IN_AT);
        add_registered_token(&mut e, usdc());
        ft_deposit_at(&mut e, usdc(), grantor(), usdc_amount, CHECKED_IN_AT);
        let (_, unlock_at) = e.deadlines_of();
        let unlock_at = unlock_at.0;
        set_context(stranger(), unlock_at);
        e.poke_expiry();
        let snapshot = [e.snapshot_of(NEAR).0, e.snapshot_of(Some(usdc())).0];
        let expected: Vec<[u128; 2]> = labels.iter().map(|l| {
            [e.claimable_of((*l).into(), NEAR).0, e.claimable_of((*l).into(), Some(usdc())).0]
        }).collect();

        let mut received = [[0u128; 2]; 3];
        let mut failures = failures.into_iter();
        let mut nonce = 100;
        for &h in &order {
            // Claim, settle each transfer (some fail), and retry until nothing is left.
            for _attempt in 0..20 {
                nonce += 1;
                let att = claim_att(labels[h], faces[h], nonce, unlock_at);
                let ok = call_reverting(&mut e, |e| claim_with(e, labels[h], &att, unlock_at, CLAIM_DEPOSIT));
                if !ok {
                    break; // nothing (left) to claim
                }
                for (_, asset, amount) in payouts() {
                    let i = asset.is_some() as usize;
                    let sent = !failures.next().unwrap_or(false);
                    set_callback_context(sent);
                    e.on_paid(labels[h].into(), asset, U128(amount));
                    if sent {
                        received[h][i] += amount;
                    }
                }
            }
        }
        for i in 0..2 {
            let total: u128 = received.iter().map(|r| r[i]).sum();
            prop_assert!(total <= snapshot[i]);
            for h in 0..3 {
                prop_assert_eq!(received[h][i], expected[h][i], "{} asset {}", labels[h], i);
            }
            let asset = if i == 0 { NEAR } else { Some(usdc()) };
            prop_assert_eq!(e.balance_of(asset), U128(snapshot[i] - total));
        }
    }
}

#[test]
fn a_token_payout_registers_the_heir_with_the_token_first() {
    let (mut e, unlock_at) = funded_estate();
    son_claims(&mut e, 10, unlock_at);
    let calls: Vec<(String, near_sdk::serde_json::Value)> = get_created_receipts()
        .into_iter()
        .filter(|r| r.receiver_id == usdc())
        .flat_map(|r| r.actions)
        .filter_map(|a| match a {
            MockAction::FunctionCallWeight {
                method_name, args, ..
            } => Some((
                String::from_utf8(method_name).unwrap(),
                near_sdk::serde_json::from_slice(&args).unwrap(),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(calls[0].0, "storage_deposit");
    assert_eq!(
        calls[0].1["account_id"],
        near_sdk::serde_json::json!(heir_id("son"))
    );
    assert_eq!(calls[1].0, "ft_transfer");
}

// ---------- named heirs claim from their own account ----------

/// `funded_estate`, with a named heir `kay` (`kaynearhackathon2.testnet`) in the daughter's place.
fn estate_with_a_named_heir() -> (Estate, u64) {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 6_000);
    add_named_heir_at(&mut e, "kay", named_id(2), 4_000, 0);
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

fn named_claim_att(nonce: u64, now: u64) -> Att {
    let mut att = claim_att("kay", DAUGHTER_FACE, nonce, now);
    att.subject = named_id(2);
    att
}

#[test]
fn a_named_heir_is_paid_into_their_own_account() {
    let (mut e, unlock_at) = estate_with_a_named_heir();
    let near_share = e.balance_of(NEAR).0 * 4_000 / 10_000;
    claim_with(
        &mut e,
        "kay",
        &named_claim_att(10, unlock_at),
        unlock_at,
        CLAIM_DEPOSIT,
    );

    let mut paid = payouts();
    paid.sort_by_key(|(_, asset, _)| asset.clone());
    assert_eq!(
        paid,
        vec![
            (named_id(2), NEAR, near_share),
            (named_id(2), Some(usdc()), 2_000_000),
        ]
    );
}

#[test]
#[should_panic(expected = "not entitled")]
fn a_named_heirs_slot_cannot_be_claimed_from_the_sub_account() {
    let (mut e, unlock_at) = estate_with_a_named_heir();
    let mut att = named_claim_att(10, unlock_at);
    att.subject = heir_id("kay"); // the account the label would name, had add_heir made it
    claim_with(&mut e, "kay", &att, unlock_at, CLAIM_DEPOSIT);
}

#[test]
#[should_panic(expected = "invalid signature")]
fn an_attestation_for_another_account_does_not_work() {
    let (mut e, unlock_at) = estate_with_a_named_heir();
    let mut att = named_claim_att(10, unlock_at);
    att.subject = named_id(3); // signed for someone else, sent by kay
    set_context_paying(named_id(2), unlock_at, CLAIM_DEPOSIT);
    let (attestation, signature) = att.sign();
    e.claim("kay".into(), attestation, signature);
}
