import { RPC_URLS } from "./config.ts";

/** Read-only contract calls over JSON-RPC. No wallet, no keys: anyone can read any contract. */

export class ViewError extends Error {}

type RpcReply = {
  result?: { result?: number[]; error?: string };
  error?: { cause?: { name?: string; info?: unknown }; message?: string; data?: string };
};

/**
 * Calls `method` on `contractId` with JSON `args` and returns the parsed JSON result. Tries each
 * RPC in turn, but only on a network failure: a contract's own error is final.
 */
export async function view<T>(
  contractId: string,
  method: string,
  args: Record<string, unknown> = {},
  urls: readonly string[] = RPC_URLS,
  fetcher: typeof fetch = fetch,
): Promise<T> {
  const body = JSON.stringify({
    jsonrpc: "2.0",
    id: "herit",
    method: "query",
    params: {
      request_type: "call_function",
      finality: "optimistic",
      account_id: contractId,
      method_name: method,
      args_base64: toBase64(JSON.stringify(args)),
    },
  });
  let lastNetworkError: unknown;
  for (const url of urls) {
    let reply: RpcReply;
    try {
      const res = await fetcher(url, { method: "POST", headers: { "Content-Type": "application/json" }, body });
      if (!res.ok) throw new Error(`${url} answered ${res.status}`);
      reply = (await res.json()) as RpcReply;
    } catch (error) {
      lastNetworkError = error;
      continue;
    }
    if (reply.error || reply.result?.error) {
      throw new ViewError(
        `${contractId}.${method}: ${reply.result?.error ?? reply.error?.data ?? reply.error?.message ?? "failed"}`,
      );
    }
    const bytes = reply.result?.result ?? [];
    return JSON.parse(new TextDecoder().decode(new Uint8Array(bytes))) as T;
  }
  throw new ViewError(`no RPC answered: ${String(lastNetworkError)}`);
}

export type AccountView = { amount: string; locked: string; storage_usage: number; code_hash: string };

/**
 * An account's balance and storage, or null if it doesn't exist. Used to say whether an estate
 * label is free and how much NEAR a wallet holds.
 */
export async function viewAccount(
  accountId: string,
  urls: readonly string[] = RPC_URLS,
  fetcher: typeof fetch = fetch,
): Promise<AccountView | null> {
  const body = JSON.stringify({
    jsonrpc: "2.0",
    id: "herit",
    method: "query",
    params: { request_type: "view_account", finality: "optimistic", account_id: accountId },
  });
  let lastNetworkError: unknown;
  for (const url of urls) {
    let reply: { result?: AccountView; error?: { cause?: { name?: string }; message?: string } };
    try {
      const res = await fetcher(url, { method: "POST", headers: { "Content-Type": "application/json" }, body });
      if (!res.ok) throw new Error(`${url} answered ${res.status}`);
      reply = await res.json();
    } catch (error) {
      lastNetworkError = error;
      continue;
    }
    if (reply.error?.cause?.name === "UNKNOWN_ACCOUNT") return null;
    if (reply.error || !reply.result) throw new ViewError(`${accountId}: ${reply.error?.message ?? "no account view"}`);
    return reply.result;
  }
  throw new ViewError(`no RPC answered: ${String(lastNetworkError)}`);
}

function toBase64(text: string): string {
  return btoa(String.fromCharCode(...new TextEncoder().encode(text)));
}
