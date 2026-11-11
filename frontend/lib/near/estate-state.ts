import { RPC_URLS } from "./config.ts";
import { ViewError } from "./rpc.ts";

/**
 * The estate's own header, read from its raw `STATE` record. The contract has no view for its
 * grantor or its timers, and the estate code is a published Global Contract, so it can't grow one.
 * Its state is Borsh, and the fields we need come first:
 *
 *   factory: String, grantor: String, attestor_pk: [u8; 32], grantor_commitment: Option<[u8; 32]>,
 *   check_in_interval_ns: u64, grace_ns: u64, last_check_in_ns: u64, …
 *
 * (`Estate` in near/contracts/herit-estate/src/lib.rs. Fields are only ever appended there.)
 */
export type EstateHeader = {
  factory: string;
  grantor: string;
  /** Whether a face is bound yet: the first check-in binds it. */
  faceBound: boolean;
  intervalSec: number;
  graceSec: number;
  /** 0 until the first check-in. */
  lastCheckInNs: bigint;
};

export function decodeHeader(bytes: Uint8Array): EstateHeader {
  const data = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let at = 0;
  const need = (n: number) => {
    if (at + n > bytes.length) throw new ViewError("estate state is shorter than its header");
  };
  const string = () => {
    need(4);
    const length = data.getUint32(at, true);
    at += 4;
    need(length);
    const value = new TextDecoder().decode(bytes.subarray(at, at + length));
    at += length;
    return value;
  };
  const u64 = () => {
    need(8);
    const value = data.getBigUint64(at, true);
    at += 8;
    return value;
  };

  const factory = string();
  const grantor = string();
  need(32);
  at += 32; // attestor_pk
  need(1);
  const faceBound = bytes[at] === 1;
  at += 1 + (faceBound ? 32 : 0);
  const intervalNs = u64();
  const graceNs = u64();
  const lastCheckInNs = u64();
  const NS = BigInt(1_000_000_000);
  return {
    factory,
    grantor,
    faceBound,
    intervalSec: Number(intervalNs / NS),
    graceSec: Number(graceNs / NS),
    lastCheckInNs,
  };
}

/** Reads and decodes `id`'s header. Tries each RPC in turn on a network failure. */
export async function estateHeader(
  id: string,
  urls: readonly string[] = RPC_URLS,
  fetcher: typeof fetch = fetch,
): Promise<EstateHeader> {
  const body = JSON.stringify({
    jsonrpc: "2.0",
    id: "herit",
    method: "query",
    params: {
      request_type: "view_state",
      finality: "optimistic",
      account_id: id,
      prefix_base64: btoa("STATE"),
    },
  });
  let lastNetworkError: unknown;
  for (const url of urls) {
    let reply: { result?: { values?: { key: string; value: string }[] }; error?: { message?: string } };
    try {
      const res = await fetcher(url, { method: "POST", headers: { "Content-Type": "application/json" }, body });
      if (!res.ok) throw new Error(`${url} answered ${res.status}`);
      reply = await res.json();
    } catch (error) {
      lastNetworkError = error;
      continue;
    }
    const record = reply.result?.values?.find((v) => atob(v.key) === "STATE");
    if (!record) throw new ViewError(`${id}: ${reply.error?.message ?? "not an estate"}`);
    return decodeHeader(Uint8Array.from(atob(record.value), (c) => c.charCodeAt(0)));
  }
  throw new ViewError(`no RPC answered: ${String(lastNetworkError)}`);
}
