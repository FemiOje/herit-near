# Checkpoint 5: OutLayer attestor and Selfie Check (spike)

Part of the [build plan](../near-build-plan.md). The worker code, publishing steps and pass criteria are in [the deployment guide](../near-deployment-guide.md), §7.1–§7.5.

**Status:** not started
**Who:** Claude writes the worker, its tests and the dev pages; you do the parts that need your wallet, your World developer account and your phone
**Time box:** 3 hours. If it isn't passing by then, take the fallback below rather than cutting anything.

## Goal

Prove the riskiest piece still unbuilt: a real **World ID Selfie Check** on your phone produces an attestation, signed inside an OutLayer TEE, that the estate contract from Checkpoint 4 accepts. Nobody, including us, holds the attestor's private key.

The contract doesn't change in this checkpoint. It already checks one ed25519 key (`attestor_pk`), and only the place where the signature is made changes.

## Understand first

| Sepolia Herit | Here |
|---|---|
| The Next.js route verifies the proof with World, then signs with `ATTESTOR_PRIVATE_KEY` from its environment | The route forwards the proof to an **OutLayer worker**. The worker verifies it with World and signs with a key that exists only inside the TEE |
| `NULLIFIER_SALT` env var | A second TEE key signs a fixed message, and that signature is the salt. Nobody can read or swap it |
| EIP-712 signature | ed25519 over the Borsh `SignedPayload`, the same bytes Checkpoint 4's golden test pins |

- **Selfie Check is requested by the app and enforced by the worker.** The app asks the World App for Selfie Check only, with IDKit's `selfieCheckLegacy` preset, as the Sepolia app did. The worker then refuses any proof that isn't exactly one Selfie Check credential.
- **Why "exactly one".** World's v4 verify reply doesn't list nullifiers per credential; the Sepolia route found this. So the nullifier is read from the proof itself, which is only safe if the proof World just verified *is* the Selfie Check. A proof carrying two credentials could otherwise pass on the other one. Requiring a single response, with identifier `selfie` (or its legacy alias `face`), closes that gap whatever the reply format turns out to be.
- **`caller: "none"` keys stay the same for every caller.** That is what lets one `attestor_pk` work for every estate. Renaming a key's path, or deleting and recreating the OutLayer project, changes the key.
- **The worker never reads its environment.** With a `caller: "none"` key the caller can supply environment variables, so a salt from the environment could be swapped.

## Write (Claude)

1. **`near/attestor-worker/`** from guide §7.1, outside the contracts workspace because it targets `wasm32-wasip2`. **One change from the guide:** `verify_with_world` requires `result.responses` to hold exactly one item, with identifier `selfie` or `face`, and refuses anything else with a message naming what it found.
2. **Worker tests,** run natively with `cargo test`. The World call and the TEE keys are behind a small seam so the logic can be tested without them:
   - **Golden:** the worker's `SignedPayload` encodes to the same hex as `herit-estate/src/golden.rs`.
   - **Selfie-only:** a proof with only a Selfie Check passes; no responses, a different credential, or Selfie Check plus another credential is refused.
   - **Input rules:** a bad action, a label on a non-claim, a missing label on a claim, and an estate outside `.herit-near.testnet` are refused. The World action is always derived from `estate_id`.
3. **Frontend dev route and page:** port `app/api/worldid/sign` from `main`, which signs the World ID request with your `RP_SIGNING_KEY`. Add a dev-only page that runs IDKit with `selfieCheckLegacy` and shows the raw result to copy.
4. **`near/scripts/spike-check-in.sh`:** sends a result to the worker, then submits the returned attestation as `check_in` on a test estate.
5. **`frontend/scripts/sign-attestation.mjs`** from guide §7.5, the dev signer and fallback, with a test asserting the same golden hex.

## Do (you)

1. **World developer portal:** confirm the app from Sepolia still exists, and give Claude the public `WLD_RP_ID`. Put `RP_SIGNING_KEY` in `frontend/.env.local` yourself; it never goes in git or in chat.
2. **OutLayer:** sign in to the dashboard with a testnet wallet, create the project `herit-attestor` from the wasm Claude builds, and try to fund a testnet Payment Key.
3. **Test estate:** deploy `herit-estate` to a throwaway account, for example `spike-kaykay.testnet`, and call `new` with `grantor` = `alice-kaykay.testnet` and the worker's `attestor_pk`. Claude gives you the exact CLI commands.
4. **Selfie Check:** open the dev page, do a real Selfie Check in the World App, and run the spike script with the result.

## Pass criteria (guide §7.4)

- [ ] A Payment Key can be funded for `testnet-api.outlayer.ai`. **If not:** a funded relay account calls `request_execution` on `outlayer.testnet` from the route. The key stays the same, so nothing else changes.
- [ ] The worker builds for `wasm32-wasip2` and the OutLayer project accepts it.
- [ ] `public_key` returns the same key on two calls.
- [ ] One real Selfie Check returns an attestation, and the test estate accepts it as a `check_in`.
- [ ] A proof without a Selfie Check credential is refused by the live worker.

Save one real World verify reply (it holds no secrets) in `herit-docs/`. Then tighten the worker's success check to the field that reply actually uses, because the format is undocumented.

## Fallback (guide §7.5)

If the time box runs out, sign in the Next.js route with a local dev key instead. The contract stays as it is. Record which path was taken and why in the build plan. The dev signer exists either way, because the CLI smoke test needs it.

## Check

```bash
cd near/attestor-worker && cargo test && cargo build --target wasm32-wasip2 --release
cd ../.. && cd frontend && node --test scripts/
```

## Verified at the stop

- [ ] Go or fallback recorded in the build plan
- [ ] The worker refuses any proof that isn't exactly one Selfie Check
- [ ] Worker golden, selfie-only and input tests pass, and the JS signer's golden test passes
- [ ] A real Selfie Check produced a `check_in` the contract accepted (go path), with the transaction link recorded
- [ ] No secrets in git: `RP_SIGNING_KEY`, the Payment Key and any dev key stay in `.env.local`
