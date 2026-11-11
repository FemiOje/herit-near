"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import type { NearConnector } from "@hot-labs/near-connect";

import type { Call } from "./calls";
import { NETWORK, RPC_URLS } from "./config";
import { TxError, readOutcome, type CallResult } from "./outcome";

/**
 * The connected NEAR wallet, through HOT's `near-connect` (HOT, Meteor, Intear, MyNearWallet…).
 * The connector loads in the browser only: it uses the DOM, IndexedDB and sandboxed iframes.
 */

export type CallOptions = {
  /** Gas in Tgas. */
  tgas?: number;
  /** Attached deposit in yoctoNEAR. */
  deposit?: bigint;
};

export { TxError, panicMessage, type CallResult } from "./outcome";

type WalletState = {
  /** The signed-in account, or null. */
  accountId: string | null;
  /** True until the connector has loaded and restored any previous session. */
  loading: boolean;
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  /** Signs and sends one function call; throws `TxError` if it fails on chain. */
  call: (receiverId: string, method: string, args: object, options?: CallOptions) => Promise<CallResult>;
  /**
   * Several calls under one wallet approval, sent in order. Throws `TxError` naming the first
   * that failed; `TxError.completed` says how many landed before it.
   */
  callMany: (calls: readonly Call[]) => Promise<CallResult[]>;
};

const functionCall = (c: Pick<Call, "method" | "args" | "tgas" | "deposit">) => ({
  type: "FunctionCall" as const,
  params: {
    methodName: c.method,
    args: c.args,
    gas: (BigInt(c.tgas) * BigInt(1e12)).toString(),
    deposit: c.deposit.toString(),
  },
});

const WalletContext = createContext<WalletState | null>(null);

export function WalletProvider({ children }: { children: React.ReactNode }) {
  const connector = useRef<NearConnector | null>(null);
  const [accountId, setAccountId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const { NearConnector } = await import("@hot-labs/near-connect");
      const c = new NearConnector({
        network: NETWORK,
        providers: { [NETWORK]: [...RPC_URLS] },
        footerBranding: null,
      });
      connector.current = c;
      c.on("wallet:signIn", (event) => setAccountId(event.accounts[0]?.accountId ?? null));
      c.on("wallet:signOut", () => setAccountId(null));
      try {
        const { accounts } = await c.getConnectedWallet();
        if (!cancelled) setAccountId(accounts[0]?.accountId ?? null);
      } catch {
        // Nothing connected yet.
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
      connector.current?.removeAllListeners();
    };
  }, []);

  const connect = useCallback(async () => {
    if (!connector.current) return;
    await connector.current.connect();
  }, []);

  const disconnect = useCallback(async () => {
    await connector.current?.disconnect();
    setAccountId(null);
  }, []);

  const call = useCallback(
    async (receiverId: string, method: string, args: object, options: CallOptions = {}) => {
      if (!connector.current || !accountId) throw new TxError("connect a wallet first");
      const wallet = await connector.current.wallet();
      const outcome = await wallet.signAndSendTransaction({
        signerId: accountId,
        receiverId,
        actions: [
          functionCall({ method, args: args as Record<string, unknown>, tgas: options.tgas ?? 30, deposit: options.deposit ?? BigInt(0) }),
        ],
      });
      return readOutcome(outcome);
    },
    [accountId],
  );

  const callMany = useCallback(
    async (calls: readonly Call[]) => {
      if (!connector.current || !accountId) throw new TxError("connect a wallet first");
      const wallet = await connector.current.wallet();
      const outcomes = await wallet.signAndSendTransactions({
        signerId: accountId,
        transactions: calls.map((c) => ({ receiverId: c.receiverId, actions: [functionCall(c)] })),
      });
      const results: CallResult[] = [];
      for (const outcome of outcomes) {
        try {
          results.push(readOutcome(outcome));
        } catch (error) {
          if (error instanceof TxError) throw new TxError(error.message, error.txHash, results.length);
          throw error;
        }
      }
      return results;
    },
    [accountId],
  );

  const value = useMemo(
    () => ({ accountId, loading, connect, disconnect, call, callMany }),
    [accountId, loading, connect, disconnect, call, callMany],
  );
  return <WalletContext.Provider value={value}>{children}</WalletContext.Provider>;
}

export function useWallet(): WalletState {
  const state = useContext(WalletContext);
  if (!state) throw new Error("useWallet needs a WalletProvider");
  return state;
}
