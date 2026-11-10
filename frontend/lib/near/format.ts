/** Amounts are strings of integers on the wire (yoctoNEAR, token units); `bigint` in the app. */

export const NEAR_DECIMALS = 24;
export const BPS = 10_000;

/** `1.5` NEAR → `"1500000000000000000000000"`. Refuses more decimals than the unit has. */
export function parseUnits(value: string, decimals: number): bigint {
  const trimmed = value.trim();
  if (!/^\d+(\.\d+)?$/.test(trimmed)) throw new Error(`not an amount: ${value}`);
  const [whole, fraction = ""] = trimmed.split(".");
  if (fraction.length > decimals) throw new Error(`more than ${decimals} decimals: ${value}`);
  return BigInt(whole + fraction.padEnd(decimals, "0"));
}

export const parseNear = (value: string) => parseUnits(value, NEAR_DECIMALS);

/**
 * `1500000000000000000000000n`, 24 → `"1.5"`. Truncates to `maxFraction` digits rather than
 * rounding, so it never shows more than there is.
 */
export function formatUnits(amount: bigint, decimals: number, maxFraction = 4): string {
  const negative = amount < BigInt(0);
  const digits = (negative ? -amount : amount).toString().padStart(decimals + 1, "0");
  const whole = digits.slice(0, digits.length - decimals);
  const fraction = digits.slice(digits.length - decimals, digits.length - decimals + maxFraction).replace(/0+$/, "");
  const shown = fraction ? `${whole}.${fraction}` : whole;
  if (shown === "0" && amount !== BigInt(0)) return `<0.${"0".repeat(maxFraction - 1)}1`;
  return negative ? `-${shown}` : shown;
}

export const formatNear = (yocto: bigint, maxFraction = 4) => formatUnits(yocto, NEAR_DECIMALS, maxFraction);

/** `6000` → `"60%"`, `2550` → `"25.5%"`. */
export function bpsToPercent(bps: number): string {
  const percent = bps / 100;
  return `${Number.isInteger(percent) ? percent : percent.toFixed(2).replace(/0+$/, "")}%`;
}

/** `amount × bps / 10,000`, rounded down as the estate's payout rounds. */
export function shareOf(amount: bigint, bps: number): bigint {
  return (amount * BigInt(bps)) / BigInt(BPS);
}

/** Nanoseconds since the epoch (as the contract keeps time) → milliseconds. */
export function nsToMs(ns: string | bigint): number {
  return Number(BigInt(ns) / BigInt(1_000_000));
}
