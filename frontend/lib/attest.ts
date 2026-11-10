import { createHash } from "node:crypto";

import { encodePayload } from "../scripts/attestation-payload.mjs";
import { estateAccountId, isLabel, worldAction, type SelfieCheckPurpose } from "./selfie-check.ts";

/**
 * Everything the attestor decides, for `POST /api/attest`: the testnet path, which signs with a
 * local key (Checkpoint 5 fallback, guide §7.5). The same rules as the OutLayer worker in
 * near/attestor-worker/src/logic.rs, which is the mainnet path. Keep the two in step.
 *
 * Kept apart from World's HTTP API and the key so it can be tested; the route plugs in the real
 * ones. Imports use relative `.ts` paths so `node --test` can run it directly.
 */

/** Under the contract's 30-minute cap. */
export const TTL_NS = 15n * 60n * 1_000_000_000n;
export const ACTION_CHECKIN = 0;
export const ACTION_CLAIM = 1;
/** Selfie Check's credential identifier in an IDKit result. `face` is its legacy alias. */
const SELFIE_IDENTIFIERS = ["selfie", "face"];
/** The only World ID protocol version accepted. See `selfieNullifier`. */
const PROTOCOL_VERSION = "3.0";

/** A refusal whose message describes the request or the proof, never a key. Safe to return. */
export class AttestError extends Error {}

export type AttestRequest = {
  purpose: SelfieCheckPurpose;
  /**
   * The NEAR account that will send the transaction: the grantor for a check-in, the heir's
   * sub-account for a claim. The contract checks who may call, so a wrong subject only produces an
   * attestation nobody can use.
   */
  subject: string;
  /** The IDKit result, forwarded to World untouched. */
  result: unknown;
};

/** What the estate's `check_in` and `claim` take as arguments. */
export type AttestResponse = {
  attestation: {
    action: number;
    heir_label: string | null;
    commitment: string;
    nonce: string;
    expiry_ns: string;
  };
  signature: string;
};

export type Host = {
  /** POSTs `body` to World's verify endpoint. `reply` is null if it wasn't JSON. */
  verifyWithWorld(body: string): Promise<{ status: number; reply: unknown }>;
  /** Signs with the attestor key. */
  sign(message: Uint8Array): Uint8Array;
  /** 32 secret bytes mixed into every commitment. */
  salt: Uint8Array;
  nowNs(): bigint;
  randomNonce(): bigint;
};

type Json = Record<string, unknown>;
const asObject = (v: unknown): Json => (typeof v === "object" && v !== null ? (v as Json) : {});

/** Checks the request's shape. Throws `AttestError` with a message safe to return. */
export function parseRequest(body: unknown): AttestRequest {
  const { purpose, subject, result } = asObject(body);
  const { kind, estateLabel, heirLabel } = asObject(purpose);
  if (!isLabel(estateLabel)) throw new AttestError("estateLabel must be a single lowercase segment");
  if (typeof subject !== "string" || !/^[a-z0-9._-]{2,64}$/.test(subject)) {
    throw new AttestError("subject must be the NEAR account that will send the transaction");
  }
  if (typeof result !== "object" || result === null) throw new AttestError("result is required");
  if (kind === "checkin") return { purpose: { kind, estateLabel }, subject, result };
  if (kind === "claim") {
    if (!isLabel(heirLabel)) throw new AttestError("heirLabel must be a single lowercase segment");
    return { purpose: { kind, estateLabel, heirLabel }, subject, result };
  }
  throw new AttestError("kind must be 'checkin' or 'claim'");
}

/**
 * Returns the Selfie Check nullifier, and refuses any proof that isn't exactly one Selfie Check.
 *
 * Checked on the proof before calling World, so a wrong credential costs nothing, and again on
 * World's reply in `checkVerified`. A proof with a second credential could otherwise pass on that
 * one.
 *
 * Only World ID 3.0 proofs, which is all `selfieCheckLegacy` returns. A person's 3.0 and 4.0
 * nullifiers differ, so accepting both would give one human two commitments.
 */
