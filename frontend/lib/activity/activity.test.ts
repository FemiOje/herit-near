import assert from "node:assert/strict";
import test from "node:test";

import { describe, parseActivity, type FastTx } from "./events.ts";

const E = "keytest.herit-near.testnet";
const ev = (event: string, data: object) => `EVENT_JSON:${JSON.stringify({ standard: "herit", version: "1.0.0", event, data: [data] })}`;
const tx = (hash: string, signer: string, receipts: [string, string[]][]): FastTx => ({
  transaction: { hash, signer_id: signer },
  receipts: receipts.map(([receiver, logs]) => ({ receipt: { receiver_id: receiver }, execution_outcome: { outcome: { logs } } })),
});

test("only the estate's own herit events count, newest first, in logged order", () => {
  const txs = [
    tx("OPEN", "alice.testnet", [
      ["herit-near.testnet", [ev("estate_opened", { estate: E, grantor: "alice.testnet" })]],
      [E, [ev("estate_opened", { grantor: "alice.testnet" })]],
    ]),
    tx("CLAIM", "son." + E, [
      [E, [ev("snapshotted", { assets: 1 }), ev("unlocked", { at: "1", heirs: 2 }), "a plain log", 'EVENT_JSON:{"standard":"nep171","event":"nft_mint"}']],
      ["son." + E, []],
      [E, [ev("claimed", { label: "son", asset: "near", amount: "200000000000000000000000" })]],
    ]),
    tx("UNKNOWN", "x", [[E, [ev("deposited", { asset: "near", amount: "1" })]]]),
  ];
  const times = [
    { transaction_hash: "OPEN", tx_block_timestamp: "1000000000" },
    { transaction_hash: "CLAIM", tx_block_timestamp: "5000000000" },
  ];
  const a = parseActivity(E, times, txs);
  assert.deepEqual(a.map((x) => `${x.txHash} ${x.event}`), ["CLAIM snapshotted", "CLAIM unlocked", "CLAIM claimed", "OPEN estate_opened"]);
  assert.equal(a[0].at, 5000);
  assert.equal(a[0].by, "son." + E);
});

test("every event reads as a sentence", () => {
  assert.equal(describe({ event: "claimed", data: { label: "son", asset: "near", amount: "200000000000000000000000" } }).text, "son received 0.2 NEAR");
  assert.equal(describe({ event: "deposited", data: { asset: "mock-usdc-kaykay.testnet", amount: "10000000" } }).text, "10 mUSDC deposited");
  assert.equal(describe({ event: "configured", data: { interval_sec: 120, grace_sec: 60 } }).text, "clock set: check in every 2 min, then 1 min grace");
  assert.equal(describe({ event: "share_set", data: { label: "son", asset: "near", bps: 2500 } }).text, "son's share of NEAR set to 25%");
  assert.equal(describe({ event: "unlocked", data: { heirs: 1 } }).tone, "bad");
  assert.equal(describe({ event: "something_new", data: {} }).text, "something new");
});

test("a failed receipt's logs don't count: what it did was rolled back", () => {
  const failed: FastTx = {
    transaction: { hash: "AGAIN", signer_id: "son." + E },
    receipts: [{ receipt: { receiver_id: E }, execution_outcome: { outcome: { logs: [ev("claim_attested", { label: "son" })], status: { Failure: {} } } } }],
  };
  assert.deepEqual(parseActivity(E, [{ transaction_hash: "AGAIN", tx_block_timestamp: "1" }], [failed]), []);
});
