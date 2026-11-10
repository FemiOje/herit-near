# Checkpoint 4: Estate attestations and check-in

Part of the [build plan](../near-build-plan.md). The starter code is in [the deployment guide](../near-deployment-guide.md), §6.1 (`SignedPayload`, `Attestation`, `check_in`, `verify`), and the matching JS encoder is in §7.5.

**Status:** done, checks passed on 2026-11-10
**Who:** planned as yours; at your request Claude wrote all of it, contract and every test layer. Read the code before Checkpoint 5

## Goal

Let the grantor prove they're alive. `check_in` takes an attestation signed by the World ID attestor, checks it, and restarts the clock from Checkpoint 3. The first check-in binds the grantor's face commitment, and every later check-in must come from the same face. Claims and withdrawals in later checkpoints reuse the same `verify`.

## Understand first

| Solidity (Sepolia Herit) | Here |
|---|---|
| EIP-712 typed data | A **Borsh**-encoded `SignedPayload`. Borsh is positional: fields are written in declaration order with no names, so the signer and the contract must declare the same fields in the same order with the same types |
| `ecrecover(hash, v, r, s) == attestor` | `env::ed25519_verify(&sig, &msg, &attestor_pk)`. ed25519 can't recover a key from a signature, so the contract checks against the one stored key |
| Domain separator with `chainId` and `verifyingContract` | `domain` plus `estate_id` from `env::current_account_id()`, and `subject` from `env::predecessor_account_id()` |
| `mapping(uint256 => bool) usedNonces` | `LookupSet<u64>`, a `near_sdk::store` collection |

- **The estate and the caller come from the runtime, not the arguments.** The contract rebuilds the payload with its own account id and the caller's id before checking the signature. A signature made for another estate or another caller then fails, with no extra checks.
- **Collections live in storage under a prefix.** Each `store` collection takes a key from a `#[derive(BorshStorageKey)]` enum, and two collections must never share one. Add a `Key` enum with just `Nonces` for now; later checkpoints append variants **at the end**, because the variant's position is its prefix.
- **Storage costs NEAR.** Every used nonce adds a few dozen bytes, paid from the estate's balance. That's fine for the demo; a long-lived estate would need pruning, which is noted for after the hackathon.
- **A failed transaction reverts everything**, including the nonce being marked used. In a unit test nothing reverts, so a test that expects a panic must not reuse the estate afterwards.
- **Check-in from Grace is allowed and is the recovery path.** A real Selfie Check is exactly the proof Grace waits for. Once the clock says Unlocked, even unpoked, check-in is refused.

## Prepare (Claude, before you start)

1. Add `ed25519-dalek` to `herit-estate`'s dev-dependencies, and to the tests package for the sandbox tests.
2. In `src/testing.rs`, replace the all-zero `ATTESTOR_PK` with a fixed test key pair, and add a helper that signs a check-in attestation for a given estate, caller, face, nonce and expiry. Your tests call it instead of building signatures by hand.

## Write (you)

In `near/contracts/herit-estate/src/lib.rs`, take these from guide §6.1:

1. **Constants:** `ACTION_CHECKIN`, `DOMAIN`, `MAX_ATTESTATION_LIFETIME_NS`. Leave out `ACTION_CLAIM` and `ACTION_WITHDRAW` until their checkpoints, or clippy fails on them as unused.
2. **Types:** `SignedPayload` and `Attestation`, with the fields **in exactly the guide's order**.
3. **State:** a `Key` enum with `Nonces`, and the `used_nonces: LookupSet<u64>` field, initialised in `new`.
4. **Methods:** `check_in`, and the views `nonce_used` and `commitment_of`.
5. **Internal:** `verify`, as in the guide.
6. **Add a `not_unlocked` guard** and use it in `check_in`. Later checkpoints reuse it.

## Tests

| Layer | Written by | What it covers |
|---|---|---|
| Unit | You | The list below |
| Golden | Claude | The Borsh bytes of one fixed `SignedPayload`, pinned as hex. The JS dev signer (§7.5) asserts the same hex when it's written, so the two encoders can't drift apart |
| Fuzz | Claude | Changing any single field of a signed payload (estate, caller, action, label, commitment, nonce, expiry) makes it fail; random signatures never verify |
| Invariant | Claude | Checkpoint 3's random sequences plus check-ins with the right face, a wrong face, a replayed nonce or a bad expiry. After every step: the bound commitment never changes, a used nonce is never accepted again, status only moves back to Active through a successful check-in, and Unlocked never leaves |
| Sandbox | Claude | Real transactions: a check-in starts the clock, a replay is refused, and after `fast_forward` past the deadlines a check-in is refused |

**Your unit tests**, in `mod tests` in `lib.rs`:

1. A valid first check-in binds the commitment, starts the clock (`deadlines_of` is no longer `(0, 0)`) and marks the nonce used.
2. A second check-in from the same face moves the deadlines forward.
3. A check-in from a different face is refused (`"wrong human"`).
4. Reusing a nonce is refused (`"nonce used"`).
5. An attestation that has expired is refused (`"attestation expired"`). One that expires more than 30 minutes ahead is refused (`"attestation lives too long"`); exactly 30 minutes is accepted.
6. The wrong action is refused (`"wrong action"`).
7. A signature made for another estate, for another caller, or by another key is refused (`"invalid signature"`).
8. A non-grantor is refused (`"not the grantor"`).
9. A check-in in Grace brings the estate back to Active.
10. A check-in after the clock says Unlocked, with nobody having poked, is refused (`"estate unlocked"`).
11. An all-zero commitment and a signature that isn't 64 bytes are refused.

## Check

Run from `near/`:

```bash
(cd contracts/herit-estate && cargo near build non-reproducible-wasm)
ls -l target/near/herit_estate/herit_estate.wasm
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

Note the wasm size each time: it was 103 KB after Checkpoint 3 and 132 KB after this one, and the Global Contract burns 10 NEAR per 100 KB.

**Where the tests live:** unit tests in `mod check_in_tests` in `src/lib.rs`, signing helpers in `src/testing.rs`, the golden test in `src/golden.rs`, fuzz in `src/fuzz.rs`, invariants in `src/invariant.rs`, sandbox in `near/tests/estate_check_in.rs`.

**What the invariant tests caught (in the test harness, not the contract):** mocked storage outlives each test estate, and `LookupSet::insert` writes to storage straight away. So nonces leaked from one random sequence into the next, and a refused check-in left its nonce marked used. `new_estate` now starts from empty storage, and `call_reverting` snapshots and restores storage the way a failed transaction does.

## Verified at the stop

- [x] A signature only verifies for this estate and this caller
- [x] The first check-in binds the face, and a different face is refused
- [x] A nonce can't be used twice, and attestations over 30 minutes are refused
- [x] Check-in works in Grace and is refused once Unlocked, poked or not
- [x] The golden test pins the payload bytes
- [x] Unit, fuzz, invariant and sandbox tests pass
- [x] Clippy shows no warnings and `cargo fmt` is clean