export function selfieNullifier(result: unknown): string {
  const r = asObject(result);
  if (r.protocol_version !== PROTOCOL_VERSION) {
    const got = typeof r.protocol_version === "string" ? r.protocol_version : "none";
    throw new AttestError(`proof must be World ID ${PROTOCOL_VERSION} (got: ${got})`);
  }
  if (!Array.isArray(r.responses)) throw new AttestError("proof has no responses");
  const identifiers = r.responses.map((item) => {
    const id = asObject(item).identifier;
    return typeof id === "string" ? id : "?";
  });
  if (r.responses.length !== 1) {
    throw new AttestError(
      `proof must hold exactly one Selfie Check credential (got: ${identifiers.join(", ") || "none"})`,
    );
  }
  if (!SELFIE_IDENTIFIERS.includes(identifiers[0])) {
    throw new AttestError(`proof must be a Selfie Check (got: ${identifiers[0]})`);
  }
  const nullifier = asObject(r.responses[0]).nullifier;
  if (typeof nullifier !== "string" || nullifier.trim() === "") {
    throw new AttestError("Selfie Check response has no nullifier");
  }
  return nullifier.trim().toLowerCase();
}

/**
 * Checks World's verify reply, and fails closed: anything that isn't exactly the success shape is
 * a rejection.
 *
 * The shape is the real reply recorded in herit-docs/world-verify-reply.json (Checkpoint 5):
 * `success`, `action`, `protocol_version`, `nullifier`, and `results` with one entry per
 * credential. Every field is required, and the reply's nullifier must be the one in the proof, so
 * the commitment comes from what World verified.
 */
export function checkVerified(
  status: number,
  reply: unknown,
  result: unknown,
  action: string,
  nullifier: string,
) {
  const body = asObject(reply);
  if (status < 200 || status >= 300 || body.success !== true) {
    const detail =
      typeof body.detail === "string"
        ? body.detail
        : typeof body.code === "string"
          ? body.code
          : "no detail";
    throw new AttestError(`proof rejected (status ${status}: ${detail})`);
  }
  // Nullifiers are per action, so a proof for any other action would give the same human a fresh
  // commitment and a second heir slot.
  if (asObject(result).action !== action || body.action !== action) {
    throw new AttestError("proof was not issued for this estate's action");
  }
  if (body.protocol_version !== PROTOCOL_VERSION) {
    throw new AttestError(`World verified a World ID ${String(body.protocol_version)} proof`);
  }
  const results = Array.isArray(body.results) ? body.results.map(asObject) : [];
  const [only] = results;
  if (
    results.length !== 1 ||
    !SELFIE_IDENTIFIERS.includes(String(only.identifier)) ||
    only.success !== true
  ) {
    throw new AttestError("World did not verify exactly one Selfie Check");
  }
  const same = (v: unknown) => typeof v === "string" && v.trim().toLowerCase() === nullifier;
  if (!same(body.nullifier) || !same(only.nullifier)) {
    throw new AttestError("World verified a different nullifier");
  }
}

/** `sha256("herit-near:commitment:v1" || salt || nullifier)`, as the worker computes it. */
export function commitment(salt: Uint8Array, nullifier: string): Uint8Array {
  return createHash("sha256")
    .update("herit-near:commitment:v1")
    .update(salt)
    .update(nullifier)
    .digest();
}

/** The whole operation: check, verify with World, sign. */
export async function attest(req: AttestRequest, host: Host): Promise<AttestResponse> {
  const { purpose, subject, result } = req;
  const action = worldAction(purpose);
  // Checked before calling World, so a wrong credential costs nothing.
  const nullifier = selfieNullifier(result);

  // Forwarded exactly as IDKit returned it: re-encoding a field is the usual cause of invalid_proof.
  const { status, reply } = await host.verifyWithWorld(JSON.stringify(result));
  checkVerified(status, reply, result, action, nullifier);

  const heirLabel = purpose.kind === "claim" ? purpose.heirLabel : null;
  const actionCode = purpose.kind === "claim" ? ACTION_CLAIM : ACTION_CHECKIN;
  const commit = commitment(host.salt, nullifier);
  const nonce = host.randomNonce();
  const expiryNs = host.nowNs() + TTL_NS;

  const message = encodePayload({
    estate_id: estateAccountId(purpose.estateLabel),
    subject,
    action: actionCode,
    heir_label: heirLabel,
    commitment: commit,
    nonce,
    expiry_ns: expiryNs,
  });

  return {
    attestation: {
      action: actionCode,
      heir_label: heirLabel,
      commitment: Buffer.from(commit).toString("base64"),
      nonce: nonce.toString(),
      expiry_ns: expiryNs.toString(),
    },
    signature: Buffer.from(host.sign(message)).toString("base64"),
  };
}
