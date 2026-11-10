# Checkpoint 8: Claims

Part of the [build plan](../near-build-plan.md). Starter code: [the deployment guide](../near-deployment-guide.md) §6.1 (`poke_expiry`'s snapshot and `claim`) and the §6.5 checklist.

**Status:** done on 2026-11-10; all four parts reviewed
**Who:** built together, in parts. Claude writes each part's code and tests and walks you through them; you review, and the next part starts after your OK.

## Goal

Once an estate unlocks, each heir collects their share of every asset, from their own account, with their own Selfie Check. On the shape of Sepolia's `ClaimManager` and `HeritVault.snapshot`/`payOut`.

## Parts

| Part | What | Status |
|---|---|---|
| 1 | **Snapshot at unlock:** `poke_expiry` freezes every balance; `snapshot_of`, `snapshot_taken` | Done, reviewed |
| 2 | **Who may claim:** from the heir's own account, with their own `ACTION_CLAIM` Selfie Check naming the label; not the grantor's face; one human, one heir slot; pokes the estate itself | Done, reviewed |
| 3 | **Payouts:** each asset once per heir, share × snapshot rounded down; debit first, credit back on a failed transfer; `claimable_of`, `claimable_all`, `has_claimed` | Done, reviewed |
| 4 | **Lifecycle sandbox test:** a whole estate, from opening to both heirs paid, on a real node | Done, reviewed |

## Part 1: The snapshot at unlock

| Sepolia | Estate (NEAR) |
|---|---|
| `HeritVault.s_snapshots`, `s_snapshotTaken` | `snapshots: LookupMap<Asset, u128>`, `snapshot_taken: bool` |
| `HeritRegistry._unlock` calls `I_VAULT.snapshot` on the Unlocked transition of `pokeExpiry` | `poke_expiry` calls `snapshot()` on the Unlocked transition |
| `snapshot` is a no-op the second time: two callers can poke in the same block, and re-running after a payout "would shrink the ruler" | The same |
| `Snapshotted(estateId, tokenCount)`, `Unlocked(estateId, at, heirCount)` | `snapshotted {"assets"}`, `unlocked {"at","heirs"}` |
| `snapshotOf`, `snapshotTaken` | `snapshot_of(asset)`, `snapshot_taken()` |

### Understand first

- **Why a snapshot.** Shares are percentages, and the balance shrinks as heirs are paid. Measuring every share against the balance at unlock means heirs claiming on different days split the same total, whoever goes first.
- **When.** On the transition to Unlocked, which happens on the first `poke_expiry` after the deadline. Until someone pokes, `status_of` already says Unlocked but there is no snapshot; Part 2's `claim` pokes first, as `ClaimManager` relied on `pokeExpiry`.
- **NEAR detail 1: storage.** `poke_expiry` is permissionless, so whoever pokes pays only gas, not storage. Writing up to 10 new snapshot entries at unlock would therefore stake storage out of the estate's own balance, which is the heirs' NEAR. So each asset's snapshot entry is created at 0 when the asset is listed, paid for with the listing (the first NEAR deposit, or `add_token`). Taking the snapshot only overwrites those entries: no new storage.
- **NEAR detail 2: a withdrawal in flight at unlock.** A withdrawal sent in Grace is debited at once, but its transfer runs later. If the estate unlocks in between and the transfer then fails, the amount comes back after the snapshot was taken. `on_withdrawn` then adds it to the snapshot too, so the heirs still to claim share it instead of it being stranded.

### Built

| Item | Where |
|---|---|
| Storage key `Snapshots` (appended); `snapshots` and `snapshot_taken` in `Estate` | `src/lib.rs` |
| `snapshot()`: freezes every listed balance once, logs `snapshotted` | `src/lib.rs` |
| `poke_expiry` snapshots on Unlocked; `unlocked` now carries the heir count | `src/lib.rs` |
| `list_asset` creates the snapshot entry; `unlist_asset` removes it | `src/lib.rs` |
| `on_withdrawn` adds a post-unlock failed withdrawal to the snapshot | `src/lib.rs` |
| `snapshot_of(asset) -> U128`, `snapshot_taken() -> bool` | `src/lib.rs` |
| Vault test helpers moved to `src/testing.rs` so claims can use them | `src/testing.rs` |

### Tests

| Kind | Where | What |
|---|---|---|
| Unit (7) | `src/claim_tests.rs` | No snapshot before unlock, nor in Grace; unlocking freezes NEAR and mUSDC and logs both events; an unpoked unlock has none yet; taken once, even after a payout and a second poke; taking it writes no new storage; a withdrawal that fails after unlock joins the snapshot, and one that fails before unlock doesn't create one |
| Fuzz | `src/claim_tests.rs` | Any mix of deposits across NEAR and two tokens: the snapshot equals every balance exactly |
| Sandbox (1) | `near/tests/estate_claims.rs` | On a real node a stranger pokes the lapsed estate: the snapshot matches the balances, storage doesn't grow, and the estate's free NEAR still covers the heirs' |

Shared sandbox helpers (setup, signing for any subject, label and face, deposits, mock USDC, waiting for unlock) are in `near/tests/common/mod.rs`.

Wasm: 236 KB.

## Part 2: Who may claim

| Sepolia | Estate (NEAR) |
|---|---|
| `ClaimManager.claim` pokes, then requires `Unlocked` (`EstateNotUnlocked`) | `claim` pokes, then requires Unlocked ("estate not unlocked") |
| `HeirNotFound` | "no such heir" |
| `canClaim(estateId, label, heir)`: the ENS role the gate granted (`NotEntitled`) | The caller must be the heir's own account, `<label>.<estate>` ("not entitled") |
| `LivenessAttestor._verify` with `ACTION_CLAIM`; subject = `msg.sender` | `verify(.., ACTION_CLAIM)`: subject is the caller, and the attestation's label must be this label |
| `WrongHuman`: the grantor's commitment can't claim | "wrong human: the grantor cannot claim" |
| `s_claimUsed[commitment]`: one claim per commitment (`CommitmentUsed`) | `claim_commitments: commitment → label`: one human, one heir slot, but the same human may claim **the same** label again |
| `claimCommitmentUsed` | `claim_commitment_used(commitment) -> Option<label>` |

### Understand first

- **Who the heir is: their account.** On Sepolia the gate granted the heir an ENS role at unlock, and `canClaim` checked it. On NEAR the heir *is* `son.alice.herit-near.testnet`, an account only the heir holds a key to (Checkpoint 6), so the check is simply that the claim comes from that account.
- **Which human: their face.** The attestation proves a live human did a Selfie Check for this estate, this label and this caller. The grantor's face can never claim, so a grantor can't fake their own death and collect.
- **One human, one heir slot.** Each face commitment is recorded against the label it claimed for. A second heir slot for the same face is refused, which stops one person collecting as both "son" and "daughter".
- **Why a retry is allowed.** On Sepolia the payout was in the same transaction, so a claim either paid or reverted. On NEAR each transfer is a separate receipt that can fail after `claim` returns (Part 3 credits it back). The heir must then be able to claim again, so the same face may claim the same label as many times as needed; each asset is still paid only once (Part 3).
- **`claim` pokes first.** The first claim after the deadline unlocks the estate and takes the snapshot (Part 1). If the claim is then refused, the whole call reverts, poke included.
- **Storage: the heir pays.** The first claim writes the commitment record (about 100 bytes, 0.001 NEAR). `claim` is `#[payable]` and charges the heir for it out of their account's 0.1 NEAR seed, refunding the surplus, so it never comes out of the estate. A retry writes nothing and needs nothing attached.
- **Gas.** Gas costs 0.0001 NEAR per Tgas burnt, but NEAR reserves *attached* gas up front at a pessimistic price, about 0.1 NEAR per 100 Tgas, on every network. So a signer must hold that much, and the heir's 0.1 NEAR seed covers a claim attaching up to about 80 Tgas. The tests attach 50. (First written here as a sandbox quirk; the testnet smoke run in Checkpoint 10 showed it's NEAR's rule.)

### Built

| Item | Where |
|---|---|
| `ACTION_CLAIM = 1`; storage key `ClaimCommitments` (appended); `claim_commitments` in `Estate` | `src/lib.rs` |
| `#[payable] claim(label, attestation, signature)`: pokes; Unlocked; known heir; from the heir's account; attestation for this label; valid `ACTION_CLAIM`; not the grantor's face; face unused or used for this label; records it, charges its storage; logs `claim_attested` | `src/lib.rs` |
| `claim_commitment_used(commitment) -> Option<String>` | `src/lib.rs` |

### Tests

| Kind | Where | What |
|---|---|---|
| Unit (8) | `src/claim_tests.rs` | A claim after the deadline pokes, unlocks, snapshots and records the face; no claim while Active, in Grace or a nanosecond before unlock, and the poke rolls back with it; nine refusals each with their own error (unknown heir, grantor's account, sibling's account, signed for the sibling, no label, grantor's face, check-in attestation, another key, used nonce); one face can't take two slots; the same face retries its own slot with nothing attached; two heirs with their own faces both claim; the first claim must pay for its record; unknown and malformed commitments |
| Invariant | `src/claim_tests.rs` | 64 random runs of claims with any label, any face (the grantor's included), from the right account or a sibling's: each goes through exactly when the rules allow, no face ever holds two labels, and the grantor's face is never recorded |
| Sandbox (1) | `near/tests/estate_claims.rs` | Two real heir accounts signing with their own keys: too early refused; the sibling's account and the grantor's face refused; the son's claim unlocks, snapshots and records his face; his face can't then claim the daughter's slot |

Wasm: 249 KB.

## Part 3: Payouts

| Sepolia | Estate (NEAR) |
|---|---|
| `ClaimManager.claim` loops `tokensOf` | `claim` loops `tokens` |
| Skip a token already in `s_paid` | Skip an asset already in `paid: LookupSet<(label, asset)>` |
| `bps == 0`: skip, record nothing | The same |
| Mark paid, then `payOut`: `snapshot × bps / 10,000`, rounded down ("dust beats paying out more than was there") | The same; `payout_amount` |
| Amount 0: stays marked, not counted | The same |
| `_debit` before `_send` | Debited before the transfer is scheduled |
| `NothingToClaim` if no token paid | "nothing to claim" |
| `Claimed` per token, `ClaimSettled` | `claimed` per asset (from the callback), `claim_settled` |
| `claimableOf`, `claimableAll`, `hasClaimed` | `claimable_of`, `claimable_all`, `has_claimed` |

### Understand first

- **Shares of the snapshot, money out of the balance.** Each payout is the heir's effective share (Checkpoint 7 Part 3) of the asset's snapshot (Part 1), rounded down. The balance falls as heirs are paid, but the snapshot doesn't, so a daughter claiming a week after her brother still gets 40% of the same total. Rounding leaves dust in the estate rather than ever paying out more than was there.
- **Each asset once.** An asset is marked paid for that heir before its transfer is sent. A second claim skips it, and if nothing is left the claim is refused with "nothing to claim".
- **A failed transfer is undone, then retried.** Every transfer has its own `on_paid` callback. If the transfer failed, the callback credits the amount back to the balance and removes the paid mark; the heir claims again (Part 2 allows the same face to retry its own slot) and only the unpaid assets are sent.
- **The heir is registered with each token first.** A brand-new heir account isn't registered with mock USDC, so a bare `ft_transfer` to it would fail every time; Sepolia's ERC-20s needed no such step. So each token payout calls the token's `storage_deposit` for the heir, then `ft_transfer`. `then` runs the transfer whether or not the registration succeeded, so an heir who was already registered is still paid.
- **The heir pays the costs, not the estate.** `claim` is `#[payable]`: it charges the heir for the records it writes (the commitment the first time, a paid mark per asset), plus 0.00125 NEAR registration and 1 yoctoNEAR per token payout, and refunds the rest. Nothing comes out of what the other heirs are owed.
- **Gas.** A two-asset claim burnt 14 Tgas on the sandbox, about 0.0014 NEAR at testnet prices: the 0.1 NEAR seed covers it many times over. Payout callbacks get 5 Tgas, token calls 10.
- **`claimable_of` reads ahead.** Once the clock says Unlocked it shows the amount the snapshot will give even before anyone pokes, so the heir's page can show it straight away. 0 before unlock and once paid.

### Built

| Item | Where |
|---|---|
| Storage key `Paid` (appended); `paid` in `Estate`; `PAYOUT_CALLBACK_GAS` | `src/lib.rs` |
| `claim` now pays: loops the assets, skips paid and 0% ones, marks, computes, debits, sends NEAR or registers-then-`ft_transfer`s, each with `on_paid`; refuses "nothing to claim"; charges the heir; returns the number of transfers; logs `claim_settled` | `src/lib.rs` |
| `#[private] on_paid(label, asset, amount)`: logs `claimed`, or credits back, unmarks and logs `claim_failed` | `src/lib.rs` |
| `payout_amount`, `claimable`; views `claimable_of`, `claimable_all`, `has_claimed` | `src/lib.rs` |

### Tests

| Kind | Where | What |
|---|---|---|
| Unit (13) | `src/claim_tests.rs` | A claim pays the heir's share of every asset, debited first, one callback per transfer; heirs claiming a week apart split the same snapshot; 3 units split 60/40 pay 1 and 1 and leave 1 of dust; a share rounding to 0 is marked but not sent; a 0% asset is skipped and not marked; overrides are paid as set; a paid heir, and an heir with no share, have nothing to claim; a failed payout is credited back, unmarked, shows as claimable and is resent alone; a successful one is logged; the claimable views before unlock, unlocked but unpoked, and after a claim; the heir must pay the costs; a token payout registers the heir first. Part 2's retry test now retries after a failed payout |
| Fuzz | `src/claim_tests.rs` | 48 runs with random default shares, an optional override, random NEAR and mUSDC amounts, a random claim order and random transfer failures with retries: in the end each heir received exactly their rounded-down share of the snapshot, never more than the snapshot in total, and the books hold exactly the rest |
| Sandbox (1) | `near/tests/estate_claims.rs` | Two real heirs claim NEAR and mock USDC (the son with a 20% USDC override): their token balances and NEAR are exactly their shares, 1 unit of dust stays, a second claim finds nothing, and the estate still covers its books |

Wasm: 263 KB.

## Part 4: The lifecycle on a real node

One sandbox test, `near/tests/estate_lifecycle.rs`, runs a whole estate through its life: the deployment guide's §8.4 smoke run, minus the factory (Checkpoint 9 or later), with the attestor's signatures made by the test.

| Phase | What happens | What's checked |
|---|---|---|
| Setup | Two heirs with their own keys (son 60%, daughter 40%); the first check-in binds the grantor's face; 5 NEAR and 10 mUSDC deposited; the son's mUSDC share overridden to 25% | Status Active |
| Alive | The grantor withdraws 1 NEAR with a Selfie Check | The books drop by exactly 1 NEAR |
| A missed check-in | The clock passes the interval; a poke enters Grace; the grantor checks in | Back to Active: recovery works |
| Death | The grantor stops; the clock passes interval + grace; nobody pokes | Check-in, withdraw and deposit are all refused as "estate unlocked", unpoked |
| Son claims | From his own account with his own face | His claim logs `snapshotted`, `unlocked`, `claim_attested` first; the snapshot equals the balances at unlock |
| Daughter claims later | 50 blocks on, same | Paid off the same snapshot |
| The end | | Son: 25% of mUSDC (2.5) and 60% of the NEAR; daughter: 40% of each; the 35% of mUSDC nobody was given stays; under 2 yoctoNEAR of rounding dust; every pair marked paid; second claims find "nothing to claim"; the estate's free NEAR still covers its books |
| The trail | Every Herit event across the run | Exactly the expected events (the activity feed's story), nothing paid before the first claim, Grace before the snapshot |

### Understand first

- **Why one long test.** Each piece was tested on its own in Checkpoints 3–8. This one proves they fit: the face bound at the first check-in is the one `withdraw` and `check_in` later match; the snapshot taken by the son's claim is the one the daughter is paid from; Grace really is recoverable and Unlocked really isn't.
- **Events per receipt.** Each `claimed` comes from its own callback receipt, so it lands after `claim_settled`, in whichever order the transfers finish. The test compares the trail as a set and then checks the order of the phases. An indexer for the activity feed must do the same: order by block, not assume one transaction's events arrive together.
- **Only Herit's events.** The token logs its own NEP-141 `ft_transfer` events too; the trail keeps only `"standard":"herit"`.
- **The 35% nobody was given stays in the estate.** Shares of mUSDC add to 65% (25 + 40). As on Sepolia, unallocated shares aren't distributed; that's the grantor's choice when setting shares.

### Built

| Item | Where |
|---|---|
| The lifecycle test | `near/tests/estate_lifecycle.rs` |
| Helpers: `check_in_with(nonce)`, `withdraw`, `status`, `wait_for_status`, `events` (Herit events only) | `near/tests/common/mod.rs` |

The test takes about 75 seconds, most of it waiting for the clock.

## Check

```bash
cd near/contracts/herit-estate && cargo near build non-reproducible-wasm && cd ../..
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```
