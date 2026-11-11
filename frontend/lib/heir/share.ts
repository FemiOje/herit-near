import { BPS } from "../near/format.ts";

/**
 * Where an heir stands on one asset:
 * - `none`: no share of it;
 * - `expected`: before unlock, what they'd get if it unlocked now (the balance, which moves);
 * - `claimable`: unlocked and not yet paid: their share of the frozen snapshot;
 * - `paid`: paid out.
 */
export type LineState = "none" | "expected" | "claimable" | "paid";

export type ShareInput = {
  bps: number;
  balance: bigint;
  snapshot: bigint;
  snapshotTaken: boolean;
  paid: boolean;
  /** The estate's status says Unlocked, poked or not. */
  unlocked: boolean;
};

/**
 * The amount the contract's `payout_amount` would give: the snapshot once taken, else the
 * balance (which an unpoked unlock will freeze as is), times the share, rounded down.
 */
export function shareLine(i: ShareInput): { state: LineState; amount: bigint } {
  const ruler = i.snapshotTaken ? i.snapshot : i.balance;
  const amount = (ruler * BigInt(i.bps)) / BigInt(BPS);
  if (i.bps === 0) return { state: "none", amount: BigInt(0) };
  if (i.paid) return { state: "paid", amount };
  return { state: i.unlocked ? "claimable" : "expected", amount };
}

/** Whether a claim would do anything: unlocked, and something unpaid with an amount above 0. */
export function hasSomethingToClaim(lines: readonly { state: LineState; amount: bigint }[]): boolean {
  return lines.some((l) => l.state === "claimable" && l.amount > BigInt(0));
}
