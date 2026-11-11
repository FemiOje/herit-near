import { TOKEN_REGISTRATION } from "../near/calls.ts";
import { parseNear } from "../near/format.ts";

/**
 * What a claim costs and what the heir's account must hold to send it, from the estate's own
 * constants (`herit-estate/src/lib.rs`):
 * - gas: the call itself, plus per NEAR payout a transfer and its `on_paid` callback (5 Tgas),
 *   and per token payout a registration and a transfer (`TOKEN_CALL_GAS`, 10 Tgas each) and the
 *   callback (`PAYOUT_CALLBACK_GAS`, 5 Tgas); plus headroom;
 * - deposit: the heir pays for what the claim writes (their commitment, one paid mark per asset)
 *   and, per token, their registration with it (`TOKEN_REGISTRATION`) and the 1 yoctoNEAR
 *   `ft_transfer` needs. The estate refunds the surplus;
 * - held: NEAR reserves the attached gas up front at a pessimistic price, about 0.1 NEAR per
 *   100 Tgas (Checkpoint 10 found this on testnet), so the signer must hold that and the deposit.
 *   Almost all of it comes back.
 */

const BASE_TGAS = 15;
const NEAR_PAYOUT_TGAS = 5;
const TOKEN_PAYOUT_TGAS = 25;
const HEADROOM_TGAS = 10;
const MAX_TGAS = 300;

const RECORDS = parseNear("0.005");
const PER_ASSET_RECORD = parseNear("0.002");
/** What NEAR holds per Tgas attached: 0.1 NEAR per 100 Tgas. */
export const HELD_PER_TGAS = parseNear("0.001");
/** What one byte of an account's storage locks. */
export const BYTE_COST = BigInt(10) ** BigInt(19);

export type ClaimCost = { tgas: number; deposit: bigint; held: bigint };

/** `nearPayouts` is 0 or 1; `tokenPayouts` counts the tokens with something to pay. */
export function claimCost(nearPayouts: number, tokenPayouts: number): ClaimCost {
  const raw = BASE_TGAS + NEAR_PAYOUT_TGAS * nearPayouts + TOKEN_PAYOUT_TGAS * tokenPayouts + HEADROOM_TGAS;
  const tgas = Math.min(MAX_TGAS, Math.ceil(raw / 5) * 5);
  const assets = BigInt(nearPayouts + tokenPayouts);
  const tokens = BigInt(tokenPayouts);
  const deposit = RECORDS + PER_ASSET_RECORD * assets + (TOKEN_REGISTRATION + BigInt(1)) * tokens;
  return { tgas, deposit, held: deposit + HELD_PER_TGAS * BigInt(tgas) };
}

/** What an account can spend: its balance less what its storage locks. */
export function spendable(account: { amount: string; storage_usage: number }): bigint {
  const free = BigInt(account.amount) - BigInt(account.storage_usage) * BYTE_COST;
  return free > BigInt(0) ? free : BigInt(0);
}
