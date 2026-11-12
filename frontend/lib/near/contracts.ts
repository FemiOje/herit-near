import { FACTORY } from "./config.ts";
import { view } from "./rpc.ts";

/** Typed reads of the factory and an estate, matching the contracts' JSON. Amounts stay strings. */

// ---------- factory ----------

export type EstateEntry = { account_id: string; grantor: string; opened_at_ns: string };
export type FactoryConfig = { estate_code_hash: string; attestor_pk: string };

export const factory = {
  estatesOf: (grantor: string) => view<string[]>(FACTORY, "estates_of", { grantor }),
  estates: (fromIndex: number, limit: number) =>
    view<EstateEntry[]>(FACTORY, "estates", { from_index: fromIndex, limit }),
  estatesCount: () => view<number>(FACTORY, "estates_count"),
  config: () => view<FactoryConfig>(FACTORY, "config"),
};

// ---------- estate ----------

/** As the contract spells it. */
export type ContractStatus = "Active" | "Grace" | "Unlocked";
/** `null` is NEAR; otherwise a NEP-141 token account. */
export type Asset = string | null;

export type HeirView = {
  label: string;
  account_id: string;
  relationship: string;
  default_bps: number;
  created: boolean;
};

export type NftToken = {
  token_id: string;
  owner_id: string;
  metadata: {
    title: string | null;
    description: string | null;
    media: string | null;
    extra: string | null;
  } | null;
};

export type CredentialState = "locked" | "grace" | "claimable" | "claimed";

export const estate = (id: string) => ({
  statusOf: () => view<ContractStatus>(id, "status_of"),
  /** `[grace starts, unlock]` in ns; `["0","0"]` until the first check-in. */
  deadlinesOf: () => view<[string, string]>(id, "deadlines_of"),
  commitmentOf: () => view<string | null>(id, "commitment_of"),
  heirsOf: () => view<HeirView[]>(id, "heirs_of"),
  heirOf: (label: string) => view<HeirView | null>(id, "heir_of", { label }),
  tokensOf: () => view<Asset[]>(id, "tokens_of"),
  balanceOf: (asset: Asset) => view<string>(id, "balance_of", { asset }),
  snapshotOf: (asset: Asset) => view<string>(id, "snapshot_of", { asset }),
  snapshotTaken: () => view<boolean>(id, "snapshot_taken"),
  shareOf: (label: string, asset: Asset) => view<number>(id, "share_of", { label, asset }),
  totalShareOf: (asset: Asset) => view<number>(id, "total_share_of", { asset }),
  claimableAll: (label: string) => view<[Asset, string][]>(id, "claimable_all", { label }),
  hasClaimed: (label: string, asset: Asset) => view<boolean>(id, "has_claimed", { label, asset }),
  nftToken: (label: string) => view<NftToken | null>(id, "nft_token", { token_id: label }),
  /** Every heir's credential: at most MAX_HEIRS (10), so one page holds them all. */
  nftTokens: () => view<NftToken[]>(id, "nft_tokens", { from_index: "0", limit: 10 }),
  /** The credentials `account` holds here: one per heir slot it has in this estate. */
  nftTokensForOwner: (account: string) => view<NftToken[]>(id, "nft_tokens_for_owner", { account_id: account }),
});

// ---------- NEP-141 tokens ----------

export type FtMetadata = { symbol: string; decimals: number; name: string };

export const ft = (token: string) => ({
  metadata: () => view<FtMetadata>(token, "ft_metadata"),
  balanceOf: (accountId: string) => view<string>(token, "ft_balance_of", { account_id: accountId }),
  /** Null if `accountId` hasn't registered with the token, so it can't receive it yet. */
  storageBalanceOf: (accountId: string) =>
    view<{ total: string } | null>(token, "storage_balance_of", { account_id: accountId }),
});

/**
 * One picture for a whole estate's credentials. Unclaimed credentials all share the estate's state
 * (locked, grace or claimable), so the first unclaimed one stands for them; once every heir has
 * claimed, it's `claimed`. `counts` is for the tooltip. Null when there are no credentials yet.
 */
export function estateCredential(tokens: readonly NftToken[]): { state: CredentialState; media: string | null; counts: Partial<Record<CredentialState, number>> } | null {
  const counts: Partial<Record<CredentialState, number>> = {};
  let shown: NftToken | null = null;
  let shownState: CredentialState | null = null;
  for (const token of tokens) {
    const state = credentialState(token);
    if (!state) continue;
    counts[state] = (counts[state] ?? 0) + 1;
    if (!shownState || (shownState === "claimed" && state !== "claimed")) {
      shown = token;
      shownState = state;
    }
  }
  return shownState ? { state: shownState, media: shown?.metadata?.media ?? null, counts } : null;
}

/** The credential's state, from its `extra` JSON. */
export function credentialState(token: NftToken | null): CredentialState | null {
  try {
    const state = JSON.parse(token?.metadata?.extra ?? "{}").state;
    return ["locked", "grace", "claimable", "claimed"].includes(state) ? state : null;
  } catch {
    return null;
  }
}
