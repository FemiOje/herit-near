//! Fuzz tests: one method at a time, random inputs.

use proptest::prelude::*;

use super::testing::*;
use super::*;

fn in_bounds(interval: u32, grace: u32) -> bool {
    (MIN_CHECK_IN_INTERVAL_SEC..=MAX_CHECK_IN_INTERVAL_SEC).contains(&interval)
        && (MIN_GRACE_SEC..=MAX_GRACE_SEC).contains(&grace)
}

/// Mostly values near the bounds, where off-by-one bugs live, plus anything at all.
fn timer(min: u32, max: u32) -> impl Strategy<Value = u32> {
    prop_oneof![
        (min.saturating_sub(2)..=min.saturating_add(2)),
        (max.saturating_sub(2)..=max.saturating_add(2)),
        min..=max,
        any::<u32>(),
    ]
}

proptest! {
    #[test]
    fn new_accepts_timers_exactly_inside_the_bounds(
        interval in timer(MIN_CHECK_IN_INTERVAL_SEC, MAX_CHECK_IN_INTERVAL_SEC),
        grace in timer(MIN_GRACE_SEC, MAX_GRACE_SEC),
    ) {
        set_context(factory(), 0);
        let opened = std::panic::catch_unwind(|| {
            Estate::new(grantor(), attestor_pk(), interval, grace)
        })
        .is_ok();
        prop_assert_eq!(opened, in_bounds(interval, grace));
    }

    #[test]
    fn configure_accepts_timers_exactly_inside_the_bounds(
        interval in timer(MIN_CHECK_IN_INTERVAL_SEC, MAX_CHECK_IN_INTERVAL_SEC),
        grace in timer(MIN_GRACE_SEC, MAX_GRACE_SEC),
    ) {
        let mut e = new_estate(); // clock not started, so no deadline can be crossed
        set_context(grantor(), 0);
        let ok = call_reverting(&mut e, |e| e.configure(interval, grace));
        prop_assert_eq!(ok, in_bounds(interval, grace));
    }

    #[test]
    fn status_matches_the_thresholds(
        interval in MIN_CHECK_IN_INTERVAL_SEC..=MAX_CHECK_IN_INTERVAL_SEC,
        grace in MIN_GRACE_SEC..=MAX_GRACE_SEC,
        checked_in_at in 1..=u64::MAX / 4,
        elapsed in prop_oneof![0..=u64::MAX / 4, 0..=200 * 86_400 * NS_PER_SEC],
    ) {
        let mut e = new_estate_with(interval, grace);
        e.last_check_in_ns = checked_in_at;
        let now = checked_in_at + elapsed;
        set_context(stranger(), now);

        let interval_ns = interval as u64 * NS_PER_SEC;
        let expected = if elapsed >= interval_ns + grace as u64 * NS_PER_SEC {
            Status::Unlocked
        } else if elapsed >= interval_ns {
            Status::Grace
        } else {
            Status::Active
        };
        prop_assert_eq!(e.status_of(), expected);
    }
}

// ---------- Checkpoint 4: attestations ----------

/// Changes exactly one thing about a valid check-in: what was signed, or what is submitted.
fn tamper(signed: &mut Att, submitted: &mut Att, which: u8, salt: u64) {
    match which {
        0 => signed.estate = other_estate_id(),
        1 => signed.subject = stranger(),
        2 => submitted.action = 1 + (salt % 255) as u8,
        3 => submitted.heir_label = Some(format!("h{salt}")),
        4 => submitted.commitment[(salt % 32) as usize] ^= 1 + (salt % 255) as u8,
        5 => submitted.nonce = submitted.nonce.wrapping_add(1 + salt % 1_000),
        6 => submitted.expiry_ns -= 1 + salt % (10 * 60 * NS_PER_SEC), // still inside the window
        _ => *signed = signed.clone().signed_by_another_key(),
    }
}

proptest! {
    #[test]
    fn any_valid_attestation_is_accepted(
        nonce in any::<u64>(),
        lifetime in 1..=MAX_ATTESTATION_LIFETIME_NS,
        face in any::<[u8; 32]>().prop_filter("non-zero", |f| *f != [0; 32]),
        now in CHECKED_IN_AT..=CHECKED_IN_AT * 2,
    ) {
        let mut e = new_estate();
        let mut att = Att::check_in(nonce, now);
        att.expiry_ns = now + lifetime;
        att.commitment = face;
        set_context(grantor(), now);
        let (attestation, signature) = att.sign();
        e.check_in(attestation, signature);
        prop_assert_eq!(e.commitment_of().map(|c| c.0), Some(face.to_vec()));
        prop_assert!(e.nonce_used(U64(nonce)));
    }

    #[test]
    fn changing_any_signed_field_is_refused(
        which in 0u8..8,
        salt in any::<u64>(),
        nonce in any::<u64>(),
    ) {
        let mut e = new_estate();
        let mut signed = Att::check_in(nonce, CHECKED_IN_AT);
        let mut submitted = signed.clone();
        tamper(&mut signed, &mut submitted, which, salt);

        let (_, signature) = signed.sign();
        let attestation = submitted.attestation();
        set_context(grantor(), CHECKED_IN_AT);
        let ok = call_reverting(&mut e, |e| e.check_in(attestation, signature));

        prop_assert!(!ok, "tampering with field {} was accepted", which);
        prop_assert!(e.commitment_of().is_none());
        prop_assert!(!e.nonce_used(U64(signed.nonce)));
        prop_assert!(!e.nonce_used(U64(submitted.nonce)));
    }

    #[test]
    fn random_signatures_never_verify(
        lo in any::<[u8; 32]>(),
        hi in any::<[u8; 32]>(),
        nonce in any::<u64>(),
    ) {
        let mut e = new_estate();
        let attestation = Att::check_in(nonce, CHECKED_IN_AT).attestation();
        let signature = Base64VecU8([lo, hi].concat());
        set_context(grantor(), CHECKED_IN_AT);
        let ok = call_reverting(&mut e, |e| e.check_in(attestation, signature));
        prop_assert!(!ok);
        prop_assert!(!e.nonce_used(U64(nonce)));
    }
}
