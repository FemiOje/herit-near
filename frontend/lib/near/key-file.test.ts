import assert from "node:assert/strict";
import test from "node:test";

import { KeyPair } from "near-api-js";

import { KeyFileError, keyProblem, parseKeyFile } from "./key-file.ts";
import { panicMessage, readOutcome, TxError } from "./outcome.ts";

const pair = KeyPair.fromRandom("ed25519");
const other = KeyPair.fromRandom("ed25519");
const SON = "son.alice.herit-near.testnet";
const file = (o: object) => JSON.stringify(o);

test("a key file from the heirs page or the NEAR CLI reads back", () => {
  const k = parseKeyFile(file({ account_id: SON, public_key: pair.getPublicKey().toString(), private_key: pair.toString() }));
  assert.deepEqual(k, { accountId: SON, publicKey: pair.getPublicKey().toString(), secret: pair.toString() });
  // The heirs page saves no account id; the public key is worked out from the private one.
  assert.equal(parseKeyFile(file({ private_key: pair.toString() })).accountId, null);
  assert.equal(parseKeyFile(file({ secret_key: pair.toString() })).publicKey, pair.getPublicKey().toString());
});

test("a broken key file is refused before anything is asked of NEAR", () => {
  assert.throws(() => parseKeyFile("not json"), KeyFileError);
  assert.throws(() => parseKeyFile(file({ public_key: "ed25519:x" })), /no ed25519 private key/);
  assert.throws(() => parseKeyFile(file({ private_key: "ed25519:notakey" })), /malformed/);
  assert.throws(
    () => parseKeyFile(file({ public_key: other.getPublicKey().toString(), private_key: pair.toString() })),
    /doesn't match/,
  );
});

test("the key must be a full-access key of the heir's account", async () => {
  const k = parseKeyFile(file({ private_key: pair.toString() }));
  const full = async () => "FullAccess" as const;
  assert.equal(await keyProblem(k, SON, full), undefined);
  assert.match((await keyProblem(k, SON, async () => null))!, /isn't one of/);
  assert.match((await keyProblem(k, SON, async () => ({ FunctionCall: { receiver_id: "x", method_names: [] } })))!, /full-access/);
  const forOther = parseKeyFile(file({ account_id: "daughter.alice.herit-near.testnet", private_key: pair.toString() }));
  assert.match((await keyProblem(forOther, SON, full))!, /is for daughter/);
});

test("a wallet's outcome and a key signer's read the same way", () => {
  const ok = readOutcome({ transaction: { hash: "H" }, status: { SuccessValue: btoa("2") } });
  assert.deepEqual(ok, { txHash: "H", value: 2 });
  assert.throws(
    () => readOutcome({ transaction: { hash: "H" }, status: { Failure: { ActionError: { kind: { FunctionCallError: { ExecutionError: "Smart contract panicked: nothing to claim" } } } } } }),
    (e: unknown) => e instanceof TxError && e.message === "nothing to claim" && e.txHash === "H",
  );
  assert.equal(panicMessage("Error: Smart contract panicked: estate not unlocked"), "estate not unlocked");
});
