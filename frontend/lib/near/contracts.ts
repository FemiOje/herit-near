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
  tokensOf: () => view<Asset[]>(id, "tokens_of"),
  balanceOf: (asset: Asset) => view<string>(id, "balance_of", { asset }),
  snapshotOf: (asset: Asset) => view<string>(id, "snapshot_of", { asset }),
  snapshotTaken: () => view<boolean>(id, "snapshot_taken"),
  shareOf: (label: string, asset: Asset) => view<number>(id, "share_of", { label, asset }),
  totalShareOf: (asset: Asset) => view<number>(id, "total_share_of", { asset }),
  claimableAll: (label: string) => view<[Asset, string][]>(id, "claimable_all", { label }),
  hasClaimed: (label: string, asset: Asset) => view<boolean>(id, "has_claimed", { label, asset }),
  nftToken: (label: string) => view<NftToken | null>(id, "nft_token", { token_id: label }),
});

/** The credential's state, from its `extra` JSON. */
export function credentialState(token: NftToken | null): CredentialState | null {
  try {
    const state = JSON.parse(token?.metadata?.extra ?? "{}").state;
    return ["locked", "grace", "claimable", "claimed"].includes(state) ? state : null;
  } catch {
    return null;
  }
}
