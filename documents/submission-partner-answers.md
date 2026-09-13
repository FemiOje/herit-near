# ETHOnline 2026 — partner prize answers

Paste-ready text for the "How are you using this Protocol / API?" table.
Permalinks pinned to `a8f39f4ed061ef335ad58fe517d44eb3750d8980`.

---

## World — $7,000

### Why you're applicable for this prize

```
Herit uses World ID Selfie Check twice, for two genuinely different jobs: a recurring liveness
proof that decides whether a grantor's estate stays locked, and a one-time personhood gate an
heir must pass before they can claim. It is the product, not a signup step. A dead-man's switch
normally treats "the key signed something" as "the owner is alive", which means the thief who
stole the key can keep the estate locked open indefinitely; Herit binds each estate to one
salted World ID nullifier on the first check-in, and every later check-in must match it, so a
stolen key is a different human and simply cannot check in. Selfie Check specifically, rather
than an Orb proof, because inheritance needs to know the owner is alive today and not merely
that they are a unique person.
```

### Link to the line of code where the tech is used

```
https://github.com/Kelechikizito/herit/blob/a8f39f4ed061ef335ad58fe517d44eb3750d8980/frontend/app/api/worldid/verify/route.ts#L189
```

Secondary links, if a second field allows one:

- RP request signed server-side — `frontend/app/api/worldid/sign/route.ts#L87`
- The widget and the `selfieCheckLegacy` preset — `frontend/components/selfie-check/selfie-check-modal.tsx#L223`
- The contract that verifies the attestation — `src/LivenessAttestor.sol#L149`

### How easy is it to use? (1–10)

**6** — the SDK works and the happy path is short, but three separate things cost hours each and
none of them were visible from the docs. See the feedback below.

### Additional feedback for the sponsor

```
Six things, in the order they cost us time.

1. The action string is the nullifier namespace, and nothing says so where you choose it. Both
facts are documented, but not in the same place, and the naming instinct is to be granular and
descriptive — which is exactly backwards. Our first design used claim:{estate}:{heir}, which
reads correctly and is wrong: one person registered as two heirs of the same estate gets two
different nullifiers, two different commitments, and is paid twice, with every individual proof
valid and every test passing. Please state it as a rule at the point of choice, in the Portal's
create-action form and on the action prop: one action is one uniqueness namespace, so it should
be as coarse as the thing you want unique per human, and anything varying per attempt belongs in
the signal. The signal is the right escape hatch and is under-sold — it reads as a way to bind a
proof to a transaction, not as the answer to "I need per-item context without per-item
nullifiers".

2. The v4 verify endpoint's success response shape is not documented anywhere we could find.
Every integrator has to write a branch on "did this verify", and we shipped a predicate that was
an explicit guess, written to fail closed so a wrong guess rejects good proofs rather than
accepting bad ones. This is the single highest-value doc fix on the list. Relatedly, we lost an
evening to reading the nullifier out of that response when it actually lives in the IDKit result
we had forwarded — worth a sentence saying so.

3. Selfie Check has no on-chain verifier, and that constraint is not stated where Selfie Check is
sold. We found it at architecture time, not evaluation time. It forced an entire contract
(LivenessAttestor), a replay scheme, and a signing key we must hold for the life of the protocol
— the top centralization risk in our own threat model. Note that the World ID Router IS deployed
on Ethereum Sepolia beside the ENSv2 set and we verified it live, so "no chain has both" was not
our blocker; the blocker was that on-chain verification is groupId 1, Orb only. A per-credential
table of on-chain vs cloud-only would have saved the whole detour. Longer term, a World-signed
attestation over a Cloud Verify result with a published verifier contract would stop every
integrator writing the same contract with their own key custody.

4. The documented replay rule breaks recurring liveness. Guidance everywhere is UNIQUE(action,
nullifier), reject duplicates. That is right for the airdrop case and wrong for a liveness
credential, which is inherently recurring: the same human produces the same nullifier every time
by design. Applied literally it accepts a grantor's first check-in and locks them out forever
after, which for us is the maximally destructive failure, reached by following the docs
correctly. Please split the guidance by shape and give the recurring case a worked example —
bind on first sight, compare on every later one, which detects substitution rather than merely
counting.

5. signRequest's output does not fit rp_context's input. Four of five field names differ and
rp_id is not in the output at all. Because it crosses a network boundary as JSON, nothing
type-checks it: passing the object straight through compiles, serializes, and hands the widget
correct values under wrong keys. Either return the RpContext shape minus rp_id, or export the
remap.

6. Portal state failures are indistinguishable from application bugs. app_mode being mini-app
rather than external, the Selfie Check feature flag being off, and an action registered in one
environment but not the other all surface as "the widget produced nothing". A read endpoint
returning app_mode, enabled credentials, and whether an action exists in an environment would
turn all three into a startup assertion with a precise message. Failing that, sort the error
codes in the docs by remedy rather than cause — credential_unavailable and user_rejected are
equally "failed" to the code and completely different to the operator.

Selfie Check is the right primitive for this product and no other credential substitutes for it.
Everything above is integration cost around a credential that works.
```

---

## ENS — $5,000

### Why you're applicable for this prize

