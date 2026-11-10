# Checkpoint 10: The factory and the testnet deploy

Part of the [build plan](../near-build-plan.md). Starter code: [the deployment guide](../near-deployment-guide.md) §4 (accounts and the Global Contract budget), §6.3 (`herit-factory`), §6.4 (fallback) and §8.3–§8.4 (deploy and smoke run).

**Status:** done on 2026-11-10; all four parts reviewed. Herit is live on testnet, and a whole estate has run on it
**Who:** built together, in parts. Claude writes each part's code and tests and walks you through them; you review, and the next part starts after your OK. You sign the testnet transactions.

## Goal

Herit goes live on testnet: `herit-near.testnet` runs the factory, `open_estate` creates keyless estates such as `alice.herit-near.testnet` that all run one immutable, published copy of the estate code, and a whole estate runs on chain with every step linked on NearBlocks.

## Parts

| Part | What | Status |
|---|---|---|
| 1 | **Size:** measure the estate wasm and take the cheap savings, with no behaviour change | Done, reviewed |
| 2 | **The factory contract:** `open_estate`, its rollback, `set_attestor`, views; the grantor pays for the factory's records | Done, reviewed |
| 3 | **Publish to testnet:** mock USDC, the estate code as a Global Contract, the factory deployed and initialised in one transaction | Done, reviewed |
| 4 | **Smoke run:** a real estate opened through the factory, through its whole life, with NearBlocks links | Done, reviewed |

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

## Part 3: Published on testnet (2026-11-10)

You chose a **reproducible build** for what goes on chain, so anyone can check the published code is our source.

### Understand first

- **Why Docker, when Ethereum didn't need it.** Etherscan verifies by recompiling with the same `solc` version: `solc` gives the same bytes on any machine. Rust-to-wasm doesn't: the output depends on the toolchain, the OS, dependency versions, even the folder path (our local wasm contains `/Users/user/...` strings). So NEAR builds in one pinned Docker image, `sourcescan/cargo-near:0.21.1-rust-1.96.1@sha256:86bd27dd…`, pinned in our `Cargo.toml` since Checkpoint 2. SourceScan, which NearBlocks uses, rebuilds the commit in the same image and checks the hash. Docker only builds; nothing runs in it on chain.
- **Apple Silicon.** The image is `linux/amd64`. Docker's default QEMU emulation crashed `rustc` (`qemu: uncaught target signal 11`); with Docker Desktop's **Use Rosetta for x86_64/amd64 emulation** switched on, it builds. The image and the output don't change.
- **What the wasm says about itself.** Each reproducible build embeds NEP-330 metadata: the source link `github.com/FemiOje/herit-near/tree/bb8e6124…` (the commit pushed just before), the build image and the build command.
- **Tested before published.** The 37 sandbox tests ran on these exact wasm files, and the estate's hash was checked again just before the publish.

### What's on chain

