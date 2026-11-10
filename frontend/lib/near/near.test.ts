// Run with `pnpm test`.
import { test } from "node:test";
import assert from "node:assert/strict";

import { estateId, estateOfHeir, isAccountId, isLabel, labelOf } from "./account.ts";
import { bpsToPercent, formatNear, formatUnits, nsToMs, parseNear, parseUnits, shareOf } from "./format.ts";
import { ViewError, view } from "./rpc.ts";
import { credentialState } from "./contracts.ts";

const F = "herit-near.testnet";

test("account ids follow NEAR's rule", () => {
  for (const ok of ["alice.testnet", "herit-near.testnet", "son.alice.herit-near.testnet", "a1", "x_y.near"]) {
    assert.ok(isAccountId(ok), ok);
  }
  for (const bad of ["", "a", "Alice.testnet", "a..b", ".a", "a.", "a--", "a b", "a".repeat(65)]) {
    assert.ok(!isAccountId(bad), bad);
  }
  assert.ok(isLabel("daughter-2"));
  assert.ok(!isLabel("a.b"));
});

test("estates and heirs map onto each other", () => {
  assert.equal(estateId("alice", F), "alice.herit-near.testnet");
  assert.equal(estateOfHeir("son.alice.herit-near.testnet", F), "alice.herit-near.testnet");
  assert.equal(estateOfHeir("alice.herit-near.testnet", F), null, "an estate isn't an heir");
  assert.equal(estateOfHeir("son.alice.other.testnet", F), null);
  assert.equal(estateOfHeir("x.son.alice.herit-near.testnet", F), null, "too deep");
  assert.equal(labelOf("son.alice.herit-near.testnet"), "son");
});

test("amounts parse and format exactly", () => {
  assert.equal(parseNear("1.5"), 1_500_000_000_000_000_000_000_000n);
  assert.equal(parseNear("0"), 0n);
  assert.equal(parseUnits("2.5", 6), 2_500_000n);
  assert.throws(() => parseUnits("0.0000001", 6));
  assert.throws(() => parseNear("-1"));
  assert.throws(() => parseNear("1e3"));
  assert.equal(formatNear(1_500_000_000_000_000_000_000_000n), "1.5");
  assert.equal(formatNear(2_498_380_000_000_000_000_000_000n), "2.4983", "truncated, not rounded");
  assert.equal(formatUnits(2_500_000n, 6), "2.5");
  assert.equal(formatUnits(0n, 6), "0");
  assert.equal(formatUnits(1n, 6), "<0.0001", "dust is never shown as 0");
  assert.equal(bpsToPercent(6000), "60%");
  assert.equal(bpsToPercent(2550), "25.5%");
  assert.equal(shareOf(3n, 6000), 1n, "rounded down, as the estate pays");
  assert.equal(nsToMs("1794314348641418591"), 1794314348641);
});

test("the credential state comes from the token's extra JSON", () => {
  const token = (extra: string | null) => ({ token_id: "son", owner_id: "son.x", metadata: { title: null, description: null, media: null, extra } });
  assert.equal(credentialState(token('{"state":"claimable"}')), "claimable");
  assert.equal(credentialState(token('{"state":"nonsense"}')), null);
  assert.equal(credentialState(token("not json")), null);
  assert.equal(credentialState(null), null);
});

function reply(body: unknown, status = 200): typeof fetch {
  return (async () => new Response(JSON.stringify(body), { status })) as typeof fetch;
}

test("view decodes a contract's JSON result", async () => {
  const bytes = [...new TextEncoder().encode('["alice.herit-near.testnet"]')];
  const out = await view<string[]>(F, "estates_of", { grantor: "a.testnet" }, ["https://rpc"], reply({ result: { result: bytes } }));
  assert.deepEqual(out, ["alice.herit-near.testnet"]);
});

test("view sends the args as base64 JSON", async () => {
  let sent: { params: { method_name: string; args_base64: string } } = { params: { method_name: "", args_base64: "" } };
  const fetcher = (async (_url: string, init: RequestInit) => {
    sent = JSON.parse(init.body as string);
    return new Response(JSON.stringify({ result: { result: [...new TextEncoder().encode("1")] } }));
  }) as typeof fetch;
  await view(F, "estates_count", { a: "é" }, ["https://rpc"], fetcher);
  assert.equal(sent.params.method_name, "estates_count");
  assert.equal(new TextDecoder().decode(Uint8Array.from(atob(sent.params.args_base64), (c) => c.charCodeAt(0))), '{"a":"é"}');
});

test("a contract error is final; a network error falls back to the next RPC", async () => {
  await assert.rejects(
    view(F, "nope", {}, ["https://a", "https://b"], reply({ result: { error: "MethodNotFound" } })),
    (e: unknown) => e instanceof ViewError && /MethodNotFound/.test(e.message),
  );
  let calls = 0;
  const flaky = (async () => {
    calls += 1;
    if (calls === 1) throw new Error("ECONNRESET");
    return new Response(JSON.stringify({ result: { result: [...new TextEncoder().encode("3")] } }));
  }) as typeof fetch;
  assert.equal(await view<number>(F, "estates_count", {}, ["https://a", "https://b"], flaky), 3);
  assert.equal(calls, 2);
  await assert.rejects(view(F, "x", {}, ["https://a"], reply({}, 503)), /no RPC answered/);
});
