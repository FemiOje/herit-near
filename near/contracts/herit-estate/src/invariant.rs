//! Invariant tests: random sequences of calls, with the invariants checked after every step.
//! A call that panics is rolled back, as a failed transaction would be on chain.

use proptest::prelude::*;

use super::testing::*;
use super::*;

#[derive(Debug, Clone)]
enum Op {
    /// Move the clock forward by this many seconds.
    Advance(u64),
    Poke,
    Configure {
        by_grantor: bool,
        interval: u32,
        grace: u32,
    },
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0..=120 * 86_400u64).prop_map(Op::Advance),
        2 => Just(Op::Poke),
        2 => (any::<bool>(), 0..=MAX_CHECK_IN_INTERVAL_SEC + 10, 0..=MAX_GRACE_SEC + 10)
            .prop_map(|(by_grantor, interval, grace)| Op::Configure { by_grantor, interval, grace }),
    ]
}

fn rank(s: Status) -> u8 {
    match s {
        Status::Active => 0,
        Status::Grace => 1,
        Status::Unlocked => 2,
    }
}

/// Runs `ops` and checks the invariants after each one. `started` says whether the clock was
/// started (a check-in happened) before the run.
fn run(started: bool, ops: Vec<Op>) -> Result<(), TestCaseError> {
    let mut e = new_estate();
    let mut now = CHECKED_IN_AT;
    if started {
        e.last_check_in_ns = CHECKED_IN_AT;
    }
    let checked_in_at = e.last_check_in_ns;
    set_context(stranger(), now);
    let mut prev = e.status_of();

    for op in ops {
        match op {
            Op::Advance(sec) => {
                now += sec * NS_PER_SEC;
                set_context(stranger(), now);
            }
            Op::Poke => {
                let s = e.poke_expiry(); // permissionless, so the caller doesn't matter
                prop_assert_eq!(s, e.status, "poke must store what it returns");
            }
            Op::Configure {
                by_grantor,
                interval,
                grace,
            } => {
                set_context(if by_grantor { grantor() } else { stranger() }, now);
                let was = e.status_of();
                let ok = call_reverting(&mut e, |e| e.configure(interval, grace));
                if ok {
                    prop_assert!(by_grantor, "only the grantor can configure");
                    prop_assert_eq!(was, Status::Active, "configure only works in Active");
                }
            }
        }

        // The context is already at `now`; status_of doesn't depend on the caller.
        let s = e.status_of();
        prop_assert!(
            rank(s) >= rank(prev),
            "status went backwards: {:?} -> {:?}",
            prev,
            s
        );
        if prev == Status::Unlocked {
            prop_assert_eq!(s, Status::Unlocked, "Unlocked must never leave");
        }
        if !started {
            prop_assert_eq!(s, Status::Active, "no check-in must never lapse");
        }
        prop_assert!(
            rank(e.status) <= rank(s),
            "the cache can't be ahead of the clock"
        );
        prop_assert_eq!(
            e.last_check_in_ns,
            checked_in_at,
            "only a check-in moves the clock start"
        );
        prev = s;
    }
    Ok(())
}

proptest! {
    // Each step rebuilds the mocked chain, so 64 sequences of up to 40 steps keeps this under a minute.
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn invariants_hold_after_a_check_in(ops in proptest::collection::vec(op(), 1..40)) {
        run(true, ops)?;
    }

    #[test]
    fn invariants_hold_with_no_check_in(ops in proptest::collection::vec(op(), 1..40)) {
        run(false, ops)?;
    }
}