| What | Account | Code | Transaction |
|---|---|---|---|
| Publisher topped up (22 NEAR from the factory) | `herit-code-kaykay.testnet` | | [`GB8kPYFZ…`](https://testnet.nearblocks.io/txns/GB8kPYFZwN7BdeCEUZt5H8ZnTGiqUP3U27PbXX5fVvYF) |
| Mock USDC (`mUSDC`, 6 decimals), 1,000,000 minted to the demo grantor `alice-kaykay.testnet` | `mock-usdc-kaykay.testnet` | local build (test token) | [`8rcMMBpM…`](https://testnet.nearblocks.io/txns/8rcMMBpMjE9agkakMJ278b48UExgqxWrPiepB7z7KCjA) |
| **Estate code, published as an immutable Global Contract by hash** | published by `herit-code-kaykay.testnet` | `87NJpRzyMsaL4WWYspngjbHoLBKPJSjiyrFEpDKXNgE8`, 302,829 bytes, reproducible | [`FvSiTJ2W…`](https://testnet.nearblocks.io/txns/FvSiTJ2W6etG8qX9Yx7iW8qxVE6AxTBEx52D3Ft5uHJ9) |
| **Factory, deployed and initialised in one transaction** | `herit-near.testnet` | `9Ppy6MHANNM3i5tR21pjmkgx9gKAwfoHyxtmFT6iNHpK` (hex `7cb68281…`), 151,499 bytes, reproducible | [`46VXZTEM…`](https://testnet.nearblocks.io/txns/46VXZTEMSGgmnWdjhwZvXXRtkZ4CKk6MsEaDnL1RVYf8) |

### Part 4: The smoke run (2026-11-10)

`alice.herit-near.testnet`, opened through the factory by `alice-kaykay.testnet`, run through its whole life by `near/scripts/smoke-testnet.sh` (120 s interval, 60 s grace). Every attestation came from the dev signer with the testnet attestor key, so the heirs could have faces different from the grantor's; the real Selfie Check path was proven in Checkpoint 5. Each heir got a fresh key that only their account holds, kept outside git.

| # | Step | Transaction | Events |
|---|---|---|---|
| 1 | The grantor opens `alice.herit-near.testnet` through the factory (1 NEAR) | [`56a5W8St…`](https://testnet.nearblocks.io/txns/56a5W8StD9NC5CXtHCwMP3xwz6XKvG78hYD7wpAfhfqm) | `estate_opened` (estate and factory) |
| 2 | Heir `son.alice.herit-near.testnet` (60%), with his own key | [`2J1LxqBp…`](https://testnet.nearblocks.io/txns/2J1LxqBp7ZBffc9zqGsTdnufwN65djfhmMyPSSFtdJLg) | `nft_mint`, `heir_added` |
| 2 | Heir `daughter.alice.herit-near.testnet` (40%), with her own key | [`EGJjZ6hh…`](https://testnet.nearblocks.io/txns/EGJjZ6hhx3p1TPfNJt3TBTnqtQWz1876XfpSVrPfUszS) | `nft_mint`, `heir_added` |
| 3 | Deposit 3 NEAR | [`BRMH7Uz7…`](https://testnet.nearblocks.io/txns/BRMH7Uz7sphkozW45RKoCTEruzrLMBSScA4GfxExqd1z) | `deposited` |
| 4 | Add mock USDC | [`3uz5YF6z…`](https://testnet.nearblocks.io/txns/3uz5YF6zQJLV2Q7Yo6MYkapekS8giCzhq8mRJDUcRaTW) | `token_added` |
| 4 | Deposit 10 mUSDC (`ft_transfer_call`) | [`De1Qs78K…`](https://testnet.nearblocks.io/txns/De1Qs78KtLyLTbefjcJkWDkdtLFW2cnjCDn18auZVfBH) | `ft_transfer`, `deposited` |
| 5 | The son's mUSDC share set to 25% | [`F7fxJbCU…`](https://testnet.nearblocks.io/txns/F7fxJbCUVMEvebt4mEdT848VAeJ9HUABvL8HbQyBZmgP) | `share_set` |
| 6 | First check-in: binds the grantor's face, starts the clock | [`EG4JCk3C…`](https://testnet.nearblocks.io/txns/EG4JCk3ChiE2C7dpCfER1NktvJWfPz4SzoctbdWj8Qj6) | `checked_in` |
| 7 | The grantor withdraws 0.5 NEAR with a fresh withdrawal attestation | [`CRVET1Rb…`](https://testnet.nearblocks.io/txns/CRVET1Rbb1hxMJvDRQMDwknSVreVxZKw2AzqWAYNLmPN) | `withdrawn` |
| 8 | Before the deadline: Active, 2.49838 NEAR and 10 mUSDC held, credentials `locked` | | |
| 9 | Three minutes on: the clock says Unlocked, nobody has poked, credentials `claimable` | | |
| 10 | **The son claims**, from his own account: his claim unlocks and freezes the estate, then pays him | [`576wFUGR…`](https://testnet.nearblocks.io/txns/576wFUGRt8CJaFNj3VzUaJ2ERAT4TCPnxPLP4DX6mseE) | `snapshotted`, `unlocked`, `claim_attested`, `claim_settled`, `claimed` ×2 |
| 10 | **The daughter claims**, from her own account | [`2KavSpcd…`](https://testnet.nearblocks.io/txns/2KavSpcdTV5hxeDUh9e5ebbTCz5REQL5Vce3B8u5VWsh) | `claim_attested`, `claim_settled`, `claimed` ×2 |

### The result

| | Son | Daughter | Left in the estate |
|---|---|---|---|
| mUSDC (snapshot 10) | 2.5 (his 25% override) | 4 (her 40%) | 3.5: the 35% nobody was given |
| NEAR (snapshot 2.49838) | 1.49903 (60%) | 0.99935 (40%) | 0 |
| Credential | `claimed` | `claimed` | |

The son's account now holds 1.594 NEAR (his share plus what was left of his 0.1 NEAR seed after costs), the daughter's 1.095 NEAR.

### Found: gas has to be held up front

The first claim attempt was refused before it was sent: "Account son.alice.herit-near.testnet does not have enough balance (0.1 NEAR) to cover TX cost (0.1109 NEAR)". NEAR reserves attached gas at a pessimistic price, about 0.1 NEAR per 100 Tgas, and only refunds what isn't burnt afterwards. So a signer must *hold* that much, even though a claim of two assets burns only about 14 Tgas (0.0014 NEAR). Checkpoint 8 put this down to the sandbox's gas price; it's NEAR's rule everywhere.

What it means:
- **With its 0.1 NEAR seed, an heir can attach about 80 Tgas.** A claim needs roughly 15 Tgas plus about 5 per NEAR payout and 25 per token payout (registration, transfer, callback): NEAR and up to two tokens fit. The run's claims attached 50 Tgas.
- **More tokens need more NEAR on the heir's account.** Anyone can send NEAR to an heir's account (it's an ordinary account), so the grantor, a co-heir or the app can top it up. The seed is in the published estate code, so it can't change on testnet.
- **For the frontend:** attach gas by the estate's asset count, not a fixed 300 Tgas, and say "send this heir 0.1 NEAR" when the seed won't cover it.
- **For mainnet:** a larger seed, or a claim that pays tokens across several calls, before publishing.

The script now attaches 50 Tgas to claims.

## Checked afterwards

- `herit-near.testnet` `version`: `0.1.0`; `config`: estate code hash `87NJpRzy…`, attestor `ed25519:FzTUQSiL…` (the testnet route's key from Checkpoint 5); `estates_count`: 0.
- The RPC's `view_global_contract_code` for `87NJpRzy…` returns all 302,829 bytes.
- `alice-kaykay.testnet` holds 1,000,000 mUSDC.

### What it cost

| Account | Before | After | Where it went |
|---|---|---|---|
| `herit-code-kaykay.testnet` | 10 + 22 NEAR | 1.71 NEAR | about 30.28 NEAR burned by the publish |
| `herit-near.testnet` | 47.99 NEAR | 25.99 NEAR | 22 NEAR to the publisher; 1.5 NEAR now locked for the factory's 151.8 KB |

## Check

```bash
cd near/contracts/herit-estate && cargo near build non-reproducible-wasm && cd ../..
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```
