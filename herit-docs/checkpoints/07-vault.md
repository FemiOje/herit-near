# Checkpoint 7: The vault

Part of the [build plan](../near-build-plan.md). Starter code: [the deployment guide](../near-deployment-guide.md) §6.1 (`deposit`, `ft_on_transfer`) and §6.5 (`add_token`, `set_share`, `withdraw`).

**Status:** all four parts built and tested on 2026-11-10; Part 4 waiting for your review
**Who:** built together, in parts. Claude writes each part's code and tests and walks you through them; you review, and the next part starts after your OK.

## Goal

The estate holds NEAR and stablecoins for the heirs, knows each heir's share of each asset, and lets the grantor take money back out only with a fresh Selfie Check. Claims (Checkpoint 8) pay out of this vault.

## Parts

| Part | What | Status |
|---|---|---|
| 1 | **NEAR deposits** on the `HeritVault` shape: per-asset `balances`, a `tokens` list capped at 10, `deposit()`, `balance_of`, `tokens_of` | Done, reviewed |
| 2 | **Token deposits:** `add_token` lists a NEP-141 token and registers the estate with it; `ft_on_transfer` is `depositERC20`. Sandbox test with mock USDC | Done, reviewed |
| 3 | **Per-asset shares** on the `HeritRegistry` shape: `set_share` overrides a default for one asset, every asset capped at 100% | Done, reviewed |
| 4 | **Withdrawals with a fresh Selfie Check:** `withdraw` needs an `ACTION_WITHDRAW` attestation from the grantor's bound face; debit first, credit back if the transfer fails | Done, waiting for your review |

## Part 1: NEAR deposits

Built on the shape of `HeritVault` from the Sepolia build, as you asked on 2026-11-10.

| `HeritVault` (Sepolia) | Estate (NEAR) |
|---|---|
| `s_balances[estateId][token]` | `balances: LookupMap<Asset, u128>`. One estate per contract, so no estate id |
| `NATIVE = address(0)` for ETH | `Asset = Option<AccountId>`: `None` is NEAR, `Some(token)` a NEP-141 contract |
| `s_tokens` + `s_listed`, listed on first deposit, `MAX_TOKENS = 10` counting ETH | `tokens: Vector<Asset>`, listed on first deposit, `MAX_TOKENS = 10` counting NEAR. Topping up a listed asset always works |
| `depositETH`: grantor, not unlocked, non-zero | `deposit()`: the same rules |
| `depositERC20` credits the measured delta, not the amount asked | `deposit()` credits what arrives: on the first deposit, the storage that listing NEAR takes comes out of the amount credited |
| `balanceOf`, `tokensOf` | `balance_of(asset)`, `tokens_of()` |
| `snapshot` on unlock | Checkpoint 8, in `poke_expiry` |

### Understand first

- **The balance is counted, never read from the account.** `HeritVault.balanceOf` doesn't use `address(this).balance`, and the estate doesn't use `env::account_balance()`, which also pays for storage and collects gas rewards.
- **Storage is the NEAR-specific part.** On Ethereum, gas pays for new storage. On NEAR, an account must hold 0.01 NEAR per 1 KB it stores. Listing NEAR writes about 104 bytes (a list entry and a balance entry), so the first deposit stakes about 0.001 NEAR. That comes out of what's credited, like Sepolia's rule of crediting what arrives. The books then never promise the heirs NEAR that is locked as storage. Later deposits write nothing new and are credited in full.
- **A deposit smaller than its storage cost is refused**, as is an empty one, one from anyone but the grantor, and one after unlock, whether or not anyone has poked the estate.

### Built

| Item | Where |
|---|---|
| `Asset`, `MAX_TOKENS`, storage keys `Balances` and `Tokens` (appended) | `src/lib.rs` |
| `balances: LookupMap<Asset, u128>`, `tokens: Vector<Asset>` in `Estate` | `src/lib.rs` |
| `#[payable] deposit()`; logs `deposited` with the amount credited | `src/lib.rs` |
| `list_asset` (`_listToken`, returns the storage cost) and `credit` | `src/lib.rs` |
| `balance_of(asset) -> U128`, `tokens_of() -> Vec<Asset>` | `src/lib.rs` |

