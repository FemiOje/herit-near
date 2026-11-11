import assert from "node:assert/strict";
import test from "node:test";

import { NOTHING_YET, OPENING_DEPOSIT, isSetUp, setupPlan, setupProblems, type SetupDraft } from "./setup-plan.ts";
import { parseNear } from "../near/format.ts";

const KEY = "ed25519:FzTUQSiLeJji46M3GXzoiBntYV5wbnomrF4ywomBJwfe";
const draft: SetupDraft = {
  label: "alice",
  intervalSec: 120,
  graceSec: 60,
  heirs: [
    { label: "son", relationship: "son", bps: 6000, publicKey: KEY },
    { label: "daughter", relationship: "daughter", bps: 4000, publicKey: KEY },
  ],
  depositNear: "3",
};
const signed = {
  attestation: { action: 0, heir_label: null, commitment: "AA==", nonce: "1", expiry_ns: "2" },
  signature: "BB==",
};
const F = "herit-near.testnet";

test("a fresh setup is open, check in, every heir, deposit, in that order", () => {
  const plan = setupPlan(F, draft, NOTHING_YET, signed);
  assert.deepEqual(
    plan.map((c) => `${c.receiverId} ${c.method}`),
    [
      "herit-near.testnet open_estate",
      "alice.herit-near.testnet check_in",
      "alice.herit-near.testnet add_heir",
      "alice.herit-near.testnet add_heir",
      "alice.herit-near.testnet deposit",
    ],
  );
  assert.equal(plan[0].deposit, OPENING_DEPOSIT);
  assert.equal(plan[4].deposit, parseNear("3"));
});

test("an estate can't be set up without the Selfie Check", () => {
  assert.throws(() => setupPlan(F, draft, NOTHING_YET, null), /Selfie Check/);
});

test("a setup that stopped halfway sends only what's missing", () => {
  const progress = { exists: true, faceBound: true, heirs: ["son"], vaultNear: BigInt(0) };
  const plan = setupPlan(F, draft, progress, null);
  assert.deepEqual(plan.map((c) => c.method), ["add_heir", "deposit"]);
  assert.equal(plan[0].args.label, "daughter");
  const done = setupPlan(F, draft, { ...progress, heirs: ["son", "daughter"], vaultNear: BigInt(1) }, null);
  assert.deepEqual(done, []);
});

test("no heirs, no setup", () => {
  assert.ok(setupProblems({ ...draft, heirs: [] }).includes("add at least one heir"));
  assert.deepEqual(setupProblems(draft), []);
  assert.ok(!isSetUp({ exists: true, faceBound: true, heirs: [] }));
  assert.ok(!isSetUp({ exists: true, faceBound: false, heirs: ["son"] }));
  assert.ok(isSetUp({ exists: true, faceBound: true, heirs: ["son"] }));
});

test("shares, labels, keys and timers are checked before anything is sent", () => {
  const over = { ...draft, heirs: [{ ...draft.heirs[0], bps: 7000 }, draft.heirs[1]] };
  assert.ok(setupProblems(over).some((p) => p.includes("110%")));
  assert.ok(setupProblems({ ...draft, heirs: [draft.heirs[0], { ...draft.heirs[1], label: "son" }] }).includes("two heirs share a label"));
  assert.ok(setupProblems({ ...draft, heirs: [{ label: "son", relationship: "son", bps: 6000, publicKey: "nope" }] }).some((p) => p.includes("public key")));
  assert.ok(setupProblems({ ...draft, intervalSec: 60 }).length > 0);
  assert.ok(setupProblems({ ...draft, depositNear: "lots" }).length > 0);
  assert.deepEqual(setupProblems({ ...draft, depositNear: "" }), [], "the vault can be funded later");
});

test("a named heir goes to add_heir_account, a key heir to add_heir, in one batch", () => {
  const mixed: SetupDraft = {
    ...draft,
    heirs: [
      { label: "son", relationship: "son", bps: 6000, publicKey: KEY },
      { kind: "account", label: "kay", relationship: "friend", bps: 4000, accountId: "kaynearhackathon2.testnet" },
    ],
  };
  assert.deepEqual(setupProblems(mixed, { factory: F, grantor: "alice-kaykay.testnet" }), []);
  const plan = setupPlan(F, mixed, NOTHING_YET, signed);
  assert.deepEqual(plan.map((c) => c.method), ["open_estate", "check_in", "add_heir", "add_heir_account", "deposit"]);
  assert.deepEqual(plan[3].args, { label: "kay", account_id: "kaynearhackathon2.testnet", relationship: "friend", default_bps: 4000 });
  assert.equal(plan[3].deposit, parseNear("0.01"));
});

test("a named heir can't be the grantor, under the estate, or named twice", () => {
  const named = (label: string, accountId: string) => ({ kind: "account" as const, label, relationship: label, bps: 1000, accountId });
  const ctx = { factory: F, grantor: "alice-kaykay.testnet" };
  const problems = (heirs: SetupDraft["heirs"]) => setupProblems({ ...draft, heirs }, ctx).join(" | ");
  assert.match(problems([named("me", "alice-kaykay.testnet")]), /your own heir/);
  assert.match(problems([named("x", "son.alice.herit-near.testnet")]), /under this estate/);
  assert.match(problems([named("x", "alice.herit-near.testnet")]), /under this estate/);
  assert.match(problems([named("a", "kay.testnet"), named("b", "kay.testnet")]), /already an heir/);
  assert.match(problems([named("a", "Not An Account")]), /a NEAR account/);
  // Another estate's sub-account is someone else's account: fine.
  assert.equal(problems([named("a", "son.bob.herit-near.testnet")]), "");
});

test("a draft saved before named heirs still reads as key heirs", () => {
  const old = JSON.parse(JSON.stringify(draft)) as SetupDraft; // no `kind` on its heirs
  assert.deepEqual(setupPlan(F, old, NOTHING_YET, signed).filter((c) => c.method.startsWith("add_heir")).map((c) => c.method), ["add_heir", "add_heir"]);
});
