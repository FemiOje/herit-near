import { estateId, isLabel, namedHeirProblem } from "../near/account.ts";
import { MIN_OPEN_DEPOSIT, calls, isPublicKey, type Call, type Signed } from "../near/calls.ts";
import { BPS, bpsToPercent, parseNear } from "../near/format.ts";
import { GRACE_BOUNDS, INTERVAL_BOUNDS } from "./clock.ts";

/**
 * Setup as one plan: an estate is only set up once it exists, its grantor's face is bound by a
 * Selfie Check, and it has heirs. The contract can't demand that in one call (the account has to
 * exist before anything can be sent to it), so the app does: the Selfie Check happens first,
 * since its attestation names the estate's id, which is known before the estate exists; then
 * every step goes to the wallet as one batch.
 *
 * The plan is worked out from what is on chain each time, so a batch that stopped halfway is
 * finished by sending only what's missing.
 */

/**
 * Who receives an heir's share: an account the heir already has, or a new sub-account of the
 * estate holding a key the heir made. Drafts saved before named heirs have no `kind`: a key.
 */
export type HeirTarget = { kind: "account"; accountId: string } | { kind?: "key"; publicKey: string };

export type HeirDraft = { label: string; relationship: string; bps: number } & HeirTarget;

export const isNamed = <T extends HeirTarget>(t: T): t is Extract<T, { kind: "account" }> => t.kind === "account";

export type SetupDraft = {
  label: string;
  intervalSec: number;
  graceSec: number;
  heirs: HeirDraft[];
  /** NEAR for the vault, as typed. "" or "0" for none yet. */
  depositNear: string;
};

/** What exists on chain already. */
export type SetupProgress = {
  exists: boolean;
  faceBound: boolean;
  /** Labels of heirs already added. */
  heirs: readonly string[];
  vaultNear: bigint;
};

export const NOTHING_YET: SetupProgress = { exists: false, faceBound: false, heirs: [], vaultNear: BigInt(0) };

/** What the estate account itself is opened with: its storage, plus the factory's fee. */
export const OPENING_DEPOSIT = parseNear("0.2");
export const MAX_HEIRS = 10;

export function vaultDeposit(draft: SetupDraft): bigint | null {
  const text = draft.depositNear.trim();
  if (text === "") return BigInt(0);
  try {
    return parseNear(text);
  } catch {
    return null;
  }
}

/**
 * Every reason the draft can't be sent yet, in words. Empty when it can. `factory` places the
 * estate, whose own sub-accounts can't be named heirs; `grantor` can't be their own heir.
 */
export function setupProblems(draft: SetupDraft, { factory, grantor = null }: { factory?: string; grantor?: string | null } = {}): string[] {
  const problems: string[] = [];
  if (!isLabel(draft.label)) problems.push("name the estate: lowercase letters, digits, - and _");
  if (draft.intervalSec < INTERVAL_BOUNDS.min || draft.intervalSec > INTERVAL_BOUNDS.max) problems.push("the check-in interval is out of bounds");
  if (draft.graceSec < GRACE_BOUNDS.min || draft.graceSec > GRACE_BOUNDS.max) problems.push("the grace period is out of bounds");
  if (draft.heirs.length === 0) problems.push("add at least one heir");
  if (draft.heirs.length > MAX_HEIRS) problems.push(`at most ${MAX_HEIRS} heirs`);
  const total = draft.heirs.reduce((sum, h) => sum + h.bps, 0);
  if (total > BPS) problems.push(`the heirs' shares add up to ${bpsToPercent(total)}, over 100%`);
  if (draft.heirs.some((h) => h.bps <= 0)) problems.push("every heir needs a share above 0%");
  const labels = draft.heirs.map((h) => h.label);
  if (new Set(labels).size !== labels.length) problems.push("two heirs share a label");
  const estate = factory ? estateId(draft.label, factory) : null;
  const named = draft.heirs.filter(isNamed);
  for (const h of draft.heirs) {
    if (!isLabel(h.label)) problems.push(`heir "${h.label}": the label is one lowercase segment`);
    if (isNamed(h)) {
      const others = named.filter((o) => o !== h).map((o) => o.accountId);
      const problem = namedHeirProblem(h.accountId, estate, grantor, others);
      if (problem) problems.push(`heir "${h.label}": ${problem}`);
    } else if (!isPublicKey(h.publicKey)) {
      problems.push(`heir "${h.label}": needs their public key, ed25519:…`);
    }
  }
  if (vaultDeposit(draft) === null) problems.push("the vault deposit isn't an amount");
  return problems;
}

/**
 * The transactions still needed, in order: open, check in, each heir not yet added, the vault
 * deposit. Each later step needs the ones before it, and a wallet sends a batch in order.
 */
export function setupPlan(factory: string, draft: SetupDraft, progress: SetupProgress, signed: Signed | null): Call[] {
  const id = estateId(draft.label, factory);
  const plan: Call[] = [];
  if (!progress.exists) plan.push(calls.openEstate(factory, draft.label, draft.intervalSec, draft.graceSec, OPENING_DEPOSIT));
  if (!progress.faceBound) {
    if (!signed) throw new Error("the first check-in needs a Selfie Check");
    plan.push(calls.checkIn(id, signed));
  }
  for (const h of draft.heirs) {
    if (progress.heirs.includes(h.label)) continue;
    const relationship = h.relationship || h.label;
    plan.push(isNamed(h) ? calls.addHeirAccount(id, h.label, h.accountId, relationship, h.bps) : calls.addHeir(id, h.label, h.publicKey, relationship, h.bps));
  }
  const deposit = vaultDeposit(draft) ?? BigInt(0);
  if (deposit > BigInt(0) && progress.vaultNear === BigInt(0)) plan.push(calls.depositNear(id, deposit));
  return plan;
}

/** Done means a bound face and at least one heir. */
export function isSetUp(progress: Pick<SetupProgress, "exists" | "faceBound" | "heirs">): boolean {
  return progress.exists && progress.faceBound && progress.heirs.length > 0;
}

// Kept in step with the factory's floor, which the opening deposit must clear.
if (OPENING_DEPOSIT < MIN_OPEN_DEPOSIT) throw new Error("OPENING_DEPOSIT is below the factory's minimum");
