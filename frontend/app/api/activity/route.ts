import { parseActivity, type AccountTx, type FastTx } from "@/lib/activity/events";
import { isAccountId } from "@/lib/near/account";
import { FACTORY, NETWORK } from "@/lib/near/config";

/**
 * GET /api/activity?estate=<id>: the estate's Herit events, newest first.
 *
 * No hackathon partner offers indexing, so this reads FastNEAR's Transactions API: the estate's
 * latest transactions, then their receipts, whose logs carry the estate's events. It runs on the
 * server so a key can be added later without reaching the browser; today the API needs none.
 * Only estates under our factory are served, so this isn't an open proxy.
 */

const BASE = NETWORK === "mainnet" ? "https://tx.main.fastnear.com" : "https://tx.test.fastnear.com";
const LIMIT = 50;
/** The API's batch limit isn't documented; 20 hashes per call stays well clear of it. */
const BATCH = 20;

async function post<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
    cache: "no-store",
    signal: AbortSignal.timeout(10_000),
  });
  if (!res.ok) throw new Error(`FastNEAR answered ${res.status}`);
  return res.json() as Promise<T>;
}

export async function GET(request: Request) {
  const estate = new URL(request.url).searchParams.get("estate") ?? "";
  if (!isAccountId(estate) || !estate.endsWith(`.${FACTORY}`) || estate.split(".").length !== FACTORY.split(".").length + 1) {
    return Response.json({ error: `estate must be an estate of ${FACTORY}` }, { status: 400 });
  }
  try {
    const { account_txs = [] } = await post<{ account_txs?: AccountTx[] }>("/v0/account", { account_id: estate, limit: LIMIT, desc: true });
    const hashes = account_txs.map((t) => t.transaction_hash);
    const txs: FastTx[] = [];
    for (let i = 0; i < hashes.length; i += BATCH) {
      const { transactions = [] } = await post<{ transactions?: FastTx[] }>("/v0/transactions", { tx_hashes: hashes.slice(i, i + BATCH) });
      txs.push(...transactions);
    }
    return Response.json({ events: parseActivity(estate, account_txs, txs), more: account_txs.length === LIMIT });
  } catch (error) {
    return Response.json({ error: `couldn't read the activity: ${error instanceof Error ? error.message : "unknown"}` }, { status: 502 });
  }
}
