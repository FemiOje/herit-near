# Checkpoint 5: Selfie Check attestor (OutLayer spike, testnet fallback)

Part of the [build plan](../near-build-plan.md). The worker code, publishing steps and pass criteria are in [the deployment guide](../near-deployment-guide.md), §7.1–§7.5.

**Status:** Passed on 2026-11-10 on the fallback path: testnet signs in the Next.js route with a local key, and a real sandbox Selfie Check landed a `check_in` on `spike.herit-near.testnet`. The OutLayer worker is built and tested, and waits for mainnet.
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
- **Only World ID 3.0 proofs.** The `selfieCheckLegacy` preset only returns 3.0 proofs, and a person's 3.0 and 4.0 nullifiers differ. Accepting both would give one human two commitments and two heir slots, so the worker refuses anything but `protocol_version: "3.0"`.
- **The proof must name our action.** Nullifiers are per action, so a proof for any other action would give the same human a fresh commitment. The worker refuses a proof whose `action` is missing or isn't `herit:<estate label>`. On 3.0 results IDKit marks `action` optional; the spike confirms real results carry it.
- **Selfie Check is in preview at World** ("Contact us if you need it enabled"). The app must have it switched on.
- **We test in World's sandbox** (`WORLD_ENVIRONMENT=sandbox` in `frontend/.env.local`), using the World sandbox testing app on your iPhone. The IDKit result carries its own `environment`, and the worker forwards the whole result to World's verify API, so a sandbox proof is checked against sandbox with no worker change. Sandbox identities aren't real humans, and their nullifiers differ from production ones. Before mainnet, decide whether the worker should require `environment: "production"`.

## Write (Claude)

1. **`near/attestor-worker/`** from guide §7.1, outside the contracts workspace because it targets `wasm32-wasip2`. **One change from the guide:** `verify_with_world` requires `result.responses` to hold exactly one item, with identifier `selfie` or `face`, and refuses anything else with a message naming what it found.
2. **Worker tests,** run natively with `cargo test`. The World call and the TEE keys are behind a small seam so the logic can be tested without them:
   - **Golden:** the worker's `SignedPayload` encodes to the same hex as `herit-estate/src/golden.rs`.
   - **Selfie-only:** a proof with only a Selfie Check passes; no responses, a different credential, or Selfie Check plus another credential is refused.
   - **Input rules:** a bad action, a label on a non-claim, a missing label on a claim, and an estate outside `.herit-near.testnet` are refused. The World action is always derived from `estate_id`.
3. **Frontend dev route and page:** port `app/api/worldid/sign` from `main`, which signs the World ID request with your `RP_SIGNING_KEY`. Add a dev-only page that runs IDKit with `selfieCheckLegacy` and shows the raw result to copy.
4. **`near/scripts/spike-check-in.sh`:** sends a result to the worker, then submits the returned attestation as `check_in` on a test estate.
5. **`frontend/scripts/sign-attestation.mjs`** from guide §7.5, the dev signer and fallback, with a test asserting the same golden hex.

### Built (Claude, 2026-11-10)

| Piece | Where | Tests |
|---|---|---|
| Worker | `near/attestor-worker/` (`src/logic.rs` decides, `src/main.rs` talks to World and the TEE) | 18 native tests in `src/logic/tests.rs`, including one where the real estate contract accepts the worker's attestation |
| Signing route | `frontend/app/api/worldid/sign/route.ts`, reading `WORLD_APP_ID`, `WORLD_RP_ID`, `WORLD_SIGNING_KEY`, `WORLD_ENVIRONMENT` | Run against your real config |
| Dev page | `frontend/app/dev/selfie/` at `/dev/selfie`, not served in production | Renders |
| Dev signer | `frontend/scripts/sign-attestation.mjs`, encoder in `attestation-payload.mjs` | `pnpm test`: golden bytes; a real run verified |
| Spike script (OutLayer path) | `near/scripts/spike-check-in.sh` | Syntax checked |
| **Testnet attestor (fallback)** | `frontend/app/api/attest/route.ts`, rules in `frontend/lib/attest.ts`, the same as the worker's | 12 tests in `lib/attest.test.ts` (`pnpm test`), including a real ed25519 signature over the golden-tested payload; route smoke-tested against World |

The worker shares `near/target` (set in `near/attestor-worker/.cargo/config.toml`), so the NEAR SDK isn't compiled twice. The built wasm is `near/target/wasm32-wasip2/release/herit-attestor.wasm`, with the manifest in its `outlayer.manifest` custom section.

## Decision: fallback on testnet (2026-11-10)

You chose to sign on testnet in the Next.js route, as the Sepolia app did, and to use OutLayer only if Herit goes to mainnet. That means no OutLayer project, Payment Key or wasm hosting for now.

- **What it costs:** whoever holds `ATTESTOR_SECRET_KEY` (in `frontend/.env.local`) can forge check-ins and claims on testnet estates. That is acceptable on testnet, and the README should say so.
- **What stays the same:** the contract, the payload and the Selfie Check rules. `lib/attest.ts` applies the worker's rules line for line, and the worker stays in the repo for mainnet.
- **What changes for mainnet:** new estates get the worker's `attestor_pk`. The salts differ, so commitments made on testnet don't carry over, and they don't need to.
- **Config** in `frontend/.env.local`, generated by Claude without printing the secrets: `ATTESTOR_SECRET_KEY` and `COMMITMENT_SALT`. The public key is `ed25519:FzTUQSiLeJji46M3GXzoiBntYV5wbnomrF4ywomBJwfe`.

