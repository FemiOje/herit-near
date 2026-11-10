# Herit on NEAR: Guided Build Plan

Companion to [near-deployment-guide.md](near-deployment-guide.md). That document says what Herit on NEAR is and holds the starter code; this one tracks what gets built, in what order, and who writes each piece.

**You write the two Rust contracts. Claude explains, reviews and unblocks.** Claude writes everything else: scaffolding, the OutLayer worker, sandbox tests, deploy scripts, the frontend and the README.

**Checkpoints are added one at a time.** Each has its own guide in [`checkpoints/`](checkpoints/), written only after the previous one is finished and verified.

Each checkpoint guide says:
- **Goal:** what gets built and why;
- **Understand first:** ideas to learn before starting;
- **Write:** what to write;
- **Check:** commands to run;
- **Verified at the stop:** what Claude checks before we move on.

Deadline: **2026-11-14 23:59 UTC**. Aim to submit by 12:00 UTC.

## How we test (as we build, never at the end)

- **You write unit tests for every method in the same checkpoint you write the method.** They live at the bottom of each `lib.rs` in `#[cfg(test)] mod tests` and use `testing_env!` to fake the caller, the time and attached NEAR (like `vm.prank`/`vm.warp`/`msg.value` in Foundry).
- **Unit tests can't run cross-contract calls, sub-account creation or callbacks after a real failure.** Where a checkpoint uses them, Claude adds a **sandbox test** at that same checkpoint: a real local NEAR node, so you see it working before moving on.
- **Claude writes the fuzz and invariant tests at that same checkpoint,** using `proptest`: fuzz tests for single methods, invariant tests as random call sequences with the invariants asserted after each call.
- **Every contract checkpoint ends green:** build + `cargo test` pass, with no warnings from `cargo clippy`.

---

## Progress

- [x] 1: [Baseline, tools, accounts](checkpoints/01-baseline-tools-accounts.md)
- [x] 2: [Rust workspace and test harness](checkpoints/02-workspace.md) (Claude)
- [x] 3: [Estate state machine](checkpoints/03-estate-state-machine.md) (Claude, at your request)
- [x] 4: [Estate attestations and check-in](checkpoints/04-estate-attestation.md) (Claude, at your request)
- [x] 5: [Selfie Check attestor](checkpoints/05-outlayer-selfie-check.md) (Claude + you): passed on testnet through the Next.js route; OutLayer worker ready for mainnet
- [x] 6: [Heirs as real sub-accounts](checkpoints/06-heirs.md) (Claude, at your request)

---

## Starting state

Checked in the repo on 2026-11-09:

- **No NEAR code yet.** There is no `near/` folder. The guide's §6 contracts type-check for wasm, and the §7 worker type-checks natively, but neither has been deployed or run.
- **Tools:** Rust 1.96, the `wasm32-unknown-unknown` target and Docker 29.9 are installed. `cargo-near` is 0.22 and NEAR CLI 0.30.1, both behind the guide's pins. `wasm32-wasip2` is not installed.
- **Frontend:** none on `kaykay`. The ETHGlobal app (Next.js 16.3.4, `wagmi`, `viem`, `@ensdomains/ensjs`, a subgraph) remains on `main` for reference.
- **Repo:** no `LICENSE` and no git tags. `kaykay` holds only the NEAR docs; the Foundry project remains on `main`.
- **Accounts:** `herit-near.testnet` and the helper accounts (suffix `kaykay`) were created on 2026-11-09, with keys in Keychain.

---

## Decisions taken

| Question | Decision |
|---|---|
| Who writes what | You: the 2 contracts. Claude: everything else |
| Network | NEAR testnet; a mainnet account only for the prize payout |
| Naming | `herit-near.testnet` → `alice.herit-near.testnet` → `son.alice.herit-near.testnet`, every level a real account |
| Factory account | `herit-near.testnet`, created and signed by the CLI, because Meteor blocks deploys from websites |
| Partner tech | Used first, wherever a feature needs it (guide §2.4) |
| Keys | macOS Keychain through `save-to-keychain`; never plaintext files or seed phrases on disk |
| ETHGlobal code | Deleted from the `kaykay` branch up front; it stays on `main`. The `pre-hackathon` tag keeps it recoverable and the diff valid |
| Branch | Build and push on `kaykay` |
| Demo token | Our own NEP-141 mock (`mock-usdc-<suffix>.testnet`), not Circle USDC |
| Non-native tech | Not used, with one exception: World ID Selfie Check, verified by the attestor, because no confirmed NEAR-native option proves recurring liveness |
| Attestor | Testnet: the Next.js route `POST /api/attest` verifies the Selfie Check with World and signs with a local key from `.env.local`, as the Sepolia app did (decided 2026-11-10, Checkpoint 5 fallback). Mainnet: the OutLayer worker in `near/attestor-worker/`, built and tested, so nobody holds the key. Same rules in both, and the contract doesn't change |
| World environment | `sandbox`, with the World sandbox testing app on an iPhone; it supports Selfie Check |
| Tracks | `nfts`, `realworld`, `open` |
| BTC, ETH, SOL | A stretch checkpoint, taken only once the core estate works on testnet. The estate holds them as NEAR Intents balances and pays them out in the heirs' split (guide §2.3 item 6). Mainnet only, with tiny amounts. Not Chain Signatures. First to cut |
