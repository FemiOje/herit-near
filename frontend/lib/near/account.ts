/** NEAR account ids and how Herit's accounts relate: factory → estate → heir. */

/** NEAR's account id rule: 2–64 characters, lowercase segments joined by `.`. */
const ACCOUNT_ID = /^(([a-z\d]+[-_])*[a-z\d]+\.)*([a-z\d]+[-_])*[a-z\d]+$/;

export function isAccountId(value: string): boolean {
  return value.length >= 2 && value.length <= 64 && ACCOUNT_ID.test(value);
}

/** One segment, as the factory and the estate accept for estate and heir labels. */
export function isLabel(value: string): boolean {
  return /^[a-z0-9_-]+$/.test(value) && isAccountId(value);
}

/** `alice` → `alice.herit-near.testnet`. */
export function estateId(label: string, factory: string): string {
  return `${label}.${factory}`;
}

/**
 * The estate an heir account belongs to: its id minus the first segment.
 * `son.alice.herit-near.testnet` → `alice.herit-near.testnet`. Null if it isn't under `factory`.
 */
export function estateOfHeir(heirAccount: string, factory: string): string | null {
  const dot = heirAccount.indexOf(".");
  if (dot < 0) return null;
  const estate = heirAccount.slice(dot + 1);
  return estate.endsWith(`.${factory}`) && estate.split(".").length === factory.split(".").length + 1
    ? estate
    : null;
}

/** The first segment: an estate's or an heir's label. */
export function labelOf(accountId: string): string {
  return accountId.split(".")[0];
}
