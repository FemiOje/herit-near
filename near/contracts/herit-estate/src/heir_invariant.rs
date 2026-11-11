//! Checkpoint 6 fuzz and invariant tests for heirs. Random sequences of add (as a sub-account or
//! a named account), creation callback, remove, check-in and time passing, with the heir list compared against a model after each step.
//! A call that panics is rolled back, as a failed transaction would be on chain.

use std::collections::BTreeMap;

use proptest::prelude::*;

use super::testing::*;
use super::*;

/// A small pool, so labels collide often. "Bad" and "a.b" are always refused.
const LABELS: [&str; 14] = [
    "son", "daughter", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "Bad", "a.b",
];

/// Accounts a grantor might name. The first four are fine; the rest are always refused.
fn accounts() -> [AccountId; 7] {
    [
        named_id(1),
        named_id(2),
        named_id(3),
        "son.bob.herit-near.testnet".parse().unwrap(),
        grantor(),
        estate_id(),
        heir_id("son"),
    ]
}
const NAMEABLE: usize = 4;

#[derive(Debug, Clone)]
enum Op {
    /// `add_heir_account`: named heirs are confirmed at once.
    Name {
        by_grantor: bool,
        label: usize,
        account: usize,
        bps: u16,
    },
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
        3 => (prop::bool::weighted(0.9), label.clone(), 0..7usize, 0..=4_000u16)
            .prop_map(|(by_grantor, label, account, bps)| Op::Name { by_grantor, label, account, bps }),
        5 => (prop::bool::weighted(0.9), label.clone(), 0..=4_000u16)
            .prop_map(|(by_grantor, label, bps)| Op::Add { by_grantor, label, bps }),
        4 => (label.clone(), prop::bool::weighted(0.8)).prop_map(|(label, ok)| Op::Created { label, ok }),
        2 => (prop::bool::weighted(0.9), label).prop_map(|(by_grantor, label)| Op::Remove { by_grantor, label }),
        1 => Just(Op::CheckIn),
        1 => (0..=40 * 86_400u64).prop_map(Op::Advance),
    ]
}

/// label -> (default_bps, created, account)
type Model = BTreeMap<String, (u16, bool, AccountId)>;

fn valid_label(l: &str) -> bool {
    !l.is_empty()
        && l.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

fn check(e: &Estate, m: &Model) -> Result<(), TestCaseError> {
    let mut seen = BTreeMap::new();
    for h in e.heirs_of() {
        seen.insert(h.label, (h.default_bps, h.created, h.account_id));
    }
    prop_assert_eq!(&seen, m);
    let mut owners: Vec<&AccountId> = m.values().map(|(_, _, a)| a).collect();
    owners.sort();
    owners.dedup();
    prop_assert_eq!(owners.len(), m.len(), "one account, one heir slot");
    prop_assert!(
        !owners.contains(&&grantor()),
        "the grantor is never an heir"
    );
    let total: u32 = m.values().map(|(bps, _, _)| *bps as u32).sum();
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
            Op::Name {
                by_grantor,
                label,
                account,
                bps,
            } => {
                let label = LABELS[label];
                let named = accounts()[account].clone();
                let caller = if by_grantor { grantor() } else { stranger() };
                let ok = call_reverting(&mut e, |e| {
                    set_context_paying(caller, now, NAMED_HEIR_DEPOSIT);
                    e.add_heir_account(label.into(), named.clone(), "kin".into(), bps);
                });
                let total: u32 = m.values().map(|(b, _, _)| *b as u32).sum();
                let expected = by_grantor
                    && !unlocked
                    && valid_label(label)
                    && !m.contains_key(label)
                    && (m.len() as u32) < MAX_HEIRS
                    && total + bps as u32 <= BPS
                    && account < NAMEABLE
                    && !m.values().any(|(_, _, a)| a == &named);
                prop_assert_eq!(ok, expected, "name {} {} {}", label, named, bps);
                if ok {
                    m.insert(label.into(), (bps, true, named));
                }
            }
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
                let total: u32 = m.values().map(|(b, _, _)| *b as u32).sum();
                let expected = by_grantor
                    && !unlocked
                    && valid_label(label)
                    && !m.contains_key(label)
                    && (m.len() as u32) < MAX_HEIRS
                    && total + bps as u32 <= BPS;
                prop_assert_eq!(ok, expected, "add {} {}", label, bps);
                if ok {
                    m.insert(label.into(), (bps, false, heir_id(label)));
                }
            }
            Op::Created { label, ok } => {
                let label = LABELS[label];
                if m.get(label).is_some_and(|(_, created, _)| !created) {
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
                let expected = by_grantor && !unlocked && m.get(label).is_some_and(|(_, c, _)| *c);
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
