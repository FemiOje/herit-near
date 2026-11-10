# Checkpoint 3: Estate state machine

Part of the [build plan](../near-build-plan.md). The starter code is in [the deployment guide](../near-deployment-guide.md), §6.1.

**Status:** done, checks passed on 2026-11-10
**Who:** planned as yours; at your request Claude wrote the contract and unit tests as well as the fuzz, invariant and sandbox tests. Read the code before Checkpoint 4

## Goal

Give `herit-estate` its liveness clock: it opens as Active, moves to Grace once the check-in interval runs out, and to Unlocked once grace runs out. It never goes back from Unlocked. Every later checkpoint (attestations, heirs, vault, claims) is gated by this state machine, so it has to be right first.

There's no `check_in` yet; that is Checkpoint 4. Until then, tests start the clock by setting `last_check_in_ns` directly, which a unit test can do because it lives inside the crate.

## Understand first

| Solidity | Here |
|---|---|
| `constructor(...)` | `#[init] pub fn new(...) -> Self`, called once. `#[derive(PanicOnDefault)]` makes every other call fail until it has run |
| `msg.sender` | `env::predecessor_account_id()`: whoever made *this* call, which can be a contract |
| `tx.origin` | `env::signer_account_id()`. Don't use it for authorisation, for the same reason as in Solidity |
| `block.timestamp` (seconds) | `env::block_timestamp()` in **nanoseconds**. The API takes seconds, so convert once in `new`/`configure` and store nanoseconds |
| `view` function | `&self` method: free to call, can't change state |
| `revert Error()` | `require!(cond, "message")`. Tests match on the message with `#[should_panic(expected = "...")]` |
| `emit Event(...)` | `env::log_str("EVENT_JSON:{...}")`, the NEP-297 format, done by `log_event` |

- **Status is computed, not stored.** `pending_status()` works out the status from the clock on every call. The stored `status` is only a cache that `poke_expiry` updates, like `_pendingStatus` on Sepolia. Guards always read `pending_status()`, never `self.status`.
- **No check-in means no clock.** `last_check_in_ns == 0` must always read as Active, or a new estate would lapse before the grantor ever checked in.
- **Unlocked is a one-way door.** Once the cache says Unlocked, `pending_status()` returns Unlocked without looking at the time.
- **`configure` only works in Active, and can't leave Active.** In Grace, a longer interval would recompute back to Active, which is recovery without a Selfie Check. And a shorter interval mustn't move the deadline into the past.
- **Boundaries use `>=`.** The estate is in Grace at exactly `last_check_in + interval`, and Unlocked at exactly that plus `grace`.

## Write (you)

In `near/contracts/herit-estate/src/lib.rs`, take these from guide §6.1, **and only these**:

1. **Constants:** `MIN_/MAX_CHECK_IN_INTERVAL_SEC`, `MIN_/MAX_GRACE_SEC`.
2. **`Status`** enum.
3. **`Estate` state** with these fields only: `factory`, `grantor`, `attestor_pk`, `grantor_commitment`, `check_in_interval_ns`, `grace_ns`, `last_check_in_ns`, `status`. Each later checkpoint adds its own fields. Changing the layout is fine, because nothing is deployed yet.
4. **Methods:** `new`, `poke_expiry`, `configure`, and the views `status_of` and `deadlines_of`.
5. **Internals:** `pending_status`, `only_grantor`, `validate_timers`, `decode_ed25519`, `log_event`.
6. **Keep `version()`.** Once `Estate` uses `PanicOnDefault`, even this view fails until `new` has run, so `sandbox_smoke` breaks for the estate. Claude updates it to call `new` first, as part of the sandbox test.

**Leave out of `poke_expiry` the escrow snapshot on Unlocked.** It needs the escrow and heirs, which arrive in Checkpoints 6–8. For now, the Unlocked branch only logs `unlocked`.

**Bring in nothing else yet.** Clippy runs with `-D warnings`, so an unused constant, import or helper fails the build. This includes `ACTION_*`, `SignedPayload` and the `store` collections.

**Delete the placeholder tests** in the `mod tests` block, because `Estate::default()` no longer exists. Then remove the `mod fuzz;` line and delete `src/fuzz.rs`, which Claude replaces.

## Tests

| Layer | Written by | What it covers |
|---|---|---|
| Unit | You | The list below |
| Fuzz | Claude | `new` accepts timers exactly inside the bounds; `pending_status` agrees with the thresholds for any check-in time and clock |
| Invariant | Claude | Random sequences of "advance time", `poke_expiry` and `configure`. After each step: Unlocked never leaves, status never moves backwards without a check-in, no check-in never lapses, `configure` never changes `last_check_in_ns` |
| Sandbox | Claude | Deploy and `new` on a local node, read `status_of` and `deadlines_of`, a second `new` is refused, a non-grantor `configure` is refused |

**Your unit tests**, in `mod tests` at the bottom of `lib.rs`. Build each call's context with `VMContextBuilder` and `testing_env!`, setting `predecessor_account_id` (the caller) and `block_timestamp` (the clock):

1. `new` stores the grantor and timers, sets `factory` to the caller, starts Active, and `deadlines_of` is `(0, 0)`.
2. `new` refuses each timer just outside its bounds (`"bad interval"`, `"bad grace"`) and a malformed attestor key.
3. With no check-in, the estate stays Active even 10 years later.
4. After a check-in, the status is Active one nanosecond before the interval ends, Grace exactly at it, and Unlocked exactly at interval + grace.
5. `status_of` changes with the clock without a poke; `poke_expiry` updates the cache and calling it twice changes nothing.
6. Unlocked is irreversible: poke it to Unlocked, then set the clock back, and it still reads Unlocked.
7. `configure` is refused for a non-grantor, in Grace and in Unlocked.
8. `configure` is refused when a shorter interval would put the estate past its deadline (`"timers would end Active"`).
9. A valid `configure` changes `deadlines_of` but not the last check-in time.

A test helper that builds the context from `(caller, now_ns)` keeps these short.

## Check

Run from `near/`:

```bash
(cd contracts/herit-estate && cargo near build non-reproducible-wasm)
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

Or ask Claude to run them, as in Checkpoint 2.

**Where the tests live:** unit tests in `src/lib.rs`, shared helpers in `src/testing.rs`, fuzz in `src/fuzz.rs`, invariants in `src/invariant.rs`, sandbox in `near/tests/estate_state_machine.rs`. `cargo test` takes about 3 minutes; most of it is the sandbox test that fast-forwards 500 blocks.

## Verified at the stop

- [x] An estate with no check-in never lapses
- [x] Grace and Unlocked start exactly at their deadlines
- [x] `configure` is refused outside Active and can't leave Active
- [x] Unlocked is irreversible
- [x] Unit tests (the 9 cases, as 15 tests), fuzz, invariant and sandbox tests pass
- [x] Clippy shows no warnings and `cargo fmt` is clean
