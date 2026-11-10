# Checkpoint 6: Heirs as real sub-accounts

Part of the [build plan](../near-build-plan.md). Starter code: [the deployment guide](../near-deployment-guide.md) §2.2 and §6.1 (`add_heir`, `on_heir_created`), and §6.5 (`remove_heir`).

**Status:** done on 2026-11-10
**Who:** Claude wrote the contract code and every test, at your request, as in Checkpoints 3 and 4

## Goal

The grantor names heirs, and each heir becomes a **real NEAR account under the estate**: `son.alice.herit-near.testnet`. The heir generates their own key and gives the grantor only the public half, so nobody else, including the grantor, can sign for that account. Later checkpoints pay out to these accounts (Checkpoint 8) and show them as NFT credentials.

## Understand first

| Sepolia Herit | Here |
|---|---|
| An heir is an ENS subname plus an address the grantor types in | An heir is a real account, `<label>.<estate>`, created by the estate contract |
| `registerHeir` writes a record | `add_heir` writes a record, then **creates the account in a later receipt**, so it can fail after the record exists |
| The grantor could mistype an address | The account name is derived from the label, and the heir's own key goes on it |

- **Only the estate can create `<label>.<estate>`.** NEAR lets an account create only its own direct sub-accounts. The estate has no keys, but its contract can issue the create-account action itself.
- **Account creation is asynchronous.** `add_heir` stores the heir with `created: false` and returns a batch: create the account, send it 0.1 NEAR, add the heir's key. A callback, `on_heir_created`, runs afterwards. On success it sets `created: true`. On failure, for example because the name already exists, the 0.1 NEAR has come back to the estate, so the callback removes the record and refunds the grantor.
- **A pending heir can't be removed.** Otherwise the grantor could remove "son" and add "son" again while the first creation was still in flight, and the first callback would land on the second record. Pending heirs also count towards the 100% share limit and the 10-heir cap.
- **Removing an heir leaves their account in place.** The estate holds no key to it. So a removed label can't be reused: creating the account would fail and roll back. This is intended: it's the heir's account now.
- **The grantor pays for it.** Attach NEAR with `add_heir`: the 0.1 NEAR seed (the account's storage, plus gas for the heir's claim later) and the heir record's storage. The rest is refunded in the same call.
- **`#[private]`** on the callback means only the estate itself may call it. Unit tests call methods directly and skip that check, so the sandbox test covers it.

## Built (Claude, 2026-11-10)

In `near/contracts/herit-estate/src/lib.rs`:

| Item | What it does |
|---|---|
| `Heir { account_id, relationship, default_bps, created }` | Stored in `heirs: IterableMap<String, Heir>` under the new storage key `Key::Heirs` (appended, so `Nonces` keeps its prefix) |
| `add_heir(label, public_key, relationship, default_bps)` | `#[payable]`. Grantor only, refused once unlocked (allowed in Grace). Label: one lowercase segment, `[a-z0-9_-]`, and a valid account id under the estate. Relationship: 1–32 letters, digits, spaces or hyphens, so it can go into JSON as is. Labels must be unused, there can be at most 10 heirs, and default shares can add up to at most 10,000 bps |
| `on_heir_created(label, refund)` | `#[private]` callback: confirms, or removes and refunds. Logs `heir_added` or `heir_failed` |
| `remove_heir(label)` | Grantor only, before unlock, created heirs only. Logs `heir_removed` |
| `heirs_of()`, `heir_of(label)` | Views returning `HeirView { label, account_id, relationship, default_bps, created }` |

Shares per asset (`set_share`) come with the vault in Checkpoint 7. The NFT mint and burn events come with the credential.

## Tests

| Kind | Where | What |
|---|---|---|
| Unit (23) | `src/heir_tests.rs` | The batch's actions (create, 0.1 NEAR, the heir's key, callback); surplus refund; too little NEAR; callback success and failure; every refusal (caller, unlocked, labels, relationships, taken, pending, 10 heirs, over 100%); exactly 100% accepted; adding in Grace; remove rules; views |
| Invariant | `src/heir_invariant.rs` | 64 random sequences of up to 60 calls (add, callback ok or failed, remove, check-in, time passing). After every step the heir list matches a model, each account is `<label>.<estate>`, shares stay at or under 100%, there are at most 10 heirs, and nothing changes once unlocked |
| Fuzz | `src/heir_invariant.rs` | Any string up to 70 characters is accepted as a label exactly when it is one lowercase segment that makes a valid account id |
| Sandbox (5) | `near/tests/estate_heirs.rs` | On a real local node: the account is created holding 0.1 NEAR and exactly the heir's key, and the heir can sign with it; the grantor pays only the seed, storage and gas; a label can't be added twice; a removed label's re-add fails, rolls back and refunds, with the estate keeping nothing; only the estate can run the callback; over 100% creates nothing |

## Check

```bash
cd near/contracts/herit-estate && cargo near build non-reproducible-wasm && cd ../..
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
(cd attestor-worker && cargo test)
```

## Verified at the stop

- [x] `cargo test --workspace` passes: 66 estate tests (with the earlier checkpoints'), 15 sandbox tests
- [x] `cargo clippy --workspace --all-targets -- -D warnings` is clean
- [x] The worker's 19 tests still pass against the changed contract
- [x] On a real node, the heir's account holds only the heir's key, and a failed creation leaves no NEAR in the estate
- [ ] **Watch the wasm size:** 182 KB, up from 132 KB. Publishing the Global Contract burns about 10 NEAR per 100 KB, so look for savings before the factory checkpoint publishes it

`spike.herit-near.testnet` keeps running the Checkpoint 4 code. Its stored state has no `heirs` field, so don't redeploy this version onto it; open a new estate instead.