```
In Herit an heir is not an address in an array, it is a real ENSv2 subname holding a real role.
Each estate gets its own PermissionedRegistry deployed through the VerifiableFactory, heirs are
minted as subnames beneath the grantor's name with their relationship and share written as
public text records, and the claim permission is a custom role Herit claims in ENS's own
Enhanced Access Control bitmap — nybble 10, in the free range alongside ENS's own roles rather
than in a parallel system. Unlocking an estate is literally grantRoles(heirLabel,
ROLE_HEIR_CLAIM, heir) on the deployed registry, and the claim gate is hasRoles. There is no
mirrored boolean in Herit's storage; anyone can read an heir's authority straight out of ENS
without touching our contracts. Because hasRoles requires the whole bitmap, checking
ROLE_HEIR_CLAIM | ROLE_HEIR_REGISTERED asks "has this estate unlocked for you" and "did Herit
mint this subname at all" in one call. That is what lets an heir be shown their inheritance, and
read its terms, for years before they are able to act on it — something an address array cannot
express.
```

### Link to the line of code where the tech is used

```
https://github.com/Kelechikizito/herit/blob/a8f39f4ed061ef335ad58fe517d44eb3750d8980/src/AccessControlGate.sol#L348
```

Secondary links, if a second field allows one:

- The role bitmap claiming nybbles 10 and 11 — `src/libraries/HeritRolesLib.sol#L14`
- Per-estate registry deployed via `VerifiableFactory`, with the CREATE2 address predicted
  off-chain — `src/AccessControlGate.sol` (`openEstate` / `predictEstateRegistry`)
- Fork tests against the live Sepolia registry — `test/integration/HeritLifecycleTest.t.sol`

### How easy is it to use? (1–10)

**5** — the primitives are excellent and genuinely enabled the product, but ENSv2 is beta and
several failure modes are silent rather than reverting. Detail below.

### Additional feedback for the sponsor

```
Enhanced Access Control is the reason this project exists in the shape it does. Roles on a
subname, admin counterparts, and hasRoles requiring the whole bitmap let us express "this
inheritance exists and is visible but is not exercisable yet" natively, with no shadow state.
Nothing else we looked at can express that. The feedback below is all about discoverability, not
design.

1. The deployed hackathon contracts are ahead of the public repo, and one signature differs in
the place that matters. The deployed UserRegistry takes initialize as an array of (account,
roleBitmap) pairs, selector 0x37cb53a8, while the pinned submodule declares
initialize(address,uint256), selector 0xcd6dc687. Importing the submodule's interface compiles,
deploys, and reverts on Sepolia with nothing in the error explaining why. We only found it by
scanning every selector we call against the deployed bytecode. Please publish the exact commit
the frozen set was built from, or a note listing the signatures that moved.

2. Granting a role on an expired name silently does nothing. It writes to eacVersionId + 1, a
resource id nothing reads, so the transaction succeeds and the permission does not exist. For us
that would have meant an unlock that appeared to work and left heirs unable to claim. This is
the highest-severity item here: it should revert, or at minimum be called out in bold in the EAC
docs. We handle it by renewing the heir subname up to the estate's expiry before granting
anything, which is also why our gate needs ROLE_RENEW.

3. Every grant and revoke burns and re-mints the subname token via _regenerate, so a cached token
id is a bug waiting to fire. We address names by labelhash throughout because of it. Worth a
prominent warning — caching a token id is the natural thing to do.

4. Renewing an already-expired name takes the _canRevive path, which needs root ROLE_RENEW on
that specific registry. A registry you did not deploy will not have granted it to you, so a
lapsed name is unrecoverable and renew reverts for every caller until someone notices. Easy to
miss when writing the deploy script.

5. A child's expiry is not capped against its parent's. We enforce it ourselves in registerHeir,
but it is surprising that the registry allows a subname to outlive the name it hangs from.

6. Reader-facing inconsistency on the deployed PermissionedResolver and registry: getOwner takes
a uint256 token id while getResolver and getSubregistry take a string label, and the deployed
resolver is name-based, so calling text(bytes32, string) directly reverts and records have to go
through resolve(dnsEncodedName, innerCalldata). Each is defensible alone; together they produce
bare "execution reverted" with no hint, several times over. A short "how to read a record off
this deployment" snippet in the docs would fix it.

7. Names on the frozen hackathon set do not appear in the official ENS app, which indexes the
beta set. This is expected once you know it and looks exactly like a failed registration until
you do — we went and checked whether our root name had actually registered. A line on the
hackathon banner saying so would save every team the same scare.

One thing that worked very well and deserves saying: VerifiableFactory. Deterministic per-estate
registries with verifiable provenance and no bespoke proxy code to audit is exactly the right
primitive for this, and predicting the address off-chain before the user pays let us show them
their estate's registry address before they commit to anything.
```

---

## Notes

- Star ratings are a judgement call — **6 for World, 5 for ENS** is what the build actually felt
  like, and both are paired with specific, fixable reasons rather than a vague complaint. Adjust
  if you'd rather be softer; the written feedback carries the substance either way.
- The full, longer World write-up lives in
  `documents/research/feedback/worldid-selfie-check-feedback.md` if a sponsor asks for detail.
