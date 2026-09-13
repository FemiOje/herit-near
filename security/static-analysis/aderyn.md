# Aderyn — static analysis of `src/`

```bash
aderyn --src src .
```

Aderyn 0.6.8 · solc 0.8.30 · 13 files, 1,032 nSLOC · **8 issues (1 high, 7 low)**
Run against commit `a8f39f4e`.

## Summary

| ID | Issue | Count | Verdict |
|---|---|---|---|
| H-1 | Reentrancy: state change after external call | 11 sites | Accepted — see below; overlaps Slither ID-4/ID-5 |
| L-1 | Costly operations inside loop | — | Accepted — bounded loops |
| L-2 | Large numeric literal | 3 | Accepted — basis points and time constants |
| L-3 | Modifier invoked only once | — | Accepted — named for readability |
| L-4 | `nonReentrant` is not the first modifier | 2 | **Worth fixing — cheap** |
| L-5 | Loop contains `require`/`revert` | — | Accepted — deliberate fail-fast |
| L-6 | Unchecked return | 5 | 1 overlaps Slither ID-7; rest deliberate |
| L-7 | Unused error | 2 | **Real finding — dead declarations** |

Two items are worth acting on: **L-4** and **L-7**. Neither is a vulnerability.

---

## High

### H-1 · Reentrancy: state change after external call

Eleven sites across `AccessControlGate` (L241, L249), `ClaimManager` (L80–L101) and `HeritVault`
(L176).

**Accepted.** This is the same family Slither reports as `reentrancy-no-eth`, triaged in
`slither.md` ID-4 and ID-5. In short:

- `ClaimManager.claim` writes `s_paid` at L99 **before** calling `payOut` at L101 — the effect
  precedes the interaction. It also carries `nonReentrant` and `onlyAttestor`.
- `AccessControlGate.openEstate` stores the registry address returned by `VerifiableFactory`,
  so the write cannot precede the call. The factory is `immutable` and is ENS's own contract.
- `HeritVault.depositERC20` (L176) measures balance before and after the transfer to handle
  fee-on-transfer tokens. Reading balance across the call is the point of that pattern.

Every function involved is `nonReentrant`. Aderyn flags the shape, not a reachable path.

---

## Low — worth acting on

### L-4 · `nonReentrant` is not the first modifier

- `AccessControlGate.unlockHeir` (L321) — `onlyHeritRegistry nonReentrant`
- `AccessControlGate.renewEstate` (L365)

Modifiers run in declaration order, so the reentrancy guard is currently set **after** the access
check has already run. Harmless today because neither `onlyHeritRegistry` nor the expiry check
makes an external call, but the ordering is only safe by accident — a future guard that reads
another contract would run unprotected.

Swapping the order is a one-word change per function and removes the dependency on that accident.

### L-7 · Unused error

Two errors are declared and never reverted:

| Error | Declared |
|---|---|
| `HeritRegistry__EstateNotFound(uint256 estateId)` | `HeritRegistry.sol:18` |
| `LivenessAttestor__InvalidSigner(address recovered)` | `LivenessAttestor.sol:15` |

Confirmed by grep — one occurrence each in `src/`, which is the declaration itself.

`LivenessAttestor__InvalidSigner` looks superseded: `_verify` reverts
`LivenessAttestor__InvalidSignature()` at L149 instead, which is the same condition minus the
recovered address. The richer error would make a signer mismatch far easier to debug — the
recovered address immediately distinguishes "wrong key" from "malformed signature". Either wire
it in at L149 or delete it.

`HeritRegistry__EstateNotFound` should be checked more carefully before deletion: an unused
"not found" error can mean the guard was dropped rather than never written. Worth confirming that
every path reaching estate state already fails for an unopened estate through some other check.

---

## Low — accepted

**L-1 · Costly operations inside loop.** Storage writes inside the per-token loops in
`ClaimManager.claim` and `HeritVault.snapshot`. Bounded by `MAX_TOKENS`, and each write is the
per-token record the loop exists to produce — hoisting is not possible.

**L-2 · Large numeric literal.** `ClaimManager.sol:26`, `HeritRegistry.sol:46`,
`HeritVault.sol:57` — `BPS_DENOMINATOR` and the timer bounds. Each is a named constant with a
NatSpec line; the literal is the value's definition.

**L-3 · Modifier invoked only once.** Single-use modifiers such as `onlyClaimManager` and
`onlyAttestor`. Kept because the name states the rule at the call site, which is worth more than
saving a modifier.

**L-5 · Loop contains `require`/`revert`.** `ClaimManager.claim` reverts
`ClaimManager__InvalidShare` inside the token loop. Deliberate: a share above 100% is corrupt
state, and continuing would pay out against it.

**L-6 · Unchecked return.** Five sites. `AccessControlGate.sol:259` (`register` returns a token
id — discarded on purpose, since token ids change on every grant/revoke) and
`AccessControlGate.sol:348` (`grantRoles` returns `bool`) are covered in `slither.md` under
ID-7/ID-8. The three `LivenessAttestor` sites (L105, L127, L131) forward to the registry and
claim manager, whose outcomes are reported by their own events.
