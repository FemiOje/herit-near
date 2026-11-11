import assert from "node:assert/strict";
import test from "node:test";

import { ADD_HEIR_DEPOSIT, MIN_OPEN_DEPOSIT, calls, isPublicKey } from "./calls.ts";
import { decodeHeader, estateHeader } from "./estate-state.ts";
import { parseNear } from "./format.ts";
import { formatCountdown, formatDuration, graceStartsAt, phase, unlocksAt } from "../estate/clock.ts";

/** The Borsh prefix of `Estate`, built by hand the way the contract lays it out. */
function header(opts: { face: boolean; interval: bigint; grace: bigint; last: bigint }): Uint8Array {
  const parts: number[] = [];
  const str = (s: string) => {
    const b = new TextEncoder().encode(s);
    parts.push(b.length & 0xff, (b.length >> 8) & 0xff, 0, 0, ...b);
  };
  const u64 = (v: bigint) => {
    for (let i = 0; i < 8; i++) parts.push(Number((v >> BigInt(8 * i)) & BigInt(0xff)));
  };
  str("herit-near.testnet");
  str("alice-kaykay.testnet");
  parts.push(...new Array(32).fill(7)); // attestor_pk
  parts.push(opts.face ? 1 : 0, ...(opts.face ? new Array(32).fill(9) : []));
  u64(opts.interval);
  u64(opts.grace);
  u64(opts.last);
  parts.push(1, 2, 3); // the rest of the state
  return Uint8Array.from(parts);
}

const S = BigInt(1_000_000_000);

test("the estate header decodes, with and without a bound face", () => {
  for (const face of [false, true]) {
    const h = decodeHeader(header({ face, interval: BigInt(120) * S, grace: BigInt(60) * S, last: BigInt(5) }));
    assert.deepEqual(h, {
      factory: "herit-near.testnet",
      grantor: "alice-kaykay.testnet",
      faceBound: face,
      intervalSec: 120,
      graceSec: 60,
      lastCheckInNs: BigInt(5),
    });
  }
  assert.throws(() => decodeHeader(new Uint8Array([3, 0, 0, 0, 97])));
});

test("the header is read from the STATE record over RPC", async () => {
  const bytes = header({ face: true, interval: BigInt(600) * S, grace: BigInt(300) * S, last: BigInt(0) });
  const fetcher = (async () =>
    new Response(
      JSON.stringify({
        result: { values: [{ key: btoa("STATE"), value: btoa(String.fromCharCode(...bytes)) }] },
      }),
    )) as typeof fetch;
  const h = await estateHeader("alice.herit-near.testnet", ["https://rpc"], fetcher);
  assert.equal(h.intervalSec, 600);
  assert.equal(h.grantor, "alice-kaykay.testnet");
});

test("calls carry what the contracts demand", () => {
  const signed = {
    attestation: { action: 2, heir_label: null, commitment: "AA==", nonce: "1", expiry_ns: "2" },
    signature: "BB==",
  };
  const w = calls.withdraw("e.testnet", null, BigInt(5), signed);
  assert.equal(w.deposit, BigInt(1), "withdraw needs exactly 1 yoctoNEAR");
  assert.deepEqual(w.args, { asset: null, amount: "5", ...signed });

  const t = calls.depositToken("e.testnet", "usdc.testnet", BigInt(10_000_000));
  assert.equal(t.receiverId, "usdc.testnet", "a token deposit goes through the token");
  assert.equal(t.deposit, BigInt(1));
  assert.deepEqual(t.args, { receiver_id: "e.testnet", amount: "10000000", msg: "" });

  assert.equal(MIN_OPEN_DEPOSIT, parseNear("0.11"));
  assert.ok(ADD_HEIR_DEPOSIT > parseNear("0.1"), "more than the heir's seed");
  assert.equal(calls.checkIn("e", signed).deposit, BigInt(0));
  assert.deepEqual(calls.openEstate("f", "alice", 120, 60, MIN_OPEN_DEPOSIT).args, {
    label: "alice",
    check_in_interval_sec: 120,
    grace_sec: 60,
  });
});

test("public keys are ed25519 base58", () => {
  assert.ok(isPublicKey("ed25519:FzTUQSiLeJji46M3GXzoiBntYV5wbnomrF4ywomBJwfe"));
  assert.ok(!isPublicKey("FzTUQSiLeJji46M3GXzoiBntYV5wbnomrF4ywomBJwfe"));
  assert.ok(!isPublicKey("ed25519:0OIl"));
  assert.ok(!isPublicKey("secp256k1:FzTUQSiLeJji46M3GXzoiBntYV5wbnomrF4ywomBJwfe"));
});

test("the clock counts to the window's close, then to the unlock", () => {
  const clock = { intervalSec: 120, graceSec: 60, lastCheckInNs: BigInt(1_000_000) * BigInt(1_000_000) };
  const last = 1_000_000;
  assert.equal(graceStartsAt(clock), last + 120_000);
  assert.equal(unlocksAt(clock), last + 180_000);

  assert.deepEqual(phase("active", clock, last + 30_000), { remainingSec: 90, totalSec: 120, progress: 0.25 });
  assert.deepEqual(phase("grace", clock, last + 150_000), { remainingSec: 30, totalSec: 60, progress: 0.5 });
  assert.equal(phase("active", clock, last + 999_000).remainingSec, 0, "never negative");
  assert.equal(phase("unlocked", clock, 0).progress, 1);

  const fresh = { ...clock, lastCheckInNs: BigInt(0) };
  assert.equal(graceStartsAt(fresh), null);
  assert.equal(phase("active", fresh, 0).progress, 0, "not started");
});

test("durations and countdowns read as people say them", () => {
  assert.equal(formatDuration(120), "2 mins");
  assert.equal(formatDuration(30 * 86_400), "30 days");
  assert.equal(formatDuration(3_600), "1 hour");
  assert.equal(formatCountdown(249), "04:09");
  assert.equal(formatCountdown(2 * 3_600 + 15 * 60), "02h 15m");
  assert.equal(formatCountdown(3 * 86_400 + 4 * 3_600), "3d 04h");
});

test("only estates on the code from before named heirs refuse them", async () => {
  const { supportsNamedHeirs } = await import("./calls.ts");
  assert.equal(supportsNamedHeirs("87NJpRzyMsaL4WWYspngjbHoLBKPJSjiyrFEpDKXNgE8"), false);
  assert.equal(supportsNamedHeirs("65rC6sBJGosXHCGC6ityR3poJ68ecHDZsNRAPZQjkX2p"), true);
  assert.equal(supportsNamedHeirs(null), true, "not opened yet: the factory's current code");
});

test("implicit accounts needn't exist before they're named", async () => {
  const { isImplicitAccount } = await import("./account.ts");
  assert.ok(isImplicitAccount("a".repeat(64)));
  assert.ok(isImplicitAccount(`0x${"b".repeat(40)}`));
  assert.ok(!isImplicitAccount("kay.testnet"));
});
