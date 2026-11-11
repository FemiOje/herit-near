"use client";

import { useCallback, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import type { Call } from "./calls";
import { TxError, useWallet, type CallResult } from "./wallet";

export type TxPhase = "idle" | "signing" | "success" | "error";
export type TxState = { phase: TxPhase; hash?: string; error?: string };

/**
 * Sends one call through the wallet and tracks it. On success every read is refetched, now and
 * again a moment later, since an RPC node can trail the one the wallet sent through.
 */
export function useTx() {
  const { call, accountId } = useWallet();
  const queryClient = useQueryClient();
  const [state, setState] = useState<TxState>({ phase: "idle" });

  const send = useCallback(
    async (c: Call): Promise<CallResult | null> => {
      setState({ phase: "signing" });
      try {
        const result = await call(c.receiverId, c.method, c.args, { tgas: c.tgas, deposit: c.deposit });
        setState({ phase: "success", hash: result.txHash });
        void queryClient.invalidateQueries();
        setTimeout(() => void queryClient.invalidateQueries(), 2_500);
        return result;
      } catch (error) {
        setState({
          phase: "error",
          hash: error instanceof TxError ? error.txHash : undefined,
          error: describe(error),
        });
        return null;
      }
    },
    [call, queryClient],
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
