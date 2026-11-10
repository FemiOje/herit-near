import { randomBytes } from "node:crypto";
import { KeyPair } from "near-api-js";

import { AttestError, attest, parseRequest, type Host } from "@/lib/attest";

/**
 * POST /api/attest
 *
 * Verifies a Selfie Check proof with World, then signs the attestation the estate contract checks
 * against `attestor_pk`. The testnet path: it signs with a local key from `.env.local`, as the
 * Sepolia app did. On mainnet the OutLayer worker (near/attestor-worker) does this inside a TEE
 * with the same rules, so nobody holds the key. Checkpoint 5, fallback path.
 *
 * The browser sends the transaction, never this route: the key holds no NEAR and can only assert
 * that a check passed.
 *
 * in:  { purpose: SelfieCheckPurpose, subject, result }
 * out: { attestation, signature }, the estate's `check_in` / `claim` arguments
 */

const VERIFY_BASE = "https://developer.world.org/api/v4/verify";

class ConfigError extends Error {}

export async function POST(request: Request) {
  try {
    const req = parseRequest(await request.json().catch(() => null));
    const out = await attest(req, host());
    if (process.env.NODE_ENV !== "production") {
      // Public once sent on chain anyway; lets a dev script pick it up from the log.
      console.log(`[attest] signed ${req.purpose.estateLabel} ${req.subject}: ${JSON.stringify(out)}`);
    }
    return Response.json(out);
  } catch (error) {
    if (error instanceof AttestError) {
      // Logged as well as returned: IDKit can swallow the body and report a bare error code.
      console.error(`[attest] refused: ${error.message}`);
      return Response.json({ error: error.message }, { status: 400 });
    }
    if (error instanceof ConfigError) {
      console.error(`[attest] configuration: ${error.message}`);
      return Response.json({ error: "the server is not configured to attest" }, { status: 500 });
    }
    // Name only: anything thrown from the signer may quote its key.
    console.error(`[attest] failed: ${error instanceof Error ? error.name : "unknown"}`);
    return Response.json({ error: "could not issue an attestation" }, { status: 500 });
  }
}

function host(): Host {
  const rpId = required("WORLD_RP_ID");
  const secretKey = required("ATTESTOR_SECRET_KEY");
  const saltHex = required("COMMITMENT_SALT");

  // Shapes checked here so a malformed value is our own error, not a library's that might quote it.
  if (!secretKey.startsWith("ed25519:")) {
    throw new ConfigError("ATTESTOR_SECRET_KEY must be an ed25519:... secret key");
  }
  if (!/^[0-9a-fA-F]{64}$/.test(saltHex)) {
    throw new ConfigError("COMMITMENT_SALT must be 32 hex bytes");
  }
  let keyPair: KeyPair;
  try {
    keyPair = KeyPair.fromString(secretKey as `ed25519:${string}`);
  } catch {
    throw new ConfigError("ATTESTOR_SECRET_KEY could not be parsed");
  }

  return {
    async verifyWithWorld(body) {
      const response = await fetch(`${VERIFY_BASE}/${rpId}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body,
      });
      const reply: unknown = await response.json().catch(() => null);
      if (process.env.NODE_ENV !== "production") {
        // Its success field is undocumented: Checkpoint 5 records one real reply to tighten
        // `checkVerified`. It holds no secrets.
        console.log(`[attest] world reply ${response.status}: ${JSON.stringify(reply)}`);
      }
      return { status: response.status, reply };
    },
    sign: (message) => keyPair.sign(message).signature,
    salt: Buffer.from(saltHex, "hex"),
    nowNs: () => BigInt(Date.now()) * 1_000_000n,
    // Random, never a counter: nonces are only unique per estate, but a counter shared across
    // estates would still be guessable.
    randomNonce: () => randomBytes(8).readBigUInt64LE(),
  };
}

function required(key: string): string {
  const value = process.env[key];
  if (!value) throw new ConfigError(`${key} is unset`);
  return value;
}
