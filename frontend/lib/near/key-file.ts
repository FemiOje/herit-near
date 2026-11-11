import { KeyPair, type KeyPairString } from "near-api-js";

import { isAccountId } from "./account.ts";
import { viewAccessKey } from "./rpc.ts";

/**
 * A key heir's key file, as the heirs page and the NEAR CLI save it:
 * `{ "account_id"?: "son.alice.herit-near.testnet", "public_key": "ed25519:…", "private_key": "ed25519:…" }`.
 * Read in the browser and kept in memory only: nothing here sends the private key anywhere.
 */
export type KeyFile = { accountId: string | null; publicKey: string; secret: KeyPairString };

export class KeyFileError extends Error {}

/** Parses and checks a key file: its private key must produce its public key. */
export function parseKeyFile(text: string): KeyFile {
  let raw: { account_id?: unknown; public_key?: unknown; private_key?: unknown; secret_key?: unknown };
  try {
    raw = JSON.parse(text);
  } catch {
    throw new KeyFileError("that isn't a key file: it isn't JSON");
  }
  const secret = raw.private_key ?? raw.secret_key;
  if (typeof secret !== "string" || !secret.startsWith("ed25519:")) throw new KeyFileError("the file has no ed25519 private key");
  let pair: KeyPair;
  try {
    pair = KeyPair.fromString(secret as KeyPairString);
  } catch {
    throw new KeyFileError("the private key in the file is malformed");
  }
  const publicKey = pair.getPublicKey().toString();
  if (raw.public_key !== undefined && raw.public_key !== publicKey) throw new KeyFileError("the file's public key doesn't match its private key");
  const accountId = typeof raw.account_id === "string" && isAccountId(raw.account_id) ? raw.account_id : null;
  return { accountId, publicKey, secret: secret as KeyPairString };
}

/**
 * Why `key` can't sign for `account`, or undefined if it can. A claim attaches a deposit, which
 * only a full-access key may do; `add_heir` gives the heir's account exactly that.
 */
export async function keyProblem(key: KeyFile, account: string, lookup = viewAccessKey): Promise<string | undefined> {
  if (key.accountId && key.accountId !== account) return `this key file is for ${key.accountId}, not ${account}`;
  const permission = await lookup(account, key.publicKey);
  if (permission === null) return `this key isn't one of ${account}'s keys`;
  if (permission !== "FullAccess") return "this is a function-call key; a claim needs the account's full-access key";
  return undefined;
}