### Tests

| Kind | Where | What |
|---|---|---|
| Unit (8) | `src/vault_tests.rs` | Starts empty; the first deposit lists NEAR and is credited less its storage; later deposits are credited in full, write no storage and don't re-list; too small for its storage refused; 0 refused; non-grantor refused; accepted in Grace; refused after an unpoked lapse |
| Fuzz | `src/vault_tests.rs` | Random runs of deposits, some by a stranger or too small: the balance is always the sum of the accepted ones less the one listing cost, and NEAR is listed once |
| Sandbox (2) | `near/tests/estate_vault.rs` | On a real node, the amount kept back equals the estate's extra storage times the byte cost, to the yoctoNEAR; later deposits are credited in full; a stranger's deposit is refused and returned, less gas |

Wasm: 196 KB.

## Part 2: Token deposits

| `HeritVault` (Sepolia) | Estate (NEAR) |
|---|---|
| `depositERC20(estateId, token, amount)`: the vault pulls with `transferFrom` | The grantor pushes: `ft_transfer_call` on the token, which calls the estate's `ft_on_transfer` |
| A token is listed on its first deposit | A token is listed by `add_token`, before its first deposit (see below). Same list, same `MAX_TOKENS = 10` counting NEAR |
| Credits the measured delta, for fee-on-transfer tokens | NEP-141 moves exactly `amount`, so `amount` is what arrived |
| A revert undoes the deposit | A panic in `ft_on_transfer` makes the token return the whole amount to the sender |

### Understand first

