# Checkpoint 11: The frontend

Part of the [build plan](../near-build-plan.md). Starter notes: [the deployment guide](../near-deployment-guide.md) §9. Design: the Sepolia app on `main` (`frontend/design/DESIGN.md`, now copied here).

**Status:** Parts 1 and 2 done (Part 2 committed 2026-11-11, with named heirs). Part 3 in progress: 3a done, 3a and 3b done, 3c next (2026-11-11)
**Who:** built together, in parts. Claude writes each part and walks you through it; you review, and the next part starts after your OK.

## Goal

The Sepolia app's design and screens, on NEAR: wagmi, viem, ENS and the subgraph replaced by a NEAR wallet, contract reads over RPC, the live factory and estates on testnet, and events for the activity feed. Deployed on Vercel for the judges.

## Parts

| Part | What | Status |
|---|---|---|
| 1 | **Shell and NEAR plumbing:** theme, layout, nav and footer from `main`; HOT `near-connect` wallet; typed reads of the factory and estates; config | Done |
| 2 | **The grantor's flow:** setup through the factory; dashboard with check-in by Selfie Check; the vault (deposits, token, withdraw by Selfie Check); heirs and shares | Done |
| 3 | **The heir's flow:** credential, claimable amounts, the claim with the heir's Selfie Check; gas by asset count. Decision: how an heir signs | In progress |
| 4 | **Landing, explorer, activity feed, deploy:** the landing page; estates from the factory; `/api/activity` over FastNEAR; credentials with their IPFS images; Vercel | |

## Part 1: Shell and NEAR plumbing

| Sepolia app (`main`) | NEAR app |
|---|---|
| `WagmiProvider` + wallet picker | `WalletProvider` around HOT's `NearConnector` (`lib/near/wallet.tsx`); the connector's own picker lists HOT, Meteor, Intear, MyNearWallet and the rest |
| `useReadContract` + ABIs | `view(contractId, method, args)` over JSON-RPC (`lib/near/rpc.ts`) and typed reads `factory.*` and `estate(id).*` (`lib/near/contracts.ts`) |
| `writeContract` | `useWallet().call(receiverId, method, args, { tgas, deposit })`, which returns the transaction hash or throws the contract's panic message |
| Sepolia addresses in `lib/contracts/addresses.ts` | `NEXT_PUBLIC_NEAR_NETWORK` (default `testnet`) and `NEXT_PUBLIC_HERIT_FACTORY` (default `herit-near.testnet`) in `lib/near/config.ts` |
| ENS names, `viem` formatting | NEAR account ids and `bigint` amounts (`lib/near/account.ts`, `lib/near/format.ts`) |

### Understand first

- **Reads need no wallet.** Every view is a JSON-RPC `call_function` against FastNEAR's RPC, falling back to NEAR's own on a network error. A contract's own error is never retried. Amounts stay strings on the wire and `bigint` in the app.
- **Writes go through the wallet.** `near-connect` runs each wallet's integration in a sandboxed iframe and shows its own picker. The app only ever asks the connected wallet to sign one function call. The connector loads in the browser only.
- **Which wallets.** On testnet, the connector only offers wallets that declare testnet support; HOT's own manifest says it doesn't. Meteor, MyNearWallet and Intear do. On mainnet HOT is first in the list.
- **Heir to estate is a string operation.** `son.alice.herit-near.testnet` → `alice.herit-near.testnet`: strip the first segment.
- **Next.js 16 with `cacheComponents`.** The shell is static; everything that depends on the wallet or the chain is a client component reading through TanStack Query.

### Built

