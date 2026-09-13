# Slither — static analysis of `src/`

```bash
slither . --filter-paths "lib/|test/|script/" --checklist \
  --markdown-root "https://github.com/Kelechikizito/herit/blob/<sha>/"
```

Slither 0.11.5 · solc 0.8.30 · 52 contracts, 101 detectors · **45 results**
Run against commit `a8f39f4e`. Raw checklist output is reproducible with the command above.

## Summary

| Detector | Count | Impact | Verdict |
|---|---|---|---|
| `arbitrary-send-eth` | 1 | High | False positive — both callers are access-controlled |
| `incorrect-equality` | 3 | Medium | False positive — enum and exact-value comparisons, not timestamps |
| `reentrancy-no-eth` | 2 | Medium | 1 false positive, 1 accepted (trusted factory) |
| `unused-return` | 3 | Medium | 1 worth acting on, 2 deliberate |
| `calls-loop` | 5 | Low | Accepted — bounded by `MAX_TOKENS` / `MAX_HEIRS` |
| `reentrancy-events` | 3 | Low | Accepted — event ordering only |
| `reentrancy-benign` | 1 | Low | Accepted |
| `timestamp` | 6 | Low | Accepted — the product is a timer |
| `low-level-calls` | 1 | Informational | Required for ETH transfer |
| `naming-convention` | 20 | Informational | Deliberate house style |

**Nothing requires a contract change.** One finding (`unused-return` on `grantRoles`) is worth a
decision rather than a dismissal.

---

## High

### ID-0 · `arbitrary-send-eth` — `HeritVault._send` (`HeritVault.sol:285-294`)

> sends eth to arbitrary user — `(ok,None) = address(to).call{value: amount}()`

**False positive.** `_send` is `internal` and has exactly two callers, both gated:

| Caller | Modifiers | Recipient |
|---|---|---|
| `withdraw` (L193) | `nonReentrant`, `onlyGrantorOf(estateId)`, `notUnlocked(estateId)` | `msg.sender` — the grantor, by definition |
| `payOut` (L228) | `nonReentrant`, `onlyClaimManager` | the heir `ClaimManager` resolved from ENS |

`to` is never caller-supplied on an unguarded path. Slither flags the pattern because `_send`
read in isolation takes an arbitrary address, which is what an internal helper should do.

---

## Medium

### ID-1, ID-2, ID-3 · `incorrect-equality`

- `HeritVault.depositERC20` — `credited == 0` (L180)
- `HeritRegistry.pokeExpiry` — `pending == stored` (L207)
- `HeritRegistry._pendingStatus` — `status == Status.Unlocked` (L381)

**False positives.** This detector targets strict equality against `block.timestamp` or balances,
where an off-by-one makes the branch unreachable. None of these are that. `credited == 0` is the
fee-on-transfer guard and zero is exactly the value that must be rejected; the other two compare
`Status` enum members, where equality is the only correct operator.

### ID-4 · `reentrancy-no-eth` — `ClaimManager.claim` (`ClaimManager.sol:73-110`)

> State variables written after the call(s): `s_paid[estateId][heirLabelhash][token] = true`

**False positive, and the line ordering is the opposite of what it reads like.** `s_paid` is set
at **L99**, `payOut` is called at **L101** — the effect precedes the interaction, which is
checks-effects-interactions done correctly:

```solidity
s_paid[estateId][heirLabelhash][token] = true;          // L99  effect
uint256 amount = I_VAULT.payOut(estateId, token, heir, uint16(bps));  // L101 interaction
```

The "after" in the report refers to the earlier `pokeExpiry` call at L80, not to `payOut`. On top
of that, `claim` carries `nonReentrant` and `onlyAttestor`, so the only caller is
`LivenessAttestor` holding a valid EIP-712 attestation.

### ID-5 · `reentrancy-no-eth` — `AccessControlGate.openEstate` (`AccessControlGate.sol:226-264`)

> `s_estateRegistries[estateId]` written after `I_VERIFIABLE_FACTORY.deployProxy(...)`

**Accepted.** The external call is to ENS's `VerifiableFactory`, held `immutable` and pinned to
the frozen hackathon deployment. Reordering is not possible — the address being stored is the
factory's return value. Residual risk is a compromised ENS factory, which is out of Herit's trust
boundary.

### ID-6, ID-7, ID-8 · `unused-return`

| ID | Call | Verdict |
|---|---|---|
| ID-6 | `I_CLAIM_MANAGER.claim(...)` returns `tokensPaid`, ignored by `LivenessAttestor` (L131) | Deliberate — the attestor's job ends at forwarding; the count is emitted by `ClaimSettled` |
| ID-7 | `registry.grantRoles(...)` returns `bool`, ignored in `unlockHeir` (L348) | **Worth a decision — see below** |
| ID-8 | `I_GRANTOR_REGISTRY.register(...)` returns a token id, ignored (L259) | Deliberate — `documents/deployments.md` records that token ids change on every grant/revoke via `_regenerate`, so caching one is a bug. Discarding it is correct. |

**On ID-7.** `EnhancedAccessControl.grantRoles` returns `bool` (whether roles changed). Checking
it would make `unlockHeir` revert instead of silently no-op if the grant did nothing.

The caveat that matters: it would **not** catch the failure we actually worry about. A role
granted on an expired name is written to `eacVersionId + 1`, a resource nothing reads, and that
path still returns `true`. Herit handles that case separately by renewing the heir subname before
granting. So checking the bool adds a cheap guard against an unforeseen no-op, not against the
known one. Left unchecked for the hackathon; noted here rather than dismissed.

---

## Accepted by design

**`timestamp` (6).** `block.timestamp` comparisons in `_pendingStatus`, `pokeExpiry`, `checkIn`
and the attestation expiry check. A dead-man's switch is a clock; miner drift of a few seconds is
irrelevant against intervals measured in days.

**`calls-loop` (5).** Loops in `ClaimManager.claim` and `HeritVault.snapshot` call the vault and
registry per token. Bounded by `MAX_TOKENS` and `MAX_HEIRS`, both enforced on the write path, so
the loop cannot be grown without limit by an attacker.

**`reentrancy-events` (3), `reentrancy-benign` (1).** Event emission ordering after external
calls. No state consequence.

**`low-level-calls` (1).** `address(to).call{value: amount}("")` in `_send` — the correct way to
transfer ETH to a contract wallet heir. Return value is checked and reverts
`HeritVault__EthTransferFailed`.

**`naming-convention` (20).** `I_SIGNER`, `I_HERIT_REGISTRY`, `s_estates` and similar. Immutables
in `SCREAMING_SNAKE` with an `I_` prefix and storage with `s_` is the house convention across
every contract, applied consistently. Not changing it to satisfy a linter.