## Done (2026-11-10)

Claude built the wasm, created and deployed `spike.herit-near.testnet` ([create](https://explorer.testnet.near.org/transactions/AJCZ5DJSdUTAQS5Bpi2gJFpzeqaTyGDZGnGxyzdFeikJ), [deploy](https://explorer.testnet.near.org/transactions/9mB3sFrR9GYpprQdSvuw6AcpHkLxoVe21NuSeBd1T4ea)) and sent the `check_in`. You did the Selfie Check in the sandbox app. The steps, to repeat them on another estate:


1. **World:** in the developer portal, check that Selfie Check is enabled for `app_ea26c04b8c94d8053f2b9277f1caae83`. Keep `WORLD_ENVIRONMENT=sandbox` and do the check in the World sandbox testing app on your iPhone.
2. **Build the estate wasm** (skip if `near/target/near/herit_estate/herit_estate.wasm` is fresh):
   ```bash
   cd near/contracts/herit-estate && cargo near build non-reproducible-wasm && cd ../../..
   ```
3. **Test estate** `spike.herit-near.testnet`, with the route's key as `attestor_pk`:
   ```bash
   near account create-account fund-myself spike.herit-near.testnet '2 NEAR' autogenerate-new-keypair \
     save-to-keychain sign-as herit-near.testnet network-config testnet sign-with-keychain send
   near contract deploy spike.herit-near.testnet use-file near/target/near/herit_estate/herit_estate.wasm \
     with-init-call new json-args '{"grantor":"alice-kaykay.testnet","attestor_pk":"ed25519:FzTUQSiLeJji46M3GXzoiBntYV5wbnomrF4ywomBJwfe","check_in_interval_sec":600,"grace_sec":300}' \
     prepaid-gas '100 Tgas' attached-deposit '0 NEAR' network-config testnet sign-with-keychain send
   ```
4. **Selfie Check:** run `pnpm dev` in `frontend/`, open `http://localhost:3000/dev/selfie`, keep estate `spike` and subject `alice-kaykay.testnet`, and do the check in the sandbox app. The page shows a `near contract call-function ... check_in ...` command: run it within 15 minutes. Then check the estate:
   ```bash
   near contract call-function as-read-only spike.herit-near.testnet commitment_of json-args '{}' network-config testnet now
   ```
5. **Send me** the transaction link, and the `[attest] world reply ...` line from the `pnpm dev` terminal. It holds no secrets, and I'll use it to tighten the success check.

## Pass criteria

- [x] One real sandbox Selfie Check returns an attestation, and `spike.herit-near.testnet` accepts it as a `check_in`; `commitment_of` is now set. [Transaction `4MR22kUJ…`](https://explorer.testnet.near.org/transactions/4MR22kUJqDaHhCgpNARGXTENg3JuNW2byAwEynsyxiHS), 2026-11-10.
- [x] A second Selfie Check by the same person gives the same commitment, so a second `check_in` is accepted. [Transaction `8n7WT6uB…`](https://explorer.testnet.near.org/transactions/8n7WT6uBJR1n7SMxT7jXLH6tckqhuMpxHk3ic5YExuGT), with the tightened check live. World's reply was the same shape, with `message` "Proof verified successfully (nullifier reuse)" and the first check's `created_at`: World allows a nullifier to be verified again for the same action, which recurring check-ins rely on. Replay is the contract's job, through its nonces.
- [x] A proof without a Selfie Check credential is refused (tests, and the route against World).
- [x] One real World verify reply saved in [`herit-docs/world-verify-reply.json`](../world-verify-reply.json), nullifiers redacted. `checkVerified` and the worker's `check_verified` now require that whole shape: `success: true`, the estate's `action`, `protocol_version` 3.0, exactly one `selfie` entry in `results` with `success: true`, and the proof's nullifier at both `nullifier` and `results[0].nullifier`.

## OutLayer, for mainnet (guide §7.1–§7.4)

Not done now. When Herit goes to mainnet: host `herit-attestor.wasm` at a URL with its SHA-256, create the OutLayer project, fund a Payment Key, read `public_key` twice to check it doesn't change, and deploy new estates with that key. `near/scripts/spike-check-in.sh` runs the worker end to end.

## Check

```bash
cd near/attestor-worker && cargo test && cargo clippy --all-targets -- -D warnings \
  && cargo build --target wasm32-wasip2 --release
cd ../../frontend && pnpm test && pnpm exec tsc --noEmit && pnpm lint
```

## Verified at the stop

- [x] Go or fallback recorded in the build plan (fallback on testnet, OutLayer for mainnet)
- [x] The attestor refuses any proof that isn't exactly one World ID 3.0 Selfie Check for this estate's action, in both the route and the worker
- [x] Worker tests, route tests and the JS signer's golden test pass
- [x] A real Selfie Check produced a `check_in` the contract accepted, with the transaction link recorded
- [x] No secrets in git: `WORLD_SIGNING_KEY`, `ATTESTOR_SECRET_KEY` and `COMMITMENT_SALT` stay in `.env.local`