| Item | Where |
|---|---|
| Theme, design docs, icons, decorations, logo, footer, page header, mobile nav, status pill | Copied from `main`: `app/globals.css`, `design/`, `components/ui/`, `components/layout/`, `components/estate/status-pill.tsx` |
| Root layout (DM Sans, Caveat), providers (TanStack Query, wallet), app layout | `app/layout.tsx`, `app/providers.tsx`, `app/(app)/layout.tsx` |
| Config, RPC views, typed reads, account and amount helpers, wallet | `lib/near/` |
| Nav (rewritten without ENS), wallet button and menu (account, copy, NearBlocks, disconnect), NEAR nav content | `components/layout/site-nav.tsx`, `components/wallet/connect-wallet.tsx`, `lib/content/navigation.ts` |
| Dashboard: the connected account's estates from the factory, each with its live status and unlock time | `app/(app)/dashboard/page.tsx`, `components/dashboard/my-estates.tsx` |
| A placeholder home page until Part 4 | `app/page.tsx` |

### Tests

| Kind | Where | What |
|---|---|---|
| Unit (7) | `lib/near/near.test.ts` | Account ids; estate ↔ heir mapping; parsing and formatting amounts, truncating, dust; percentages and rounded-down shares; the credential state from `extra`; `view` decoding, its base64 args, a contract error being final and a network error falling back |
| Build | | `tsc`, `eslint`, `next build` |
| Live | | The factory read from the browser's RPC: `estates_of("alice-kaykay.testnet")` → `["alice.herit-near.testnet"]` |

## Part 2: The grantor's flow

### Understand first

