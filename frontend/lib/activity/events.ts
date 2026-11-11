import { MOCK_USDC } from "../near/config.ts";
import { formatNear, formatUnits, nsToMs } from "../near/format.ts";

/**
 * An estate's activity, from its own event logs. FastNEAR's Transactions API lists the
 * transactions that touched the estate and returns each with its receipts; the estate logs a
 * NEP-297 `herit` event for everything that changes it. Only receipts the estate itself executed
 * count, so a factory's or a token's logs never pass as the estate's, and only receipts that
 * succeeded: a failed one keeps its logs although everything it did was rolled back.
 */

export type AccountTx = { transaction_hash: string; tx_block_timestamp: string; is_success?: boolean };
export type FastTx = {
  transaction: { hash: string; signer_id: string };
  receipts: { receipt: { receiver_id: string }; execution_outcome: { outcome: { logs: string[]; status?: Record<string, unknown> } } }[];
};

export type Activity = {
  event: string;
  data: Record<string, unknown>;
  txHash: string;
  /** Unix ms of the transaction's block. */
  at: number;
  /** Who signed the transaction. */
  by: string;
};

export function parseActivity(estate: string, accountTxs: readonly AccountTx[], txs: readonly FastTx[]): Activity[] {
  const times = new Map(accountTxs.map((t) => [t.transaction_hash, nsToMs(t.tx_block_timestamp)]));
  const out: Activity[] = [];
  for (const tx of txs) {
    const at = times.get(tx.transaction.hash);
    if (at === undefined) continue;
    for (const r of tx.receipts) {
      if (r.receipt.receiver_id !== estate) continue;
      if (r.execution_outcome.outcome.status && "Failure" in r.execution_outcome.outcome.status) continue;
      for (const log of r.execution_outcome.outcome.logs) {
        if (!log.startsWith("EVENT_JSON:")) continue;
        try {
          const e = JSON.parse(log.slice("EVENT_JSON:".length));
          if (e.standard !== "herit" || typeof e.event !== "string") continue;
          out.push({ event: e.event, data: (Array.isArray(e.data) ? e.data[0] : e.data) ?? {}, txHash: tx.transaction.hash, at, by: tx.transaction.signer_id });
        } catch {
          // not JSON: not ours
        }
      }
    }
  }
  // Newest first; within one transaction, the order the estate logged them.
  return out.map((a, i) => ({ a, i })).sort((x, y) => y.a.at - x.a.at || x.i - y.i).map(({ a }) => a);
}

function amount(asset: unknown, raw: unknown): string {
  const units = typeof raw === "string" && /^\d+$/.test(raw) ? BigInt(raw) : BigInt(0);
  if (asset === "near" || asset === null) return `${formatNear(units)} NEAR`;
  if (asset === MOCK_USDC.id) return `${formatUnits(units, MOCK_USDC.decimals)} ${MOCK_USDC.symbol}`;
  return `${units} of ${String(asset)}`;
}

const bps = (v: unknown) => `${Number(v) / 100}%`;
const mins = (s: unknown) => {
  const n = Number(s);
  return n % 86_400 === 0 ? `${n / 86_400}d` : n % 3_600 === 0 ? `${n / 3_600}h` : `${Math.round(n / 60)} min`;
};

/** One line a person can read, and a tone for its dot. */
export function describe(a: Pick<Activity, "event" | "data">): { text: string; tone: "good" | "warn" | "bad" | "plain" } {
  const d = a.data;
  switch (a.event) {
    case "estate_opened": return { text: `estate opened by ${d.grantor}`, tone: "plain" };
    case "configured": return { text: `clock set: check in every ${mins(d.interval_sec)}, then ${mins(d.grace_sec)} grace`, tone: "plain" };
    case "checked_in": return { text: "the grantor checked in with a Selfie Check", tone: "good" };
    case "entered_grace": return { text: "a check-in was missed: grace started", tone: "warn" };
    case "unlocked": return { text: `the estate unlocked for its ${d.heirs} heir${d.heirs === 1 ? "" : "s"}`, tone: "bad" };
    case "snapshotted": return { text: `balances frozen for the claims (${d.assets} asset${d.assets === 1 ? "" : "s"})`, tone: "plain" };
    case "heir_added": return { text: `heir ${d.label} added: ${d.account}`, tone: "plain" };
    case "heir_removed": return { text: `heir ${d.label} removed`, tone: "plain" };
    case "heir_failed": return { text: `heir ${d.label} couldn't be created; refunded`, tone: "bad" };
    case "share_set": return { text: `${d.label}'s share of ${d.asset === "near" ? "NEAR" : d.asset === MOCK_USDC.id ? MOCK_USDC.symbol : d.asset} set to ${bps(d.bps)}`, tone: "plain" };
    case "deposited": return { text: `${amount(d.asset, d.amount)} deposited`, tone: "good" };
    case "token_added": return { text: `${d.token === MOCK_USDC.id ? MOCK_USDC.symbol : d.token} added to the vault`, tone: "plain" };
    case "token_failed": return { text: `${d.token} couldn't be added; refunded`, tone: "bad" };
    case "withdrawn": return { text: `${amount(d.asset, d.amount)} withdrawn by the grantor, with a Selfie Check`, tone: "plain" };
    case "withdraw_failed": return { text: `a withdrawal of ${amount(d.asset, d.amount)} failed and was credited back`, tone: "bad" };
    case "claim_attested": return { text: `${d.label} passed their Selfie Check to claim`, tone: "good" };
    case "claim_settled": return { text: `${d.label}'s claim sent ${d.transfers} payout${d.transfers === 1 ? "" : "s"}`, tone: "plain" };
    case "claimed": return { text: `${d.label} received ${amount(d.asset, d.amount)}`, tone: "good" };
    case "claim_failed": return { text: `a payout of ${amount(d.asset, d.amount)} to ${d.label} failed; they can claim it again`, tone: "bad" };
    default: return { text: a.event.replaceAll("_", " "), tone: "plain" };
  }
}
