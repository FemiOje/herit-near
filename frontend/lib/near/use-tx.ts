"use client";

import { useCallback, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import type { Call } from "./calls";
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
 * Sends one call through the wallet and tracks it. On success every read is refetched, now and
 * again a moment later, since an RPC node can trail the one the wallet sent through.
 */
export function useTx() {
  const { call, callMany, accountId } = useWallet();
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
        const results = Array.isArray(c)
          ? await callMany(c)
          : [await call((c as Call).receiverId, (c as Call).method, (c as Call).args, { tgas: (c as Call).tgas, deposit: (c as Call).deposit })];
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
    [call, callMany, refetch],
  );

  const reset = useCallback(() => setState({ phase: "idle" }), []);
  const busy = state.phase === "signing";
  const blocked = accountId ? undefined : "connect a wallet first";
  return { ...state, send, reset, busy, blocked };
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
