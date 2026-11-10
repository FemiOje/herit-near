# Checkpoint 11: The frontend

Part of the [build plan](../near-build-plan.md). Starter notes: [the deployment guide](../near-deployment-guide.md) §9. Design: the Sepolia app on `main` (`frontend/design/DESIGN.md`, now copied here).

**Status:** Part 1 built on 2026-11-10, waiting for you to try a wallet and review
**Who:** built together, in parts. Claude writes each part and walks you through it; you review, and the next part starts after your OK.

## Goal

The Sepolia app's design and screens, on NEAR: wagmi, viem, ENS and the subgraph replaced by a NEAR wallet, contract reads over RPC, the live factory and estates on testnet, and events for the activity feed. Deployed on Vercel for the judges.

## Parts

| Part | What | Status |
|---|---|---|
| 1 | **Shell and NEAR plumbing:** theme, layout, nav and footer from `main`; HOT `near-connect` wallet; typed reads of the factory and estates; config | Built, waiting for your review |
| 2 | **The grantor's flow:** setup through the factory; dashboard with check-in by Selfie Check; the vault (deposits, token, withdraw by Selfie Check); heirs and shares | |
| 3 | **The heir's flow:** credential, claimable amounts, the claim with the heir's Selfie Check; gas by asset count. Decision: how an heir signs | |
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

## Check

```bash
cd frontend && pnpm test && pnpm exec tsc --noEmit && pnpm lint && pnpm build
```
