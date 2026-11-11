import type { EstateStatus } from "./index.ts";

/** The check-in clock, from the estate's header and the local time. Draws it; never decides it. */

export const MINUTE = 60;
export const HOUR = 60 * MINUTE;
export const DAY = 24 * HOUR;

/** The contract's bounds (`validate_timers`). */
export const INTERVAL_BOUNDS = { min: 2 * MINUTE, max: 90 * DAY } as const;
export const GRACE_BOUNDS = { min: MINUTE, max: 14 * DAY } as const;

export type Preset = { label: string; note: string; seconds: number };

/** Minutes as well as months, so the whole loop can be shown live. */
export const INTERVAL_PRESETS: readonly Preset[] = [
  { label: "2 minutes", note: "demo speed", seconds: 2 * MINUTE },
  { label: "1 hour", note: "", seconds: HOUR },
  { label: "30 days", note: "realistic", seconds: 30 * DAY },
  { label: "90 days", note: "", seconds: 90 * DAY },
];

export const GRACE_PRESETS: readonly Preset[] = [
  { label: "1 minute", note: "demo speed", seconds: MINUTE },
  { label: "30 minutes", note: "", seconds: 30 * MINUTE },
  { label: "7 days", note: "realistic", seconds: 7 * DAY },
  { label: "14 days", note: "", seconds: 14 * DAY },
];

export type Clock = { intervalSec: number; graceSec: number; lastCheckInNs: bigint };

const NS_PER_MS = BigInt(1_000_000);

/** When the check-in window closes (grace starts), in ms; null before the first check-in. */
export function graceStartsAt(clock: Clock): number | null {
  if (clock.lastCheckInNs === BigInt(0)) return null;
  return Number(clock.lastCheckInNs / NS_PER_MS) + clock.intervalSec * 1000;
}

/** When the heirs can claim, in ms; null before the first check-in. */
export function unlocksAt(clock: Clock): number | null {
  const grace = graceStartsAt(clock);
  return grace === null ? null : grace + clock.graceSec * 1000;
}

export function lastCheckInAt(clock: Clock): number | null {
  return clock.lastCheckInNs === BigInt(0) ? null : Number(clock.lastCheckInNs / NS_PER_MS);
}

/**
 * What the ring counts down to and how much of that phase is spent (0 to 1). In Active, the
 * window's close; in Grace, the unlock. Unlocked is spent; not started is empty.
 */
export function phase(status: EstateStatus, clock: Clock, nowMs: number): { remainingSec: number; totalSec: number; progress: number } {
  const graceAt = graceStartsAt(clock);
  if (status === "unlocked") return { remainingSec: 0, totalSec: clock.graceSec, progress: 1 };
  if (graceAt === null) return { remainingSec: clock.intervalSec, totalSec: clock.intervalSec, progress: 0 };
  const [target, totalSec] = status === "grace" ? [graceAt + clock.graceSec * 1000, clock.graceSec] : [graceAt, clock.intervalSec];
  const remainingSec = Math.max(0, Math.ceil((target - nowMs) / 1000));
  return { remainingSec, totalSec, progress: totalSec > 0 ? 1 - remainingSec / totalSec : 1 };
}

const pad = (n: number) => String(n).padStart(2, "0");
const plural = (n: number, unit: string) => `${n} ${unit}${n === 1 ? "" : "s"}`;

/** `3d 04h`, `02h 15m`, `04:09`. */
export function formatCountdown(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const d = Math.floor(s / DAY);
  const h = Math.floor((s % DAY) / HOUR);
  const m = Math.floor((s % HOUR) / MINUTE);
  if (d > 0) return `${d}d ${pad(h)}h`;
  if (h > 0) return `${pad(h)}h ${pad(m)}m`;
  return `${pad(m)}:${pad(s % MINUTE)}`;
}

/** `30 days`, `2 mins`, `90 secs`. */
export function formatDuration(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  if (s === 0) return "not set";
  if (s % DAY === 0) return plural(s / DAY, "day");
  if (s % HOUR === 0) return plural(s / HOUR, "hour");
  if (s % MINUTE === 0) return plural(s / MINUTE, "min");
  return plural(s, "sec");
}

export function formatStamp(ms: number | null): string {
  return ms === null ? "—" : new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}