- **`add_token` is the one NEAR-only step.** An account must be registered with a token contract (NEP-145 `storage_deposit`, 0.00125 NEAR) before it can hold that token. A token transfer can't carry NEAR to pay for this, so the grantor calls `add_token` first and attaches the listing's storage plus the registration fee; the surplus is refunded. It lists the token where Sepolia's first deposit did, so the cap is checked at the same moment in the flow.
- **Registration is another contract's call, so it's asynchronous.** `on_token_registered` runs afterwards. If registration failed (for example, the account isn't a token), the fee has come back to the estate, so the callback unlists the token, keeping the others in order, and refunds the grantor.
- **Deposits come in through `ft_on_transfer`.** It refuses a token that wasn't added, a sender other than the grantor, a deposit after unlock, and a zero amount. Each refusal is a panic, so the token returns the tokens. Accepted deposits are credited in full and logged as `deposited`.
- **The caller of `ft_on_transfer` is the token,** so `predecessor_account_id()` is the asset. A fake contract can call it and claim to be any token, but it can only credit the balance of a token it actually is, which nothing else reads. The real token's balance can't be inflated this way.

### Built

| Item | Where |
|---|---|
| `#[payable] add_token(token)`: grantor, not unlocked, not already added; lists it, registers the estate, refunds the surplus | `src/lib.rs` |
| `#[private] on_token_registered(token, refund)`: logs `token_added`, or unlists, refunds and logs `token_failed` | `src/lib.rs` |
| `ft_on_transfer` (`FungibleTokenReceiver`), keeps the whole amount | `src/lib.rs` |
| `ext_ft::storage_deposit`, `TOKEN_REGISTRATION`, `is_listed`, `unlist_asset` | `src/lib.rs` |

### Tests

| Kind | Where | What |
|---|---|---|
| Unit (14) | `src/vault_tests.rs` | `add_token`'s `storage_deposit` call (estate, registration only, 0.00125 NEAR) and callback; surplus refund; fee missing; added twice; non-grantor; after unlock; NEAR plus 9 tokens fills the list, and 10 tokens leave no room for NEAR; `token_added`; a failed registration unlists in order, refunds, and the token can be added again; deposits credited in full and logged; token not added, stranger, after unlock and zero refused |
| Fuzz | `src/vault_tests.rs` | Random deposits from two added and two unknown tokens, by the grantor or a stranger: each token's balance is the sum of its accepted deposits, and nothing leaks into another asset |
| Sandbox (4) | `near/tests/estate_vault.rs` | With real mock USDC: `add_token` registers the estate and lists the token, and deposits land in the token contract and the books alike; a stranger's deposit, and a deposit of a token that was never added, come back in full; `add_token` on an account that isn't a token unlists it and leaves no NEAR in the estate |

Wasm: 209 KB.

## Part 3: Per-asset shares

Built on the shape of `HeritRegistry` from the Sepolia build.

| `HeritRegistry` (Sepolia) | Estate (NEAR) |
|---|---|
| `s_defaultShare`, total capped by `s_allocatedDefaultBps` | `Heir.default_bps` (Checkpoint 6), defaults capped at 10,000 in total |
| `s_share` + `s_hasOverride` | `shares: LookupMap<(String, Asset), u16>`: an entry exists exactly when overridden |
| `_effectiveShare`: override if set, else default | `share()`, and the view `share_of(label, asset)` |
| `_totalBps` | `total_bps()`, and the view `total_share_of(asset)` |
| `setShare` checks the prospective total: current total − this heir's share + new share | `set_share(label, asset, bps)`: the same check |
| `s_overriddenTokens`, at most `MAX_OVERRIDE_TOKENS = 10` | `override_assets: Vector<Asset>`, at most 10 |
| `recordHeir` checks the new default against each overridden token's total | `add_heir` does the same |
| `ShareSet` event | `share_set` event |

### Understand first

- **A share is per heir and per asset.** Each heir has a default share, and an override changes it for one asset only: "the son gets 60% of everything, but only 20% of the USDC". With shares in basis points, 10,000 is 100%.
- **Two totals guard 100%.** Defaults are capped at 100% in total. Each asset with an override also has its own total, the sum of every heir's effective share of it, capped at 100%. A new heir's default lands on every asset, so `add_heir` checks it against both: that is why the overridden assets are listed.
- **The prospective total** swaps the heir's current share for the new one before comparing, as Sepolia's comment explains. Otherwise lowering a share at 100% would look like an overflow, and raising one would slip through.
- **NEAR additions.** `set_share` is `#[payable]`: a new override is new storage, paid by the grantor's attached NEAR rather than out of the heirs' NEAR; changing an existing one costs nothing. And because NEAR has `remove_heir` and failed account creations, both clear that heir's overrides, so a label added later starts from its own default.
- **Pending heirs count.** An heir whose account is still being created is in every total, so their share is never handed out twice.

### Built

| Item | Where |
|---|---|
| `MAX_OVERRIDE_TOKENS`, storage keys `Shares` and `OverrideAssets` (appended), `shares` and `override_assets` in `Estate` | `src/lib.rs` |
| `#[payable] set_share(label, asset, bps)`: grantor, not unlocked, known heir, prospective total ≤ 100%, ≤ 10 overridden assets; logs `share_set` | `src/lib.rs` |
| `add_heir` also checks every overridden asset's total; `remove_heir` and a failed creation clear the heir's overrides | `src/lib.rs` |
| `share_of(label, asset) -> u16`, `total_share_of(asset) -> u16` | `src/lib.rs` |

### Tests

| Kind | Where | What |
|---|---|---|
| Unit (14) | `src/vault_tests.rs` | Defaults apply; an override changes one asset only and is logged; the prospective-total check, exactly 100% accepted and 1 bps over refused; lowering or restating at 100%; unknown heir, non-grantor and after unlock refused; a new override must pay its storage, changing one is free and the surplus is refunded; a new heir's default checked against an overridden asset; at most 10 overridden assets; `remove_heir` and a failed creation clear overrides |
| Invariant | `src/vault_tests.rs` | 64 random runs of adds, removes and share changes across 4 labels and 3 assets: every asset's total stays at or under 100% and equals the sum of the heirs' shares, and a refused call changes no share |
| Sandbox (1) | `near/tests/estate_vault.rs` | On a real node with two heirs, NEAR and mock USDC: an override applies to its asset only, pushing an asset past 100% is refused, and the estate's free NEAR (balance less storage) still covers everything the books promise the heirs |

Wasm: 225 KB.

## Part 4: Withdrawals with a fresh Selfie Check

| `HeritVault` (Sepolia) | Estate (NEAR) |
|---|---|
| `withdraw(estateId, token, amount)`: grantor, not unlocked | `withdraw(asset, amount, attestation, signature)`: the same, plus a Selfie Check |
| `_debit` before `_send`, so a re-entering recipient finds the balance reduced | Debited before the transfer is even scheduled |
| A failed `_send` reverts the whole call | The transfer runs later, in its own receipt; `on_withdrawn` credits the amount back if it failed |
| `Withdrawn` event | `withdrawn`, or `withdraw_failed` |

### Understand first

- **A stolen key alone moves nothing.** On Sepolia, whoever held the grantor's key could call `withdraw`. Here the call also needs a fresh `ACTION_WITHDRAW` attestation, and the face in it must be the one bound at the first check-in. A thief can sign the transaction, but can't produce the attestor's signature over the grantor's face, nor reuse an old one, because nonces are single-use. This is §2.3 item 5 of the deployment guide; it makes the README's threat model ("the thief who stole her key") hold end to end.
- **No face, no withdrawal.** Until the first check-in binds a face there's nothing to match, so `withdraw` is refused with "check in first".
- **Exactly 1 yoctoNEAR attached.** This is NEAR's convention for calls that move money: a function-call access key can't attach a deposit, so only the grantor's full-access key can withdraw. It also pays the token's `ft_transfer`, which requires 1 yoctoNEAR.
- **Debit first, credit back on failure.** NEAR has no reentrancy in the Ethereum sense, but a transfer is a separate receipt that can fail after `withdraw` returns, for example when the grantor isn't registered with the token. So the balance goes down before the transfer is scheduled, and the `#[private]` callback puts it back if the transfer failed. While a withdrawal is in flight, its amount is in neither the books nor the grantor's account.
- **Grace included,** as on Sepolia: a grantor who comes back after a missed check-in can still take their money out. Refused once unlocked.

### Built

| Item | Where |
|---|---|
| `ACTION_WITHDRAW = 2`, matching the worker and the attestation route | `src/lib.rs` |
| `#[payable] withdraw(asset, amount, attestation, signature)`: one yocto, grantor, not unlocked, no heir label, valid `ACTION_WITHDRAW` attestation for the bound face, 0 < amount ≤ balance; debits, then sends NEAR or calls `ft_transfer` | `src/lib.rs` |
| `#[private] on_withdrawn(asset, amount)`: logs `withdrawn`, or credits back and logs `withdraw_failed` | `src/lib.rs` |
| `ext_ft::ft_transfer` | `src/lib.rs` |

### Tests

| Kind | Where | What |
|---|---|---|
| Unit (10) | `src/vault_tests.rs` | NEAR: debited first, transfer and callback scheduled; tokens: `ft_transfer` to the grantor with 1 yocto; success logged; failure credited back; allowed in Grace; everything can be withdrawn; refused for a wrong face, a check-in attestation, an heir label, a stranger, another key's signature, a replayed nonce, zero, more than held, an asset not held, and after unlock, each leaving every balance as it was; refused before a face is bound; refused without exactly 1 yocto |
| Fuzz | `src/vault_tests.rs` | 64 random runs of deposits, withdrawals and transfers that succeed or fail in any order: the balance is always deposits less withdrawals in flight or sent |
| Sandbox (4) | `near/tests/estate_vault.rs` | On a real node: 1 NEAR leaves the estate and reaches the grantor, less gas; mock USDC comes back and both ledgers agree; a token transfer to an unregistered grantor fails and is credited back; a thief with the grantor's key but a forged or replayed attestation moves nothing |

A sandbox lesson: balance views read the last *final* block, a couple of blocks behind, so the test lets blocks pass before reading balances.

Wasm: 233 KB.

## Check

```bash
cd near/contracts/herit-estate && cargo near build non-reproducible-wasm && cd ../..
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```
