//! Checkpoint 6 fuzz and invariant tests for heirs. Random sequences of add, creation callback,
//! remove, check-in and time passing, with the heir list compared against a model after each step.
//! A call that panics is rolled back, as a failed transaction would be on chain.

use std::collections::BTreeMap;

use proptest::prelude::*;

use super::testing::*;
use super::*;

/// A small pool, so labels collide often. "Bad" and "a.b" are always refused.
const LABELS: [&str; 14] = [
    "son", "daughter", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "Bad", "a.b",
];

#[derive(Debug, Clone)]
enum Op {
    Add {
        by_grantor: bool,
        label: usize,
        bps: u16,
    },
    /// The chain runs the creation callback once per add; the model only runs it for a pending heir.
    Created {
        label: usize,
        ok: bool,
    },
    Remove {
        by_grantor: bool,
        label: usize,
    },
    CheckIn,
    Advance(u64),
}

fn op() -> impl Strategy<Value = Op> {
    let label = 0..LABELS.len();
    prop_oneof![
        5 => (prop::bool::weighted(0.9), label.clone(), 0..=4_000u16)
            .prop_map(|(by_grantor, label, bps)| Op::Add { by_grantor, label, bps }),
        4 => (label.clone(), prop::bool::weighted(0.8)).prop_map(|(label, ok)| Op::Created { label, ok }),
        2 => (prop::bool::weighted(0.9), label).prop_map(|(by_grantor, label)| Op::Remove { by_grantor, label }),
        1 => Just(Op::CheckIn),
        1 => (0..=40 * 86_400u64).prop_map(Op::Advance),
    ]
}

/// label -> (default_bps, created)
type Model = BTreeMap<String, (u16, bool)>;

fn valid_label(l: &str) -> bool {
    !l.is_empty()
        && l.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

fn check(e: &Estate, m: &Model) -> Result<(), TestCaseError> {
    let mut seen = BTreeMap::new();
    for h in e.heirs_of() {
        prop_assert_eq!(
            &h.account_id,
            &heir_id(&h.label),
            "account is <label>.<estate>"
        );
        seen.insert(h.label, (h.default_bps, h.created));
    }
    prop_assert_eq!(&seen, m);
    let total: u32 = m.values().map(|(bps, _)| *bps as u32).sum();
    prop_assert!(total <= BPS, "shares {} exceed 100%", total);
    prop_assert!(m.len() as u32 <= MAX_HEIRS);
    Ok(())
}

fn run(ops: Vec<Op>) -> Result<(), TestCaseError> {
    let mut e = new_estate();
    let mut m = Model::new();
    let mut now = CHECKED_IN_AT;
    let mut nonce = 0;
    for op in ops {
        set_context(stranger(), now);
        let unlocked = e.status_of() == Status::Unlocked;
        match op {
            Op::Add {
                by_grantor,
                label,
                bps,
            } => {
                let label = LABELS[label];
                let caller = if by_grantor { grantor() } else { stranger() };
                let ok = call_reverting(&mut e, |e| {
                    set_context_paying(caller, now, HEIR_DEPOSIT);
                    let _ = e.add_heir(label.into(), heir_key(1), "kin".into(), bps);
                });
                let total: u32 = m.values().map(|(b, _)| *b as u32).sum();
                let expected = by_grantor
                    && !unlocked
                    && valid_label(label)
                    && !m.contains_key(label)
                    && (m.len() as u32) < MAX_HEIRS
                    && total + bps as u32 <= BPS;
                prop_assert_eq!(ok, expected, "add {} {}", label, bps);
                if ok {
                    m.insert(label.into(), (bps, false));
                }
            }
            Op::Created { label, ok } => {
                let label = LABELS[label];
                if m.get(label).is_some_and(|(_, created)| !created) {
                    set_callback_context(ok);
                    prop_assert_eq!(
                        e.on_heir_created(label.into(), near_sdk::json_types::U128(0)),
                        ok
                    );
                    if ok {
                        m.get_mut(label).unwrap().1 = true;
                    } else {
                        m.remove(label);
                    }
                }
            }
            Op::Remove { by_grantor, label } => {
                let label = LABELS[label];
                let caller = if by_grantor { grantor() } else { stranger() };
                let ok = call_reverting(&mut e, |e| {
                    set_context(caller, now);
                    e.remove_heir(label.into());
                });
                let expected = by_grantor && !unlocked && m.get(label).is_some_and(|(_, c)| *c);
                prop_assert_eq!(ok, expected, "remove {}", label);
                if ok {
                    m.remove(label);
                }
            }
            Op::CheckIn => {
                nonce += 1;
                let ok = call_reverting(&mut e, |e| check_in_at(e, now, nonce));
                prop_assert_eq!(ok, !unlocked);
            }
            Op::Advance(secs) => now += secs * NS_PER_SEC,
        }
        check(&e, &m)?;
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn heir_invariants_hold(ops in prop::collection::vec(op(), 1..60)) {
        run(ops)?;
    }

    /// Any string: accepted as a label exactly when it is one lowercase segment that makes a valid
    /// account id under the estate.
    #[test]
    fn labels_are_accepted_exactly_when_valid(label in "[a-zA-Z0-9._ !-]{0,70}") {
        let mut e = new_estate();
        let ok = call_reverting(&mut e, |e| add_heir_at(e, &label, 0, 0));
        let account_ok = format!("{label}.{}", estate_id()).parse::<AccountId>().is_ok();
        prop_assert_eq!(ok, valid_label(&label) && account_ok, "{:?}", label);
    }
}
