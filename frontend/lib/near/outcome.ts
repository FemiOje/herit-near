/** What a sent transaction came to, read the same way for a wallet and for a key signer. */

export type CallResult = { txHash: string; value: unknown };

export class TxError extends Error {
  readonly txHash?: string;
  /** In a batch: how many transactions landed before this one failed. */
  readonly completed: number;

  // Fields spelled out, not constructor shorthand: Node's type stripping runs this file in tests.
  constructor(message: string, txHash?: string, completed = 0) {
    super(message);
    this.txHash = txHash;
    this.completed = completed;
  }
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
export function readOutcome(raw: unknown): CallResult {
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
  const text = typeof failure === "string" ? failure : JSON.stringify(failure);
  const match = text.match(/Smart contract panicked: ([^"\\]+)/);
  return match ? match[1] : text;
}
