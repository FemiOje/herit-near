// The same cases as near/attestor-worker/src/logic/tests.rs. Run with `pnpm test`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { generateKeyPairSync, sign, verify } from "node:crypto";

import { encodePayload } from "../scripts/attestation-payload.mjs";
import {
  AttestError,
  TTL_NS,
  attest,
  checkVerified,
  commitment,
  parseRequest,
  selfieNullifier,
  type AttestRequest,
  type Host,
} from "./attest.ts";

const NOW_NS = 1_700_000_000_000_000_000n;
const SALT = new Uint8Array(32).fill(9);

const selfie = (nullifier: string) => ({ identifier: "selfie", nullifier });
const proof = (responses: unknown[], action = "herit:alice") => ({
  protocol_version: "3.0",
  action,
  responses,
});

/** The shape of the real sandbox reply in herit-docs/world-verify-reply.json. */
const worldReply = (nullifier = "0xabc", action = "herit:alice") => ({
  success: true,
  action,
  nullifier,
  created_at: "2026-11-10T12:28:30.522148+00:00",
  environment: "sandbox",
  protocol_version: "3.0",
  results: [{ identifier: "selfie", success: true, nullifier }],
  message: "Proof verified successfully",
});

function fakeHost(status = 200, reply: unknown = worldReply()) {
  const { privateKey, publicKey } = generateKeyPairSync("ed25519");
  const host: Host & { worldCalls: number } = {
    worldCalls: 0,
    async verifyWithWorld() {
      host.worldCalls += 1;
      return { status, reply };
    },
    sign: (message) => sign(null, message, privateKey),
    salt: SALT,
    nowNs: () => NOW_NS,
    randomNonce: () => 42n,
  };
  return { host, publicKey };
}

const checkIn = (result: unknown = proof([selfie("0xABC")])): AttestRequest => ({
  purpose: { kind: "checkin", estateLabel: "alice" },
  subject: "alice-kaykay.testnet",
  result,
});

const refuses = (fn: () => unknown, pattern?: RegExp) =>
  assert.throws(fn, (e: unknown) => e instanceof AttestError && (!pattern || pattern.test(e.message)));

test("a single Selfie Check is accepted, nullifier normalised", () => {
  assert.equal(selfieNullifier(proof([selfie(" 0xABC ")])), "0xabc");
});

test("the legacy face identifier is accepted", () => {
  assert.equal(selfieNullifier(proof([{ identifier: "face", nullifier: "0x1" }])), "0x1");
});

test("no responses, another credential, or a bundle is refused", () => {
  refuses(() => selfieNullifier(proof([])), /exactly one/);
  refuses(() => selfieNullifier({ protocol_version: "3.0" }), /no responses/);
  refuses(() => selfieNullifier(proof([{ identifier: "orb", nullifier: "0x1" }])), /orb/);
  refuses(
    () => selfieNullifier(proof([selfie("0x1"), { identifier: "orb", nullifier: "0x2" }])),
    /selfie, orb/,
  );
  refuses(() => selfieNullifier(proof([{ identifier: "selfie" }])), /no nullifier/);
});

test("only World ID 3.0 proofs are accepted", () => {
  refuses(() => selfieNullifier({ ...proof([selfie("0x1")]), protocol_version: "4.0" }), /4\.0/);
  refuses(() => selfieNullifier({ responses: [selfie("0x1")] }), /none/);
});

test("the real reply's shape is accepted", () => {
  checkVerified(200, worldReply("0x1"), proof([selfie("0x1")]), "herit:alice", "0x1");
  // Case and whitespace don't matter: the proof's nullifier is normalised before comparing.
  checkVerified(200, worldReply(" 0xABC "), proof([selfie("0xabc")]), "herit:alice", "0xabc");
});

