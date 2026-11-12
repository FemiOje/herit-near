import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import { LOCKED_CREDENTIAL_CID } from "./config.ts";
import { credentialState, estateCredential, type NftToken } from "./contracts.ts";

const NFT_RS = new URL("../../../near/contracts/herit-estate/src/nft.rs", import.meta.url);

test("the setup wizard's preview is the contract's `locked` image", () => {
  const source = readFileSync(NFT_RS, "utf8");
  const locked = source.match(/\(\s*"locked",\s*"([a-z0-9]+)"/);
  assert.ok(locked, "CREDENTIAL_IMAGES has a `locked` entry");
  assert.equal(LOCKED_CREDENTIAL_CID, locked[1]);
});

const token = (extra: string | null): NftToken => ({
  token_id: "son",
  owner_id: "son.alice.herit-near.testnet",
  metadata: { title: null, description: null, media: null, extra },
});

const held = (label: string, state: string, media = `cid-${state}`): NftToken => ({
  token_id: label,
  owner_id: `${label}.alice.herit-near.testnet`,
  metadata: { title: null, description: null, media, extra: JSON.stringify({ state }) },
});

test("an estate's picture is its unclaimed credentials' state, whatever order they come in", () => {
  const summary = estateCredential([held("son", "claimed"), held("daughter", "claimable")]);
  assert.deepEqual(summary, { state: "claimable", media: "cid-claimable", counts: { claimed: 1, claimable: 1 } });
  assert.equal(estateCredential([held("son", "locked"), held("daughter", "locked")])?.state, "locked");
});

test("an estate whose heirs have all claimed shows claimed; one with no credentials shows nothing", () => {
  assert.equal(estateCredential([held("son", "claimed"), held("daughter", "claimed")])?.state, "claimed");
  assert.equal(estateCredential([]), null);
  assert.equal(estateCredential([token("garbage")]), null);
});

test("a credential's state is read from its extra JSON", () => {
  assert.equal(credentialState(token(JSON.stringify({ state: "claimable" }))), "claimable");
  assert.equal(credentialState(token(JSON.stringify({ state: "minted" }))), null);
  assert.equal(credentialState(token("not json")), null);
  assert.equal(credentialState(null), null);
});
