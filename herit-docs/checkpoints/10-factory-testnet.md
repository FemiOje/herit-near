# Checkpoint 10: The factory and the testnet deploy

Part of the [build plan](../near-build-plan.md). Starter code: [the deployment guide](../near-deployment-guide.md) §4 (accounts and the Global Contract budget), §6.3 (`herit-factory`), §6.4 (fallback) and §8.3–§8.4 (deploy and smoke run).

**Status:** Parts 1–2 done on 2026-11-10; Part 2 waiting for your review
**Who:** built together, in parts. Claude writes each part's code and tests and walks you through them; you review, and the next part starts after your OK. You sign the testnet transactions.

## Goal

Herit goes live on testnet: `herit-near.testnet` runs the factory, `open_estate` creates keyless estates such as `alice.herit-near.testnet` that all run one immutable, published copy of the estate code, and a whole estate runs on chain with every step linked on NearBlocks.

## Parts

| Part | What | Status |
|---|---|---|
| 1 | **Size:** measure the estate wasm and take the cheap savings, with no behaviour change | Done, reviewed |
| 2 | **The factory contract:** `open_estate`, its rollback, `set_attestor`, views; the grantor pays for the factory's records | Done, waiting for your review |
| 3 | **Publish to testnet:** mock USDC, the estate code as a Global Contract, the factory deployed and initialised in one transaction | |
| 4 | **Smoke run:** a real estate opened through the factory, through its whole life, with NearBlocks links | |

## Part 1: Size

### What's in the 303 KB

| Section | Bytes | What |
|---|---|---|
| Code | 259,679 | 745 functions: our methods, near-sdk's JSON and Borsh (de)serialisation for 50 exported methods, collections, formatting |
| Data | 40,114 | Strings and tables: our error messages and event names, JSON field names, near-sdk's and serde's messages, and some standard-library leftovers (I/O error texts, float formatting, about 5 KB of compiler file paths) |
| Everything else | ~3,500 | Types, imports, exports, the embedded ABI (4.7 KB compressed) |

### What was tried

| Change | Saved | Kept? |
|---|---|---|
| `.unwrap()`/`.expect()` on `Result`s in our code replaced with `env::panic_str` (same messages) or `env::abort` (Borsh encoding, which can't fail) | 846 bytes | Yes: no behaviour change, every test passes unchanged |
| Not embedding the ABI (`--no-embed-abi`) | 4.8 KB (about 0.5 NEAR of burn) | No: explorers and the NEAR CLI read it to list the contract's methods, which is worth half a NEAR |

The standard-library leftovers come from inside near-sdk and serde_json, not from our code, so removing them would mean patching dependencies. Real savings would need restructuring (fewer exported methods, hand-written JSON): not worth the risk or the time now.

### Decision

Publish at about **302 KB**: about **30 NEAR** burned, once, by `herit-code-kaykay.testnet`. You topped up `herit-near.testnet` to 48 NEAR on 2026-11-10, so the budget is covered without the §6.4 fallback.

| Account | Balance (2026-11-10) | Role |
|---|---|---|
| `herit-near.testnet` | 47.99 NEAR | The factory |
| `herit-code-kaykay.testnet` | 10 NEAR | Publishes the estate code; needs about 22 NEAR more before Part 3 |
| `herit-funder1-kaykay.testnet` | 4.99 NEAR | Spare |
| `herit-funder2-kaykay.testnet` | 10 NEAR | Spare |
| `alice-kaykay.testnet` | 10 NEAR | The demo grantor |
| `mock-usdc-kaykay.testnet` | 10 NEAR | The test token |

On mainnet the burn would be real NEAR; restructuring for size belongs on the post-hackathon list.

## Part 2: The factory contract

From §6.3 of the deployment guide, in `near/contracts/herit-factory/src/lib.rs` (151 KB, deployed normally to `herit-near.testnet`).

| Sepolia | NEAR |
|---|---|
| `AccessControlGate.openEstate` registers an ENS name and an estate id in shared contracts | `open_estate(label, check_in_interval_sec, grace_sec)` creates a whole account, `<label>.herit-near.testnet`, running its own estate contract |
| `estatesOfGrantor` | `estates_of(grantor)` |
| The subgraph's estate list | `estates(from_index, limit)` and `estates_count()` |

### Understand first

- **One batch, all or nothing.** `open_estate` sends one receipt to the new account with four actions: create the account, fund it, attach the published estate code by its hash (`use_global_contract`), and call `new` with the caller as grantor and the factory's attestor key. If any action fails (the name is taken, or `new` refuses the timers), none of them happen, and the funding comes back to the factory.
- **Keyless.** The batch adds no access key. The estate account can only ever do what its contract says: nobody, not the grantor, not the factory, not us, can redeploy, delete or drain it. And it runs the published code, which no one can change.
- **The grantor pays.** The estate gets the deposit less a 0.01 NEAR fee, which pays for the factory's two records of it (about 300 bytes). The deployment guide's version paid those from the factory's balance; this follows the estate's rule that whoever causes the storage pays for it. The minimum deposit is 0.11 NEAR: 0.1 for the estate's own state and buffer, 0.01 for the fee.
- **Rollback.** `on_estate_created` records the estate, or, if the batch failed, sends the grantor the *whole* deposit, fee included: nothing was recorded.
- **`set_attestor`** is `#[private]`: only the factory's own key can change the attestor, and only estates opened afterwards get the new one. Existing estates are keyless and keep theirs; that's how mainnet estates will move to the OutLayer key.
- **Testing published code.** The sandbox library can't publish a Global Contract, so a 31 KB test-only contract, `contracts/test-global-publisher`, publishes the real estate wasm in the sandbox. It is never deployed to testnet: there, the NEAR CLI publishes (Part 3).

### Built

| Item | Where |
|---|---|
| `new(estate_code_hash, attestor_pk)` (both checked), `version`, `#[private] set_attestor`, `#[payable] open_estate`, `#[private] on_estate_created`, `estates_of`, `estates`, `estates_count`, `config` | `contracts/herit-factory/src/lib.rs` |
| Test-only publisher | `contracts/test-global-publisher/` |

### Tests

| Kind | Where | What |
|---|---|---|
| Unit (12) | `contracts/herit-factory/src/tests.rs` | `new` stores the hash and key and refuses bad ones; the batch: create, funding less the fee, the published code by the exact hash from `new`, `new` with the caller as grantor and the timers, and no key; the minimum deposit; bad labels; a created estate is recorded and logged; a failed one refunds the whole deposit and records nothing; listing per grantor and in pages; `set_attestor` reaches new estates and refuses a bad key |
| Fuzz | `contracts/herit-factory/src/fuzz.rs` | Any label and deposit: `open_estate` goes ahead exactly when the label is valid and at least 0.11 NEAR is attached |
| Sandbox (4) | `near/tests/estate_factory.rs` | With the real estate wasm published: the estate account runs the Global Contract by its hash, has no keys, holds the deposit less the fee, and is recorded; the estate then lives a whole life (heir, check-in, deposit, unlock, claim) with the factory's attestor key and the caller as grantor; a taken name and bad timers each refund all but gas and create nothing; only the factory can run its callback or set the attestor |

## Check

```bash
cd near/contracts/herit-estate && cargo near build non-reproducible-wasm && cd ../..
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```
