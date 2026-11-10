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
            Estate::new(grantor(), ATTESTOR_PK.into(), interval, grace)
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
