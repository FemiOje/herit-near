// The Borsh layout of the estate contract's `SignedPayload`, shared by the dev signer and its test.
// Must match the field ORDER and TYPES exactly: Borsh is positional. The golden hex in
// attestation-payload.test.mjs is the same as in near/contracts/herit-estate/src/golden.rs.
import { serialize } from "borsh";

export const DOMAIN = "herit-near:v1";

export const SCHEMA = {
  struct: {
    domain: "string",
    estate_id: "string",
    subject: "string",
    action: "u8",
    heir_label: { option: "string" },
    commitment: { array: { type: "u8", len: 32 } },
    nonce: "u64",
    expiry_ns: "u64",
  },
};

/** @returns {Uint8Array} the bytes the attestor signs */
export function encodePayload({ estate_id, subject, action, heir_label, commitment, nonce, expiry_ns }) {
  return serialize(SCHEMA, {
    domain: DOMAIN,
    estate_id,
    subject,
    action,
    heir_label: heir_label ?? null,
    commitment: [...commitment],
    nonce: BigInt(nonce),
    expiry_ns: BigInt(expiry_ns),
  });
}
