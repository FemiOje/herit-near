// DEV ONLY. Mints test attestations with a local key, for the CLI smoke test and as the fallback
// if OutLayer fails (guide §7.5). Never use this key for real estates' Selfie Checks.
//
//   ATTESTOR_SECRET_KEY=ed25519:... node scripts/sign-attestation.mjs \
//     --estate <id> --subject <id> --action 0|1|2 --human <name> [--label son]
//
// Generate a key once with:
//   node -e 'const {KeyPair}=require("near-api-js");const k=KeyPair.fromRandom("ed25519");console.log(k.toString());console.log(k.getPublicKey().toString())'
import { createHash, randomBytes } from "node:crypto";
import { parseArgs } from "node:util";
import { KeyPair } from "near-api-js";

import { encodePayload } from "./attestation-payload.mjs";

const { values: a } = parseArgs({
  options: {
    estate: { type: "string" },
    subject: { type: "string" },
    action: { type: "string" },
    human: { type: "string" },
    label: { type: "string" },
  },
});
for (const name of ["estate", "subject", "action", "human"]) {
  if (!a[name]) throw new Error(`--${name} is required`);
}
if (!process.env.ATTESTOR_SECRET_KEY) throw new Error("ATTESTOR_SECRET_KEY is unset");

// A different --human gives a different commitment, standing in for a different face.
const commitment = createHash("sha256").update(`dev-human:${a.human}`).digest();
const nonce = randomBytes(8).readBigUInt64LE();
const expiry_ns = BigInt(Date.now() + 15 * 60_000) * 1_000_000n; // under the 30-minute cap
const action = Number(a.action);
const heir_label = a.label ?? null;

const bytes = encodePayload({
  estate_id: a.estate,
  subject: a.subject,
  action,
  heir_label,
  commitment,
  nonce,
  expiry_ns,
});
const { signature } = KeyPair.fromString(process.env.ATTESTOR_SECRET_KEY).sign(bytes);

console.log(
  JSON.stringify({
    attestation: {
      action,
      heir_label,
      commitment: commitment.toString("base64"),
      nonce: nonce.toString(),
      expiry_ns: expiry_ns.toString(),
    },
    signature: Buffer.from(signature).toString("base64"),
  }),
);
