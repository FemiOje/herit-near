import assert from "node:assert/strict";
import test from "node:test";

import type { EstateEntry, HeirView, NftToken } from "../near/contracts.ts";
import { hasSomethingToClaim, shareLine } from "./share.ts";
import { findSlots, type SlotReaders } from "./slots.ts";

const F = "herit-near.testnet";
const entry = (label: string): EstateEntry => ({ account_id: `${label}.${F}`, grantor: "g.testnet", opened_at_ns: "1" });
const token = (label: string, owner: string, relationship = label): NftToken => ({
  token_id: label,
  owner_id: owner,
  metadata: { title: owner, description: null, media: null, extra: JSON.stringify({ relationship, state: "locked" }) },
});

/** A pretend chain: estates, each with its heirs as label → account. */
function chain(estates: Record<string, Record<string, string>>, broken: string[] = []): SlotReaders & { calls: number } {
  const ids = Object.keys(estates);
  const r = {
    calls: 0,
    estates: async (from: number, limit: number) => ids.slice(from, from + limit).map(entry),
    heirOf: async (estate: string, label: string): Promise<HeirView | null> => {
      const account = estates[estate.split(".")[0]]?.[label];
      return account ? { label, account_id: account, relationship: label, default_bps: 1000, created: true } : null;
    },
    credentialsOf: async (estate: string, account: string) => {
      r.calls++;
      const label = estate.split(".")[0];
      if (broken.includes(label)) throw new Error("unreadable");
      return Object.entries(estates[label] ?? {}).filter(([, a]) => a === account).map(([l]) => token(l, account));
    },
  };
  return r;
}

test("a named heir is found in every estate that named them", async () => {
  const r = chain({ alice: { kay: "their-account.testnet", son: "son.alice.herit-near.testnet" }, bob: { friend: "their-account.testnet" }, carol: {} });
  const slots = await findSlots("their-account.testnet", F, r, { pageSize: 2 });
  assert.deepEqual(slots.map((s) => `${s.estate} ${s.label}`).sort(), ["alice.herit-near.testnet kay", "bob.herit-near.testnet friend"]);
});

test("a key heir is found from their account's name, once", async () => {
  const r = chain({ alice: { son: "son.alice.herit-near.testnet" }, bob: {} });
  const slots = await findSlots("son.alice.herit-near.testnet", F, r);
  assert.equal(slots.length, 1);
  assert.deepEqual([slots[0].estate, slots[0].label], ["alice.herit-near.testnet", "son"]);
});

test("an unreadable estate is skipped, and nobody's heir finds nothing", async () => {
  const r = chain({ alice: { kay: "their-account.testnet" }, broken: {}, bob: { kay2: "their-account.testnet" } }, ["broken"]);
  assert.equal((await findSlots("their-account.testnet", F, r)).length, 2);
  assert.deepEqual(await findSlots("nobody.testnet", F, r), []);
});

test("estates are read a page at a time, up to the cap", async () => {
  const many = Object.fromEntries(Array.from({ length: 30 }, (_, i) => [`e${i}`, {}]));
  const r = chain(many);
  await findSlots("their-account.testnet", F, r, { pageSize: 10, maxEstates: 20 });
  assert.equal(r.calls, 20);
});

const base = { bps: 4000, balance: BigInt(1_000), snapshot: BigInt(0), snapshotTaken: false, paid: false, unlocked: false };

test("before unlock, an heir's line is what the balance would give", () => {
  assert.deepEqual(shareLine(base), { state: "expected", amount: BigInt(400) });
  assert.deepEqual(shareLine({ ...base, bps: 0 }), { state: "none", amount: BigInt(0) });
});

test("after unlock the snapshot rules, rounded down as the contract does", () => {
  const frozen = { ...base, unlocked: true, snapshotTaken: true, snapshot: BigInt(999), balance: BigInt(5) };
  assert.deepEqual(shareLine(frozen), { state: "claimable", amount: BigInt(399) });
  // Unlocked but nobody has poked yet: the balance is what will be frozen.
  assert.deepEqual(shareLine({ ...base, unlocked: true }), { state: "claimable", amount: BigInt(400) });
  assert.deepEqual(shareLine({ ...frozen, paid: true }).state, "paid");
});

test("a claim is worth sending only when something unpaid is claimable", () => {
  const line = (state: "none" | "expected" | "claimable" | "paid", amount: number) => ({ state, amount: BigInt(amount) });
  assert.ok(hasSomethingToClaim([line("paid", 5), line("claimable", 1)]));
  assert.ok(!hasSomethingToClaim([line("paid", 5), line("claimable", 0), line("none", 0)]));
  assert.ok(!hasSomethingToClaim([line("expected", 9)]));
});

test("a claim's gas and deposit grow with its payouts, within NEAR's limits", async () => {
  const { claimCost, spendable } = await import("./claim-cost.ts");
  const { parseNear } = await import("../near/format.ts");
  // NEAR and one token: what the smoke run sent by hand (50 Tgas, 0.01 NEAR), with headroom.
  const one = claimCost(1, 1);
  assert.equal(one.tgas, 55);
  assert.equal(one.deposit, parseNear("0.01025") + BigInt(1));
  // A key heir's 0.1 NEAR seed covers it: 0.055 NEAR held for gas plus the deposit.
  assert.ok(one.held < parseNear("0.1"), `held ${one.held}`);
  // The contract's ten assets stay under the 300 Tgas a transaction may attach.
  assert.ok(claimCost(1, 9).tgas <= 300);
  assert.ok(claimCost(1, 9).held > parseNear("0.1"), "a big estate needs more than the seed");
  assert.equal(claimCost(1, 0).tgas, 30);
  // Storage locks part of a balance.
  assert.equal(spendable({ amount: parseNear("0.1").toString(), storage_usage: 182 }), parseNear("0.1") - BigInt(182) * BigInt(10) ** BigInt(19));
  assert.equal(spendable({ amount: "5", storage_usage: 182 }), BigInt(0));
});
