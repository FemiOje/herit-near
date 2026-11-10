# Checkpoint 2: Rust workspace and test harness

Part of the [build plan](../near-build-plan.md). Layout and pins come from [the deployment guide](../near-deployment-guide.md), §3 and §5.

**Status:** done, checks passed on 2026-11-10
**Who:** Claude scaffolds; you read it, run the checks, and ask about anything unclear

## Goal

Create the `near/` Cargo workspace, so that all three contracts build to wasm and every test layer runs before any real contract logic exists. From Checkpoint 3 on, you only write contract code and tests.

## Understand first

| Foundry | Here |
|---|---|
| `foundry.toml` | `near/Cargo.toml`, the workspace file |
| `src/Estate.sol` | `contracts/herit-estate/src/lib.rs` |
| `forge build` | `cargo near build non-reproducible-wasm`, run per contract |
| `lib/` submodules | `[workspace.dependencies]`, downloaded by Cargo |
| `forge fmt --check` | `cargo fmt --all --check` |
| Slither-style linting | `cargo clippy --all-targets -- -D warnings` |

- **One contract, one package.** Each contract is its own Rust package and compiles to its own `.wasm` file.
- **`crate-type = ["cdylib", "rlib"]` is required.** `cdylib` produces the wasm that goes on-chain. `rlib` lets tests import the contract as normal Rust.
- **`overflow-checks = true` must stay on.** Release builds of Rust wrap on overflow by default, unlike Solidity 0.8. This setting makes overflow panic instead.
- **`opt-level = "z"` optimises for size,** because publishing a Global Contract burns NEAR per byte.
- **Non-reproducible builds are for development.** Reproducible builds run in Docker and let others verify the on-chain code matches the source. We only need them at deploy time, so Docker can stay off for now.
- **`Cargo.lock` gets committed,** because reproducible builds run with `--locked`.

## Write (Claude)

```
near/
├── Cargo.toml                    # workspace, shared pins, release profile
├── Cargo.lock
├── contracts/
│   ├── herit-estate/             # placeholder until Checkpoint 3
│   ├── herit-factory/            # placeholder until the factory checkpoint
│   └── mock-ft/                  # our mock USDC
└── tests/                        # its own package: sandbox and fork tests
    ├── Cargo.toml
    ├── sandbox_smoke.rs
    └── fork_smoke.rs
```

1. **`near/Cargo.toml`** as in guide §5, with `tests` added to the members.
2. **`herit-estate` and `herit-factory`** as placeholders with one `version()` view each, so the build and tests have something to run. `herit-factory` doesn't depend on `near-contract-standards`.
3. **`mock-ft`** copied from `examples/fungible-token/ft` in `near-sdk-rs` at tag `near-sdk-v5.30.1`, renamed and switched to the workspace pins.
4. **`near/tests/`** as a package with `autotests = false` and one `[[test]]` entry per file. Holds `near-workspaces`, `tokio`, `anyhow` and `serde_json`, so the contract packages stay light.
5. **Reproducible-build metadata** in each contract's `Cargo.toml`, using the values from the `near-sdk-v5.30.1` example:
   - `image = "sourcescan/cargo-near:0.21.1-rust-1.96.1"`
   - `image_digest = "sha256:86bd27dd489aeb2f5004a7507d3699c045ce2f59f5287e201002966040da4a7d"`
6. **`target/`** added to the root `.gitignore`.

The contract `repository` value is `https://github.com/FemiOje/herit-near`, which matches `git remote -v`.

**Two changes from guide §5:** a workspace root can't own tests, so `near/tests/` has to be a package or nothing in it compiles. `near-workspaces` moves there, and `proptest` is added to the contracts' dev-dependencies.

## Tests

| Layer | Tool | Lives in | Written by | Starts |
|---|---|---|---|---|
| Unit | `testing_env!` | `#[cfg(test)] mod tests` in each `lib.rs` | You | Placeholder now, real from Checkpoint 3 |
| Fuzz | `proptest` | Each contract's `src/`, under `#[cfg(test)]` | Claude | Placeholder now, real from Checkpoint 3 |
| Invariant | `proptest` random call sequences | Each contract's `src/`, under `#[cfg(test)]` | Claude | Checkpoint 3, once there is state to protect |
| Sandbox / integration | `near-workspaces` on a local node | `near/tests/` | Claude | `sandbox_smoke` now |
| Fork | `near-workspaces` `import_contract` | `near/tests/` | Claude | `fork_smoke` now |

- **A sandbox test loads the built `.wasm`.** Rebuild the contract first, or the test runs old code. Paths are relative to the package, for example `../target/near/herit_estate/herit_estate.wasm`.
- **Fork tests are limited.** They copy one live contract, and optionally its data, into the sandbox. They don't fork the whole chain like `anvil --fork`.
- **`fork_smoke` is marked `#[ignore]`** because it needs the network. Plain `cargo test` works offline, and `--ignored` runs it.

## Check

Run from `near/`:

```bash
for c in herit-estate herit-factory mock-ft; do
  (cd contracts/$c && cargo near build non-reproducible-wasm)
done
ls -l target/near/*/*.wasm

cargo test                                  # unit, fuzz, sandbox
cargo test -p herit-tests -- --ignored      # fork
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
```

Claude ran all of these on the real workspace on 2026-11-09 and they passed. The reproducible build hasn't been tried yet.

## Verified at the stop

- [x] All three contracts produce a `.wasm`
- [x] `cargo test` passes, including the sandbox smoke test
- [x] The fork smoke test passes with `--ignored`
- [x] `cargo clippy` shows no warnings and `cargo fmt` is clean
- [x] `target/` is ignored and `near/Cargo.lock` is tracked
