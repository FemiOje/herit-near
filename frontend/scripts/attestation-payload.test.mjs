// node --test scripts/
import { test } from "node:test";
import assert from "node:assert/strict";

import { encodePayload } from "./attestation-payload.mjs";

// Same payloads and hex as near/contracts/herit-estate/src/golden.rs and the worker's golden test.
const CHECK_IN_HEX =
  "0d00000068657269742d6e6561723a763118000000616c6963652e68657269742d6e6561722e746573746e657414000000616c6963652d6b61796b61792e746573746e6574000001010101010101010101010101010101010101010101010101010101010101012a00000000000000002858c2cf9d9717";
const CLAIM_HEX =
  "0d00000068657269742d6e6561723a763118000000616c6963652e68657269742d6e6561722e746573746e65741c000000736f6e2e616c6963652e68657269742d6e6561722e746573746e6574010103000000736f6e0202020202020202020202020202020202020202020202020202020202020202ffffffffffffffff002858c2cf9d9717";
const EXPIRY_NS = 1_700_000_900_000_000_000n;

const hex = (bytes) => Buffer.from(bytes).toString("hex");

test("check-in payload matches the contract's golden bytes", () => {
  const bytes = encodePayload({
    estate_id: "alice.herit-near.testnet",
    subject: "alice-kaykay.testnet",
    action: 0,
    heir_label: null,
    commitment: new Uint8Array(32).fill(1),
    nonce: 42n,
    expiry_ns: EXPIRY_NS,
  });
  assert.equal(hex(bytes), CHECK_IN_HEX);
});

test("claim payload with a label matches the contract's golden bytes", () => {
  const bytes = encodePayload({
    estate_id: "alice.herit-near.testnet",
    subject: "son.alice.herit-near.testnet",
    action: 1,
    heir_label: "son",
    commitment: new Uint8Array(32).fill(2),
    nonce: 18446744073709551615n,
    expiry_ns: EXPIRY_NS,
  });
  assert.equal(hex(bytes), CLAIM_HEX);
});
