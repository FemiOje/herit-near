import { estateOfHeir, labelOf } from "../near/account.ts";
import type { EstateEntry, HeirView, NftToken } from "../near/contracts.ts";

/** One place an account inherits: the heir `label` of `estate`. */
export type HeirSlot = { estate: string; label: string; account: string; relationship: string };

/** The chain reads discovery needs, passed in so the tests can stand in for the chain. */
export type SlotReaders = {
  estates: (fromIndex: number, limit: number) => Promise<EstateEntry[]>;
  heirOf: (estate: string, label: string) => Promise<HeirView | null>;
  credentialsOf: (estate: string, account: string) => Promise<NftToken[]>;
};

const relationshipOf = (token: NftToken): string => {
  try {
    return JSON.parse(token.metadata?.extra ?? "{}").relationship ?? token.token_id;
  } catch {
    return token.token_id;
  }
};

/**
 * Every heir slot `account` holds among the factory's estates. Two ways in:
 * - a key heir's account is `<label>.<estate>`, so its estate is in its name;
 * - a named heir can be any account, so every estate is asked for the credentials it holds
 *   (`nft_tokens_for_owner`), a page at a time. Fine for a testnet's dozens of estates; an
 *   indexer would replace it at scale.
 * An estate that can't be read is skipped, not fatal: one bad estate shouldn't hide the rest.
 */
export async function findSlots(
  account: string,
  factory: string,
  read: SlotReaders,
  { pageSize = 50, maxEstates = 1_000, concurrency = 8 } = {},
): Promise<HeirSlot[]> {
  const found = new Map<string, HeirSlot>();
  const add = (slot: HeirSlot) => found.set(`${slot.estate}#${slot.label}`, slot);

  const own = estateOfHeir(account, factory);
  if (own) {
    const heir = await read.heirOf(own, labelOf(account)).catch(() => null);
    if (heir?.account_id === account && heir.created) add({ estate: own, label: heir.label, account, relationship: heir.relationship });
  }

  for (let from = 0; from < maxEstates; from += pageSize) {
    const page = await read.estates(from, pageSize);
    for (let i = 0; i < page.length; i += concurrency) {
      const batch = page.slice(i, i + concurrency);
      const held = await Promise.all(batch.map((e) => read.credentialsOf(e.account_id, account).catch(() => [])));
      batch.forEach((e, j) => {
        for (const token of held[j]) add({ estate: e.account_id, label: token.token_id, account, relationship: relationshipOf(token) });
      });
    }
    if (page.length < pageSize) break;
  }
  return [...found.values()];
}
