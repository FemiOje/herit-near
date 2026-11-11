"use client";

import { useCallback, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import type { Call } from "./calls";
import type { KeySigner } from "./key-signer";
import { TxError, useWallet, type CallResult } from "./wallet";

export type TxPhase = "idle" | "signing" | "success" | "error";
export type TxState = {
  phase: TxPhase;
  hash?: string;
  error?: string;
  /** In a batch that failed: how many transactions landed first. */
  completed?: number;
};

/**
 * Sends one call through the wallet (or `signer`, a key heir's key file) and tracks it. On success every read is refetched, now and
 * again a moment later, since an RPC node can trail the one the wallet sent through.
 */
export function useTx(signer?: KeySigner | null) {
  const wallet = useWallet();
  const accountId = signer ? signer.accountId : wallet.accountId;
  const queryClient = useQueryClient();
  const [state, setState] = useState<TxState>({ phase: "idle" });

  const refetch = useCallback(() => {
    void queryClient.invalidateQueries();
    setTimeout(() => void queryClient.invalidateQueries(), 2_500);
  }, [queryClient]);

  /** One call, or several under one approval. Returns the last result, or null on failure. */
  const send = useCallback(
    async (c: Call | readonly Call[]): Promise<CallResult | null> => {
      setState({ phase: "signing" });
      try {
        const list: readonly Call[] = Array.isArray(c) ? c : [c as Call];
        const results = signer
          ? await sendEach(signer, list)
          : Array.isArray(c)
            ? await wallet.callMany(c)
            : [await wallet.call(list[0].receiverId, list[0].method, list[0].args, { tgas: list[0].tgas, deposit: list[0].deposit })];
        const last = results[results.length - 1];
        setState({ phase: "success", hash: last?.txHash });
        refetch();
        return last ?? null;
      } catch (error) {
        setState({
          phase: "error",
          hash: error instanceof TxError ? error.txHash : undefined,
          completed: error instanceof TxError ? error.completed : undefined,
          error: describe(error),
        });
        refetch(); // part of a batch may have landed
        return null;
      }
    },
    [signer, wallet, refetch],
  );

  const reset = useCallback(() => setState({ phase: "idle" }), []);
  const busy = state.phase === "signing";
  const blocked = accountId ? undefined : "connect a wallet first";
  return { ...state, send, reset, busy, blocked };
}

/** A key signer sends one at a time, in order; a failure says how many landed first. */
async function sendEach(signer: KeySigner, calls: readonly Call[]): Promise<CallResult[]> {
  const results: CallResult[] = [];
  for (const c of calls) {
    try {
      results.push(await signer.call(c));
    } catch (error) {
      if (error instanceof TxError) throw new TxError(error.message, error.txHash, results.length);
      throw error;
    }
  }
  return results;
}

/** Wallets reject in many shapes; this keeps the words a person can act on. */
function describe(error: unknown): string {
  const message = error instanceof Error ? error.message : typeof error === "string" ? error : JSON.stringify(error);
  if (/reject|cancel|denied|closed/i.test(message)) return "the wallet request was cancelled";
  if (/NotEnoughBalance|LackBalanceForState|not enough/i.test(message)) {
    return "not enough NEAR in this account for the deposit and the gas";
  }
  return message || "the transaction failed";
}
