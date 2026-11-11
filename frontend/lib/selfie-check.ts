import type { RpContext } from "@worldcoin/idkit";

/**
 * World ID Selfie Check, shared by the browser and the routes.
 *
 * The World action is `herit:<estate label>`: it scopes the nullifier, so one human gets one
 * nullifier per estate. The OutLayer worker derives the same string from the estate's account id
 * and refuses a proof for any other action (near/attestor-worker/src/logic.rs).
 */

/**
 * What a check is for. The heir label travels as the proof's signal, not in the action. A
 * withdrawal shares the check-in's action on purpose: the same human gets the same nullifier, so
 * the commitment matches the face the first check-in bound.
 */
export type SelfieCheckPurpose =
  | { kind: "checkin"; estateLabel: string }
  | { kind: "withdraw"; estateLabel: string }
  | { kind: "claim"; estateLabel: string; heirLabel: string };

export const WORLD_ENVIRONMENTS = ["production", "staging", "sandbox"] as const;
export type WorldEnvironment = (typeof WORLD_ENVIRONMENTS)[number];

/** What `POST /api/worldid/sign` returns. */
export type SignResponse = {
  app_id: `app_${string}`;
  action: string;
  environment: WorldEnvironment;
  rp_context: RpContext;
};

export function worldAction(purpose: SelfieCheckPurpose): string {
  return `herit:${purpose.estateLabel}`;
}

/** A single NEAR account segment, the same rule the worker applies. */
export function isLabel(value: unknown): value is string {
  return typeof value === "string" && /^[a-z0-9_-]+$/.test(value);
}

/** The estate's account id under the testnet factory. */
export function estateAccountId(estateLabel: string): string {
  return `${estateLabel}.herit-near.testnet`;
}
