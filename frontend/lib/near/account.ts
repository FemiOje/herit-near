/** NEAR account ids and how Herit's accounts relate: factory → estate → heir. */

/** NEAR's account id rule: 2–64 characters, lowercase segments joined by `.`. */
const ACCOUNT_ID = /^(([a-z\d]+[-_])*[a-z\d]+\.)*([a-z\d]+[-_])*[a-z\d]+$/;

export function isAccountId(value: string): boolean {
  return value.length >= 2 && value.length <= 64 && ACCOUNT_ID.test(value);
}

/**
 * One segment, as the factory and the estate accept for estate and heir labels. A segment may be a
 * single character (`a.alice.herit-near.testnet` is a valid account), so the 2-character minimum,
 * which is for whole account ids, doesn't apply.
 */
export function isLabel(value: string): boolean {
  return value.length <= 64 && /^([a-z\d]+[-_])*[a-z\d]+$/.test(value);
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

/**
 * Why `account` can't be named as an heir of `estate`, or undefined if it can. The estate's
 * `add_heir_account` refuses the same: the estate and anything under it (`add_heir` makes those),
 * the grantor, and an account that is already an heir. Whether the account exists is checked
 * separately, against the chain.
 */
export function namedHeirProblem(account: string, estate: string | null, grantor: string | null, taken: readonly string[]): string | undefined {
  if (!isAccountId(account)) return "a NEAR account, such as kay.testnet";
  if (estate !== null && (account === estate || account.endsWith(`.${estate}`))) return "that account is under this estate; use a key instead";
  if (grantor !== null && account === grantor) return "you can't be your own heir";
  if (taken.includes(account)) return "that account is already an heir";
  return undefined;
}

/** A 64-hex implicit account, or a 0x… one: it exists as soon as it holds NEAR, so it needn't exist yet. */
export function isImplicitAccount(account: string): boolean {
  return /^[0-9a-f]{64}$/.test(account) || /^0x[0-9a-f]{40}$/.test(account);
}
