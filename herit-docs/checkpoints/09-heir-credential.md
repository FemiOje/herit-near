# Checkpoint 9: The heir credential

Part of the [build plan](../near-build-plan.md). Starter code: [the deployment guide](../near-deployment-guide.md) §6.2 (`src/nft.rs`). Track: **#10 NFTs That Do Something**: "Dynamic NFTs · NEP-171 · Sub-accounts".

**Status:** all four parts built and tested on 2026-11-10; images pinned on Pinata; Part 4 waiting for your review
**Who:** built together, in parts. Claude writes each part's code and tests and walks you through them; you review, and the next part starts after your OK.

## Goal

Each heir holds a NEP-171 token that every NEAR wallet and explorer can show. It can't be transferred, and its state follows the estate live: `locked` → `grace` → `claimable` → `claimed`. It replaces Sepolia's `ROLE_HEIR_CLAIM` bit on an ENS subname: the token *is* the claim right, made visible.

## Parts

| Part | What | Status |
|---|---|---|
| 1 | **NEP-171 views:** `nft_metadata`, `nft_token`, `nft_tokens`, `nft_total_supply`, `nft_tokens_for_owner`, `nft_supply_for_owner`; metadata computed from live state | Done, reviewed |
| 2 | **Soulbound and the standard events:** every transfer and approval call refuses; `nft_mint` when an heir's account is created, `nft_burn` on `remove_heir` | Done, reviewed |
| 3 | **What it looks like:** one image per state on IPFS, chosen by the contract from the live state (your call, 2026-11-10, instead of an on-chain SVG) | Done, pinned, reviewed |
| 4 | **On a real node:** the credential through a whole lifecycle; every transfer route refused | Done, waiting for your review |

## Part 1: The NEP-171 views

| Sepolia | Estate (NEAR) |
|---|---|
| `ROLE_HEIR_CLAIM` granted on the heir's ENS subname by `unlockHeir` at unlock | A token per heir from the moment their account exists; its state, not its existence, changes at unlock |
| The role is a bit; nothing shows it | A NEP-171 token: wallets list it under the heir's account, explorers show it |
| `canClaim(estateId, label, heir)` | `claim` checks the caller is the heir's account (Checkpoint 8); the token mirrors that right |

### Understand first

- **One token per heir.** The token id is the heir's label (`son`), the owner is the heir's account (`son.alice.herit-near.testnet`), and the collection is the estate: each estate contract is its own NFT contract. An heir whose account is still being created has no token yet; a removed heir's token is gone.
- **Nothing is stored.** The token is computed from the heir record and the estate's state every time it's read. So it can't drift from the truth, costs no storage, and changes the moment the estate does: no transaction "updates" it.
- **The states.** `locked` while the estate is Active (including before the first check-in), `grace` in Grace, `claimable` once Unlocked, `claimed` when every asset the heir has a share of has been paid. All of them are computed from the clock, so they change without anyone poking. A failed payout makes the credential `claimable` again; an asset with a 0% share doesn't hold it back.
- **The metadata.** Title: the heir's account. Description: "Heir credential for `<estate>` · `<relationship>` · `<state>`". `extra`: JSON with the estate, relationship, default share and state, for apps. `copies: 1`. No approvals, ever.
- **Why not `near_contract_standards::NonFungibleToken`?** It stores tokens and brings approval and transfer machinery a soulbound token doesn't need, and every byte counts toward the Global Contract burn. The views use the standard's own types (`Token`, `TokenMetadata`, `NFTContractMetadata`), so the JSON is exactly what wallets expect.

### Built

| Item | Where |
|---|---|
| `nft_metadata`, `nft_token`, `nft_tokens`, `nft_total_supply`, `nft_tokens_for_owner`, `nft_supply_for_owner` | `src/nft.rs` |
| `credential_state`, `fully_paid`, `credential` (the computed token), `credentials` (filter and page) | `src/nft.rs` |
| Claim test helpers moved to `src/testing.rs` | `src/testing.rs` |

### Tests

