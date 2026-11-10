//! Invariant tests: random sequences of calls, with the invariants checked after every step.
//! A call that panics is rolled back, as a failed transaction would be on chain.

use std::collections::HashSet;

use proptest::prelude::*;

use super::testing::*;
use super::*;

#[derive(Debug, Clone, Copy)]
enum Expiry {
    Valid,
    Expired,
    TooLong,
}

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
    /// Nonces come from a small range so that replays happen often.
    CheckIn {
        by_grantor: bool,
        other_face: bool,
        nonce: u64,
        expiry: Expiry,
    },
}

fn op() -> impl Strategy<Value = Op> {
    let expiry = prop_oneof![
        6 => Just(Expiry::Valid),
        1 => Just(Expiry::Expired),
        1 => Just(Expiry::TooLong),
    ];
    prop_oneof![
        3 => (0..=120 * 86_400u64).prop_map(Op::Advance),
        2 => Just(Op::Poke),
        2 => (any::<bool>(), 0..=MAX_CHECK_IN_INTERVAL_SEC + 10, 0..=MAX_GRACE_SEC + 10)
            .prop_map(|(by_grantor, interval, grace)| Op::Configure { by_grantor, interval, grace }),
        3 => (prop::bool::weighted(0.85), prop::bool::weighted(0.2), 0..12u64, expiry)
            .prop_map(|(by_grantor, other_face, nonce, expiry)| Op::CheckIn { by_grantor, other_face, nonce, expiry }),
    ]
}

fn rank(s: Status) -> u8 {
    match s {
        Status::Active => 0,
        Status::Grace => 1,
        Status::Unlocked => 2,
    }
}

/// What the estate should look like, tracked alongside it.
struct Model {
    bound_face: Option<[u8; 32]>,
    accepted_nonces: HashSet<u64>,
    last_check_in_ns: u64,
}

/// Runs `ops` and checks the invariants after each one. `started` says whether the clock was
/// already running before the run (set directly, with no face bound yet).
fn run(started: bool, ops: Vec<Op>) -> Result<(), TestCaseError> {
    let mut e = new_estate();
    let mut now = CHECKED_IN_AT;
    if started {
        e.last_check_in_ns = CHECKED_IN_AT;
    }
    let mut m = Model {
        bound_face: None,
        accepted_nonces: HashSet::new(),
        last_check_in_ns: e.last_check_in_ns,
    };
    set_context(stranger(), now);
    let mut prev = e.status_of();

    for op in ops {
        let mut checked_in = false;
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
                if call_reverting(&mut e, |e| e.configure(interval, grace)) {
                    prop_assert!(by_grantor, "only the grantor can configure");
                    prop_assert_eq!(was, Status::Active, "configure only works in Active");
                }
            }
            Op::CheckIn {
                by_grantor,
                other_face,
                nonce,
                expiry,
            } => {
                let mut att = Att::check_in(nonce, now);
                if !by_grantor {
                    att.subject = stranger();
                }
                if other_face {
                    att.commitment = OTHER_FACE;
                }
                att.expiry_ns = match expiry {
                    Expiry::Valid => now + LIFETIME_NS,
                    Expiry::Expired => now,
                    Expiry::TooLong => now + MAX_ATTESTATION_LIFETIME_NS + 1,
                };
                set_context(att.subject.clone(), now);
                let was = e.status_of();
                let (attestation, signature) = att.sign();
                if call_reverting(&mut e, |e| e.check_in(attestation, signature)) {
                    prop_assert!(by_grantor, "only the grantor can check in");
                    prop_assert!(matches!(expiry, Expiry::Valid), "bad expiry accepted");
                    prop_assert!(was != Status::Unlocked, "check-in after unlock");
                    prop_assert!(m.accepted_nonces.insert(nonce), "nonce {} replayed", nonce);
                    let face = att.commitment;
                    prop_assert!(
                        m.bound_face.is_none_or(|b| b == face),
                        "wrong human accepted"
                    );
                    m.bound_face = Some(face);
                    m.last_check_in_ns = now;
                    checked_in = true;
                    prop_assert_eq!(e.status, Status::Active);
                }
            }
        }

        // The context is already at `now`; the views don't depend on the caller.
        let s = e.status_of();
        if checked_in {
            prop_assert_eq!(s, Status::Active, "a check-in restores Active");
        } else {
            prop_assert!(
                rank(s) >= rank(prev),
                "status went backwards: {:?} -> {:?}",
                prev,
                s
            );
        }
        if prev == Status::Unlocked {
            prop_assert_eq!(s, Status::Unlocked, "Unlocked must never leave");
        }
        if m.last_check_in_ns == 0 {
            prop_assert_eq!(s, Status::Active, "no check-in must never lapse");
        }
        prop_assert!(
            rank(e.status) <= rank(s),
            "the cache can't be ahead of the clock"
        );
        prop_assert_eq!(
            e.last_check_in_ns,
            m.last_check_in_ns,
            "only a check-in moves the clock"
        );
        prop_assert_eq!(
            e.commitment_of().map(|c| c.0),
            m.bound_face.map(|f| f.to_vec()),
            "the bound face never changes once set"
        );
        for n in 0..12u64 {
            prop_assert_eq!(
                e.nonce_used(U64(n)),
                m.accepted_nonces.contains(&n),
                "nonce {}",
                n
            );
        }
        prev = s;
    }
    Ok(())
}

proptest! {
    // Each step rebuilds the mocked chain, so 64 sequences of up to 40 steps keeps this under a minute.
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn invariants_hold_with_the_clock_already_running(ops in proptest::collection::vec(op(), 1..40)) {
        run(true, ops)?;
    }

    #[test]
    fn invariants_hold_from_a_fresh_estate(ops in proptest::collection::vec(op(), 1..40)) {
        run(false, ops)?;
    }
}
