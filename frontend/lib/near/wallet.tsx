"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import type { NearConnector } from "@hot-labs/near-connect";

import { NETWORK, RPC_URLS } from "./config";

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

export type CallResult = { txHash: string; value: unknown };

type WalletState = {
  /** The signed-in account, or null. */
  accountId: string | null;
  /** True until the connector has loaded and restored any previous session. */
  loading: boolean;
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  /** Signs and sends one function call; throws `TxError` if it fails on chain. */
  call: (receiverId: string, method: string, args: object, options?: CallOptions) => Promise<CallResult>;
};

export class TxError extends Error {
  constructor(message: string, readonly txHash?: string) {
    super(message);
  }
}

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
          {
            type: "FunctionCall",
            params: {
              methodName: method,
              args,
              gas: (BigInt(options.tgas ?? 30) * BigInt(1e12)).toString(),
              deposit: (options.deposit ?? BigInt(0)).toString(),
            },
          },
        ],
      });
      return readOutcome(outcome);
    },
    [accountId],
  );

  const value = useMemo(
    () => ({ accountId, loading, connect, disconnect, call }),
    [accountId, loading, connect, disconnect, call],
  );
  return <WalletContext.Provider value={value}>{children}</WalletContext.Provider>;
}

export function useWallet(): WalletState {
  const state = useContext(WalletContext);
  if (!state) throw new Error("useWallet needs a WalletProvider");
  return state;
}

type Outcome = {
  transaction?: { hash?: string };
  transaction_outcome?: { id?: string };
  status?: { SuccessValue?: string; Failure?: unknown } | string;
  receipts_outcome?: { outcome?: { status?: { Failure?: unknown } } }[];
};

/**
 * The transaction hash and the call's return value, or a `TxError` naming the contract's panic.
 * A receipt can fail after the transaction itself succeeded (a callback, a transfer); the estate
 * logs those itself, so only the main status counts as failure here.
 */
function readOutcome(raw: unknown): CallResult {
  const outcome = raw as Outcome;
  const txHash = outcome.transaction?.hash ?? outcome.transaction_outcome?.id ?? "";
  const status = outcome.status;
  if (status && typeof status === "object" && "Failure" in status && status.Failure) {
    throw new TxError(panicMessage(status.Failure), txHash);
  }
  const success = typeof status === "object" ? status?.SuccessValue : undefined;
  let value: unknown = null;
  if (success) {
    try {
      value = JSON.parse(atob(success));
    } catch {
      value = null;
    }
  }
  return { txHash, value };
}

/** `{ActionError: {kind: {FunctionCallError: {ExecutionError: "Smart contract panicked: x"}}}}` → `x`. */
export function panicMessage(failure: unknown): string {
  const text = JSON.stringify(failure);
  const match = text.match(/Smart contract panicked: ([^"\\]+)/);
  return match ? match[1] : text;
}