| Kind | Where | What |
|---|---|---|
| Unit (9) | `src/nft_tests.rs` | The collection's metadata; a created heir's token (owner, title, description, `extra`, one copy, no approvals); none for a pending or unknown heir; a removed heir's is gone; locked → grace → claimable → claimed, computed without a poke, each heir separately; a failed payout makes it claimable again; a 0% asset doesn't hold it back; locked before the first check-in, whatever the date; enumeration, paging and per-owner supply |
| Invariant | `src/nft_tests.rs` | 64 random runs of adds, creations that succeed or fail, and removals: the tokens are exactly the created heirs, each owned by `<label>.<estate>`, every page size joins up into the full list, and each owner holds exactly one |
| Sandbox (1) | `near/tests/estate_nft.rs` | On a real node, `nft_tokens_for_owner` for the heir's account returns one standard NEP-171 token, `locked` |

Wasm: 281 KB (263 KB after Checkpoint 8).

## Part 2: Soulbound, and the standard events

### Understand first

- **Soulbound.** The credential is the heir's right to their share, and that right is bound to the heir's account: `claim` only accepts a call from `<label>.<estate>`. A transferable token would suggest a right that can be sold or stolen; it can't. So every NEP-171 transfer call (`nft_transfer`, `nft_transfer_call`) and every NEP-178 approval call (`nft_approve`, `nft_revoke`, `nft_revoke_all`) refuses with "heir credentials are soulbound", and `nft_is_approved` is always false. The methods exist, rather than being left out, so a wallet or marketplace that tries gets a clear answer instead of "method not found".
- **Standard events.** Indexers and explorers (NearBlocks, wallets' activity tabs) learn about tokens from NEP-297 events with `"standard":"nep171"`. Herit's own events (`"standard":"herit"`) are for its activity feed; these are for everyone else. Since nothing about a token is stored, the events are the only signal that one appeared or disappeared:
  - `nft_mint` when the heir's account has been created, in `on_heir_created`'s success branch. Not in `add_heir`: if the account creation fails, there never was a token.
  - `nft_burn` in `remove_heir`.
- **No event when the state changes.** `locked` → `claimable` is computed, not written; there's no transaction to log it from. Apps re-read `nft_token` (Part 3 makes wallets show the change).

### Built

| Item | Where |
|---|---|
| `nft_transfer`, `nft_transfer_call`, `nft_approve`, `nft_revoke`, `nft_revoke_all` (all refuse), `nft_is_approved` (always false) | `src/nft.rs` |
| `emit_mint`, `emit_burn` with `near_contract_standards`' `NftMint`, `NftBurn` | `src/nft.rs` |
| `on_heir_created` mints on success; `remove_heir` burns | `src/lib.rs` |

### Tests

| Kind | Where | What |
|---|---|---|
| Unit (5) | `src/nft_tests.rs` | `nft_mint` exactly once, with owner, token id and memo, only once the account exists; none when creation fails; `nft_burn` on removal; all five transfer and approval calls refuse, even from the owner with 1 yoctoNEAR, and the owner stays the same; nobody is approved |
| Sandbox (2) | `near/tests/estate_nft.rs` | From the heir's own account on a real node, `nft_transfer`, `nft_transfer_call` and `nft_approve` refuse and the token stays put; `add_heir`'s logs carry the `nft_mint` and `remove_heir`'s the `nft_burn` |

Wasm: 302 KB. The refusing methods and the event types cost about 20 KB; Checkpoint 9 Part 3 and the factory checkpoint should look at size together.

## Part 3: The images, on IPFS

You chose IPFS images, as EVM NFTs use, over an image built on chain: it keeps the contract small.

### Understand first

- **One image per state, not per heir.** Four SVGs, about 800 bytes each, in `near/assets/credential/`: dark slate *locked*, amber *grace*, green *claimable*, indigo *claimed*, each with Herit's key and the state's word. `nft_token` returns the one for the heir's current state as `media`. Because the state is computed live, the picture in the heir's wallet changes at the deadline, with no transaction and no server. The heir's account is the token's title, so it needn't be on the picture.
- **How a wallet finds it.** NEP-177: `media` holds the CID, and the collection's `base_uri` (`https://gateway.pinata.cloud/ipfs`) is the gateway; wallets fetch `base_uri/media`. `media_hash` is the file's SHA-256, so a wallet can check what the gateway sent.
- **Raw CIDv1.** For a file this small, a version-1 CID with the raw codec is just its SHA-256 with a prefix, written in base32 (`bafkrei…`). So the CID was computed here from the bytes, before any upload, and it can only ever name these exact bytes. A test recomputes all four from the files, and the encoder is checked against the published CID of the empty file.
- **Cost.** Four CIDs and hashes in the contract: about 0.7 KB, against several KB for an on-chain SVG.
- **What it depends on.** Someone must keep the files pinned. A pinning service, or several, does that; if they ever vanish, every credential still works on chain, only its picture is missing.

### Built

| Item | Where |
|---|---|
| The four images | `near/assets/credential/*.svg` |
| How to pin them, with the CID table | `near/assets/credential/README.md` |
| `CREDENTIAL_IMAGES`: (state, CID, SHA-256) × 4; `base_uri`; `media` and `media_hash` by state | `src/nft.rs` |

### Tests

| Kind | Where | What |
|---|---|---|
| Unit (4) | `src/nft_tests.rs` | Every CID and hash in the contract recomputed from its file; the CID encoder against the empty file's known CID; the collection's `base_uri`; locked → grace → claimable → claimed each show their own image, and the four differ |
| Sandbox | `near/tests/estate_nft.rs` | On a real node the token carries a `bafkrei…` `media` and a `media_hash`, and the collection the gateway |

Wasm: 303 KB.

### Pinned (you, 2026-11-10)

You pinned the four files on Pinata. Every CID matched, and Pinata's gateway returns each file byte for byte as `image/svg+xml`.

Opening `ipfs.io` showed the SVG as text: a raw CID carries no file type, `ipfs.io` redirects browsers to `inbrowser.link`, which guessed text, and it rate-limits other clients. Wallets draw `media` with an `<img>` tag, which won't draw an SVG labelled as text. So `base_uri` is now Pinata's gateway, which labels it correctly. The CIDs didn't change: any gateway serves the same files.

## Part 4: The credential through a whole life

The lifecycle test from Checkpoint 8 (`near/tests/estate_lifecycle.rs`) now reads both heirs' credentials the way a wallet does at every phase. Each read checks the state in `extra`, that `media` is that state's image CID, and that the description ends with the state.

| Phase | Son | Daughter |
|---|---|---|
| Set up, grantor alive | `locked` | `locked` |
| Missed check-in, before anyone pokes | `grace` | |
| Grantor checks in again | `locked` | |
| Deadline passed, nobody poked | `claimable`; his `nft_transfer` to the grantor refused as soulbound | `claimable` |
| Son has claimed | `claimed` | `claimable` |
| Daughter has claimed | | `claimed` |

### Understand first

- **No transaction moves the credential.** Every change in the table comes from the clock or from the heir's own claim; the estate never writes "the credential is now claimable". That's the dynamic-NFT story for the track: a token that wakes up when the grantor stops proving they're alive.
- **The transfer is tried at the worst moment.** Once claimable, the credential is the right to real money. That's exactly when an heir might be talked into selling it, or a thief would want it; it stays put.
- **Removal** (`nft_burn`) is covered by `near/tests/estate_nft.rs`, Part 2, rather than here, so the lifecycle's own event trail stays the one an estate with two heirs produces.

### Built

| Item | Where |
|---|---|
| `credential_cid(state)`, `credential_state(label)` (reads `nft_token`, checks image and description, returns the state) | `near/tests/common/mod.rs` |
| Credential checks at every phase, and the transfer attempt once claimable | `near/tests/estate_lifecycle.rs` |

## Check

```bash
cd near/contracts/herit-estate && cargo near build non-reproducible-wasm && cd ../..
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```
