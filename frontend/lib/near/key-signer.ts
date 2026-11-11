import { Account, FailoverRpcProvider, JsonRpcProvider, teraToGas } from "near-api-js";

import type { Call } from "./calls.ts";
import { RPC_URLS } from "./config.ts";
import type { KeyFile } from "./key-file.ts";
import { TxError, panicMessage, readOutcome, type CallResult } from "./outcome.ts";

/** Signs and sends calls as `accountId` with a key held in memory, without a wallet. */
export type KeySigner = { accountId: string; call: (c: Call) => Promise<CallResult> };

/**
 * A signer for a key heir. Signing happens here, in the browser; the RPC only ever sees the
 * signed transaction. Waits for the transaction's own execution, like a wallet does.
 */
export function keySigner(accountId: string, key: KeyFile, urls: readonly string[] = RPC_URLS): KeySigner {
  const provider = new FailoverRpcProvider(urls.map((url) => new JsonRpcProvider({ url })));
  const account = new Account(accountId, provider, key.secret);
  return {
    accountId,
    call: async (c) => {
      try {
        const outcome = await account.callFunctionRaw({
          contractId: c.receiverId,
          methodName: c.method,
          args: c.args,
          gas: teraToGas(c.tgas),
          deposit: c.deposit,
          waitUntil: "EXECUTED_OPTIMISTIC",
        });
        return readOutcome(outcome);
      } catch (error) {
        if (error instanceof TxError) throw error;
        // near-api-js throws on a failed transaction; keep the contract's own words.
        throw new TxError(panicMessage(error instanceof Error ? error.message : String(error)));
      }
    },
  };
}