- **An estate isn't live until its grantor's face is bound and it has heirs.** The contract can't demand both in `open_estate`: the account must exist before anything can be sent to it. The app does: setup is a wizard (name and clock, heirs, vault, proof of life), and nothing is sent until the last step. There the Selfie Check runs first (its attestation names `<label>.herit-near.testnet`, which is known before the account exists), and then one wallet approval sends open → `check_in` → each `add_heir` → `deposit` (`signAndSendTransactions`). The batch is worked out from what's on chain each time (`setupPlan`), so if it stops halfway, "try again" or `/setup?estate=` sends only what's missing. The dashboard shows an estate without both as "setup unfinished" and links back to the wizard instead of its cards.
- **The estate's header comes from its raw state.** The contract has no view for its grantor or timers, and its code is a published Global Contract, so it can't grow one. `view_state` with the prefix `STATE` returns the Borsh record, whose first fields are factory, grantor, attestor key, the bound face (an `Option`), interval, grace and the last check-in (`lib/near/estate-state.ts`). That gives the ring its real proportions and tells the page whether the wallet is the grantor.
- **A withdrawal is a third Selfie Check purpose.** `withdraw` uses the same World action as the check-in, `herit:<label>`, so the same human gets the same nullifier and the same commitment, which the estate compares with the face the first check-in bound. The attestation carries `ACTION_WITHDRAW` (2) and no heir. Both `/api/worldid/sign` and `/api/attest` accept it.
- **The wallet is opened by a click.** After the Selfie Check the modal waits for "send": a wallet opens a popup, and a popup opened at the end of an async chain is blocked. The attestation lasts 15 minutes, so a failed send can be retried with it.
- **Every write is data first.** `lib/near/calls.ts` builds each call (receiver, method, args, gas, deposit), so the deposits the contract demands live in one place and are tested: exactly 1 yoctoNEAR on `withdraw` and on `ft_transfer_call`, 0.2 NEAR on `add_heir` (record + the 0.1 NEAR seed), 0.05 NEAR on `add_token`, 0.01 NEAR on `set_share`, at least 0.11 NEAR on `open_estate`. Surpluses are refunded by the contracts.
- **An heir is an account they have, or a key they made.** "Their NEAR account" names an existing account, such as `kaynearhackathon2.testnet`, with `add_heir_account` (0.01 NEAR): the heir claims from it with their own wallet. "A key they made" is the original way: the heirs page's key maker makes an ed25519 pair in the browser, the heir sends the public half, and `add_heir` creates `<label>.<estate>` holding it (0.2 NEAR). The app checks a named account exists first (an implicit one needn't), and refuses the grantor, the estate and anything under it, and an account already named, as the contract does. The contract change and its republish are recorded in [Checkpoint 10](10-factory-testnet.md#republished-named-heirs-2026-11-11).
- **Older estates take keys only.** An estate keeps the code it was opened with. The app reads its `global_contract_hash`: on `87NJpRzy…` (before named heirs), such as `alice` and `hh`, "their NEAR account" is greyed out.
- **A token deposit goes through the token.** `ft_transfer_call` on the token, with the estate as receiver; the estate's `ft_on_transfer` credits it. The token must be added first (`add_token`), which registers the estate with it.

### Built

| Item | Where |
|---|---|
| The `withdraw` Selfie Check purpose | `lib/selfie-check.ts`, `lib/attest.ts`, `app/api/worldid/sign/route.ts` |
| Estate header from raw state; account view; token views | `lib/near/estate-state.ts`, `lib/near/rpc.ts` (`viewAccount`), `lib/near/contracts.ts` (`ft`) |
| Every grantor transaction as data | `lib/near/calls.ts` |
| Sending and tracking a transaction, then refetching | `lib/near/use-tx.ts`, `components/estate/tx-status.tsx` |
| The clock: phases, countdowns, presets, the contract's bounds | `lib/estate/clock.ts`, `lib/estate/use-now.ts` |
| One estate's reads: header, status, heirs, vault, shares | `lib/estate/use-estate.ts` |
| Primitives from `main` | `components/ui/{card-heading,data-row,stage-list,icon-circle}.tsx`, `components/estate/countdown-ring.tsx` |
| Selfie Check modal: World ID, `/api/attest`, then the transaction | `components/selfie-check/selfie-check-modal.tsx` |
| Setup wizard: name and clock, heirs (at least one), vault, then Selfie Check and one batch; resumable; the draft kept per wallet in the browser | `app/(app)/setup/page.tsx`, `components/setup/setup-wizard.tsx`, `lib/estate/setup-plan.ts`, `lib/estate/use-setup.ts` |
| Batches under one approval | `lib/near/wallet.tsx` (`callMany`), `lib/near/use-tx.ts` |
| Dashboard: estate picker (`?estate=`), proof of life, vault, heirs summary | `app/(app)/dashboard/page.tsx`, `components/dashboard/` |
| Heirs page: add, remove, per-asset shares; the heir's key maker | `app/(app)/heirs/page.tsx`, `components/heirs/` |
| Named heirs: the "their NEAR account / a key they made" input, the existence check, which code takes them | `components/heirs/heir-target.tsx`, `lib/near/use-account.ts`, `lib/near/account.ts` (`namedHeirProblem`), `lib/near/calls.ts` (`addHeirAccount`, `supportsNamedHeirs`) |

### Tests

| Kind | Where | What |
|---|---|---|
| Unit (6) | `lib/near/grantor.test.ts` | The header decodes with and without a bound face, and from an RPC reply; the calls' deposits and receivers; public keys; the clock's phases and progress; durations and countdowns |
| Unit (5) | `lib/estate/setup-plan.test.ts` | A fresh setup is open, check-in, heirs, deposit in order; none without the Selfie Check; a half-done setup sends only what's missing; no heirs is no setup; shares, labels, keys and timers checked first |
| Unit (1) | `lib/attest.test.ts` | A withdrawal is action 2, names no heir, and gets the check-in's commitment |
| Unit (5) | `lib/estate/setup-plan.test.ts`, `lib/near/grantor.test.ts` | A mixed batch sends `add_heir` and `add_heir_account` with the right deposits; a named heir can't be the grantor, under the estate, or named twice; a draft saved before named heirs still reads as keys; only the old code refuses named heirs; implicit accounts needn't exist |
| Unit | `lib/near/near.test.ts` | One-character labels are valid, as in the contract (they were refused before) |
| Live | | Through the app's RPC code: `kay` (new code) takes named heirs, `alice` and `hh` don't; `kaynearhackathon2.testnet` exists, a made-up account doesn't |
| Live | | The header of `alice.herit-near.testnet` decoded over RPC: grantor `alice-kaykay.testnet`, 120 s, 60 s |
| Build | | `tsc`, `eslint`, `next build` |

### Try it

1. `/setup`: name, 2 min + 1 min clock, at least one heir (name an account such as `kaynearhackathon2.testnet`, or make a key on `/heirs` or with "make one here"; try one of each), some NEAR for the vault. On the last step, the Selfie Check in World App (sandbox), then approve the batch in your wallet. The dashboard opens with the clock running.
2. An estate opened before this change (such as `hh`) shows "setup unfinished": finish it from there.
3. Add mUSDC and deposit it (the mock token has no faucet, see below).
4. Withdraw a little NEAR: a second Selfie Check, then `withdraw`.
5. On `/heirs`, add another heir or set a different mUSDC share. On `alice` or `hh`, "their NEAR account" is greyed out: old code.

The mock token has no mint. To give a new wallet some, register it and send from `alice-kaykay.testnet`, which holds the supply:

```bash
near contract call-function as-transaction mock-usdc-kaykay.testnet storage_deposit json-args '{"account_id":"<wallet>"}' prepaid-gas '30 Tgas' attached-deposit '0.00125 NEAR' sign-as alice-kaykay.testnet network-config testnet sign-with-keychain send
near contract call-function as-transaction mock-usdc-kaykay.testnet ft_transfer json-args '{"receiver_id":"<wallet>","amount":"100000000"}' prepaid-gas '30 Tgas' attached-deposit '1 yoctoNEAR' sign-as alice-kaykay.testnet network-config testnet sign-with-keychain send
```

## Part 3: The heir's flow

**Decided (2026-11-11):** a named heir signs the claim with their own wallet; a key heir opens the key file they saved, and the app signs and sends the claim in their browser with `near-api-js` (the key never leaves the device). Built in three steps, each reviewed: **3a** finding a slot and the heir view, **3b** the claim, **3c** signing with the key file.

### 3a: Finding a slot, and the heir view

#### Understand first

- **Two ways to find a slot.** A key heir's account is `<label>.<estate>`, so its estate is in its name (`heir_of` confirms it). A named heir can be any account, so every estate the factory opened is asked `nft_tokens_for_owner(account)`, eight at a time, a page of 50 at a time, up to 1,000. That's under a second for testnet's handful of estates; an indexer would replace it at scale. An estate that can't be read is skipped. One account can hold slots in several estates.
- **Viewing needs no wallet.** Everything on `/claim` is a view call, so anyone can look up any heir account. Signing is 3b and 3c.
- **Amounts before unlock are estimates.** `claimable_all` is 0 until the estate unlocks. The heir view works out what the contract's `payout_amount` would pay: the balance (or, once taken, the snapshot) times the heir's share of that asset, rounded down. Each asset reads "if it unlocked now", "claimable", "paid", or "no share".
- **The credential** is the estate's live NEP-171 token: its state (locked → grace → claimable → claimed) and its IPFS image change with the estate.

#### Built

| Item | Where |
|---|---|
| Slot discovery, with the chain reads passed in | `lib/heir/slots.ts` |
| One asset's line: none, expected, claimable, paid | `lib/heir/share.ts` |
| Hooks: slots for an account; an heir's share, paid mark and snapshot per asset | `lib/heir/use-heir.ts` |
| `heir_of` and `nft_tokens_for_owner` reads | `lib/near/contracts.ts` |
| `/claim`: your slots (connected wallet), or look up any account | `app/(app)/claim/page.tsx`, `components/claim/claim-view.tsx` |
| The heir view: where the estate stands, your share per asset and what a claim needs, your credential, your co-heirs | `components/claim/{unlock-banner,share-card,credential-card,co-heirs-card}.tsx` |

#### Tests

| Kind | Where | What |
|---|---|---|
| Unit (7) | `lib/heir/heir.test.ts` | A named heir is found in every estate naming them; a key heir from their name, once; an unreadable estate is skipped; paging stops at the cap; lines before and after unlock, rounded as the contract does; a claim is only worth sending with something claimable |
| Live | | Through the app's read code: `kaynearhackathon2.testnet` → `kay2` in `kay`; `son.alice.herit-near.testnet` → `son` in `alice`; an unknown account → none |
| Build | | `tsc`, `eslint`, `next build` |

#### Try it

1. `/claim` with `kaynearhackathon2.testnet` connected (or typed into "look up"): `kay.herit-near.testnet`, heir `kay2`.
2. `/claim?estate=alice.herit-near.testnet&heir=son`: unlocked, both assets paid, credential `claimed`.
3. `/claim?estate=kay.herit-near.testnet&heir=kay2`: sealed, clock not started; no vault yet.

### 3b: The claim

#### Understand first

- **One button, every check first.** "Selfie check and claim" is enabled only when the estate is unlocked, the connected wallet is the heir's account, something unpaid has an amount, and that account holds enough NEAR. Otherwise the button says which one is missing. The claim itself pokes an unlocked estate nobody has poked, which freezes the balances.
- **Sized to its payouts.** Gas comes from the estate's own constants: about 15 Tgas for the call, 5 per NEAR payout (transfer and `on_paid`), and 25 per token payout (registration, transfer, `on_paid`), plus 10 of headroom, rounded up to 5. NEAR and one token is 55 Tgas, and the contract's 10 assets stay under 300. The deposit covers what the claim writes (0.005 NEAR plus 0.002 per asset) and, per token, the heir's registration with it (0.00125 NEAR) plus the 1 yoctoNEAR `ft_transfer` needs. The estate refunds the surplus.
- **The NEAR the heir must hold.** NEAR reserves attached gas up front at about 0.1 NEAR per 100 Tgas (found in Checkpoint 10), so the account must hold that plus the deposit: 0.065 NEAR for NEAR and one token, less than a key heir's 0.1 NEAR seed. The card shows the shortfall when there is one, and anyone can send the rest.
- **The attestation names the sender.** The Selfie Check's subject is the connected account. `/api/attest` accepted any account all along; only a comment said otherwise.
- **Key heirs wait for 3c.** Their button says signing with the key file comes next.

#### Built

| Item | Where |
|---|---|
| A claim's gas, deposit and the NEAR to hold; what an account can spend | `lib/heir/claim-cost.ts` |
| The `claim` call | `lib/near/calls.ts` |
| The button, its reasons, "NEAR for the gas", and the Selfie Check that sends the claim | `components/claim/share-card.tsx` |

#### Tests

| Kind | Where | What |
|---|---|---|
| Unit (1) | `lib/heir/heir.test.ts` | NEAR and one token is 55 Tgas and 0.01025 NEAR + 1 yocto, held under the 0.1 NEAR seed; 10 assets stay under 300 Tgas but need more than the seed; storage is subtracted from a balance |
| Live | | On `kay.herit-near.testnet` (1 NEAR, 10 mUSDC, dev check-in, 2 min + 1 min): the named heir `kay-cousin-kaykay.testnet` (25%, a faucet account) claimed with exactly what the app sends, 55 Tgas and 0.01025 NEAR + 1 yocto ([`7UJnn88i…`](https://testnet.nearblocks.io/txns/7UJnn88iX5f4naW5gAToi862Jyswm8Kiu3XGNuSbQHHk)): `snapshotted`, `unlocked`, `claim_attested`, `claim_settled`, `claimed` ×2. It got 0.2449 NEAR (its 0.25 less about 0.005 of costs) and 2.5 mUSDC |
| Build | | `tsc`, `eslint`, `next build` |

#### Try it

1. Connect `kaynearhackathon2.testnet` and open `/claim?estate=kay.herit-near.testnet&heir=kay2`: unlocked, 0.5 NEAR and 5 mUSDC claimable, every check met.
2. "Selfie check and claim": World App (sandbox), then approve the claim in the wallet. The lines turn "paid" and the credential `claimed`.
3. Connected as another account, the button says to connect `kaynearhackathon2.testnet`. On `alice`'s `son`, it says a key heir signs with their key file (3c).

## Check

```bash
cd frontend && pnpm test && pnpm exec tsc --noEmit && pnpm lint && pnpm build
```