test("anything short of World's success shape is refused", () => {
  const p = proof([selfie("0x1")]);
  const ok = worldReply("0x1");
  const check = (status: number, reply: unknown) => () =>
    checkVerified(status, reply, p, "herit:alice", "0x1");
  refuses(check(400, { code: "invalid_proof", detail: "bad proof" }), /bad proof/);
  refuses(check(500, ok));
  refuses(check(200, null));
  refuses(check(200, { ...ok, success: false }));
  refuses(check(200, { ...ok, success: "true" }));
  refuses(check(200, { ...ok, protocol_version: "4.0" }), /4\.0/);
  refuses(check(200, { ...ok, results: [] }), /exactly one/);
  refuses(check(200, { ...ok, results: [...ok.results, ...ok.results] }), /exactly one/);
  refuses(check(200, { ...ok, results: [{ ...ok.results[0], identifier: "orb" }] }));
  refuses(check(200, { ...ok, results: [{ ...ok.results[0], success: false }] }));
  refuses(check(200, { ...ok, nullifier: "0x2" }), /different nullifier/);
  refuses(check(200, { ...ok, results: [{ ...ok.results[0], nullifier: "0x2" }] }));
  refuses(check(200, { ...ok, nullifier: undefined }));
});

test("a proof for another action is refused", () => {
  const ok = worldReply("0x1");
  refuses(() =>
    checkVerified(200, ok, proof([selfie("0x1")], "herit:bob"), "herit:alice", "0x1"),
  );
  refuses(() => checkVerified(200, ok, { responses: [selfie("0x1")] }, "herit:alice", "0x1"));
  refuses(() =>
    checkVerified(200, worldReply("0x1", "herit:bob"), proof([selfie("0x1")]), "herit:alice", "0x1"),
  );
  refuses(() =>
    checkVerified(200, { ...ok, action: undefined }, proof([selfie("0x1")]), "herit:alice", "0x1"),
  );
});

test("bad requests are refused", () => {
  const good = { purpose: { kind: "checkin", estateLabel: "alice" }, subject: "a.testnet", result: {} };
  parseRequest(good);
  for (const bad of [
    null,
    { ...good, purpose: { kind: "checkin", estateLabel: "Alice" } },
    { ...good, purpose: { kind: "checkin", estateLabel: "a.b" } },
    { ...good, purpose: { kind: "claim", estateLabel: "alice" } },
    { ...good, purpose: { kind: "withdraw", estateLabel: "alice" } },
    { ...good, subject: "" },
    { ...good, subject: "Bad Account" },
    { ...good, result: null },
  ]) {
    refuses(() => parseRequest(bad));
  }
});

test("one human gets one commitment, two humans get two", () => {
  assert.deepEqual(commitment(SALT, "0xabc"), commitment(SALT, "0xabc"));
  assert.notDeepEqual(commitment(SALT, "0xabc"), commitment(SALT, "0xabd"));
  assert.notDeepEqual(commitment(SALT, "0xabc"), commitment(new Uint8Array(32), "0xabc"));
});

test("World is not called for a proof that isn't a Selfie Check", async () => {
  const { host } = fakeHost();
  await assert.rejects(attest(checkIn(proof([{ identifier: "orb", nullifier: "0x1" }])), host));
  assert.equal(host.worldCalls, 0);
});

test("nothing is signed when World rejects the proof", async () => {
  const { host } = fakeHost(400, { code: "invalid_proof" });
  await assert.rejects(attest(checkIn(), host), /invalid_proof/);
  assert.equal(host.worldCalls, 1);
});

test("the attestation is signed over the contract's Borsh payload", async () => {
  const { host, publicKey } = fakeHost();
  const out = await attest(checkIn(), host);

  assert.equal(out.attestation.action, 0);
  assert.equal(out.attestation.heir_label, null);
  assert.equal(out.attestation.nonce, "42");
  assert.equal(out.attestation.expiry_ns, (NOW_NS + TTL_NS).toString());
  const commit = Buffer.from(out.attestation.commitment, "base64");
  assert.deepEqual(commit, commitment(SALT, "0xabc"));

  // The bytes the contract rebuilds: estate id and subject come from its runtime.
  const message = encodePayload({
    estate_id: "alice.herit-near.testnet",
    subject: "alice-kaykay.testnet",
    action: 0,
    heir_label: null,
    commitment: commit,
    nonce: 42n,
    expiry_ns: NOW_NS + TTL_NS,
  });
  assert.ok(verify(null, message, publicKey, Buffer.from(out.signature, "base64")));
});

test("a claim attestation carries the heir label", async () => {
  const out = await attest(
    {
      purpose: { kind: "claim", estateLabel: "alice", heirLabel: "son" },
      subject: "son.alice.herit-near.testnet",
      result: proof([selfie("0x1")]),
    },
    fakeHost(200, worldReply("0x1")).host,
  );
  assert.equal(out.attestation.action, 1);
  assert.equal(out.attestation.heir_label, "son");
});
