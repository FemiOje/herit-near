"use client";

import { useQueries, useQuery } from "@tanstack/react-query";

import { MOCK_USDC } from "@/lib/near/config";
import { estate, ft, type Asset, type HeirView } from "@/lib/near/contracts";
import { estateHeader } from "@/lib/near/estate-state";
import { NEAR_DECIMALS } from "@/lib/near/format";
import { fromContract } from "./index";

/** How an asset reads on screen. */
export type AssetInfo = { asset: Asset; symbol: string; decimals: number };

export const NEAR_ASSET: AssetInfo = { asset: null, symbol: "NEAR", decimals: NEAR_DECIMALS };

export const assetKey = (asset: Asset) => asset ?? "near";

/** Everything the grantor screens read about one estate. Polls, so the clock keeps up. */
export function useEstate(id: string) {
  const e = estate(id);
  const header = useQuery({ queryKey: ["header", id], queryFn: () => estateHeader(id), refetchInterval: 15_000 });
  const status = useQuery({
    queryKey: ["status_of", id],
    queryFn: async () => fromContract(await e.statusOf()),
    refetchInterval: 10_000,
  });
  const heirs = useQuery({ queryKey: ["heirs_of", id], queryFn: () => e.heirsOf() });
  const tokens = useQuery({ queryKey: ["tokens_of", id], queryFn: () => e.tokensOf() });
  const assets = tokens.data ?? [];

  const balances = useQueries({
    queries: assets.map((asset) => ({
      queryKey: ["balance_of", id, assetKey(asset)],
      queryFn: async () => BigInt(await e.balanceOf(asset)),
    })),
  });
  const infos = useQueries({ queries: assets.map((asset) => assetInfoQuery(asset)) });

  const vault = assets.map((asset, i) => ({
    info: infos[i]?.data ?? { asset, symbol: asset ?? "NEAR", decimals: asset ? 0 : NEAR_DECIMALS },
    balance: balances[i]?.data,
  }));

  return { header, status, heirs, tokens, vault };
}

/** NEAR, or a token's symbol and decimals from its `ft_metadata`. */
export function assetInfoQuery(asset: Asset) {
  return {
    queryKey: ["asset_info", assetKey(asset)],
    staleTime: Infinity,
    queryFn: async (): Promise<AssetInfo> => {
      if (asset === null) return NEAR_ASSET;
      if (asset === MOCK_USDC.id) return { asset, symbol: MOCK_USDC.symbol, decimals: MOCK_USDC.decimals };
      const m = await ft(asset).metadata();
      return { asset, symbol: m.symbol, decimals: m.decimals };
    },
  };
}

/** Each heir's effective share of each asset, keyed `label` → `asset` → bps. */
export function useShares(id: string, heirs: readonly HeirView[], assets: readonly Asset[]) {
  const pairs = heirs.flatMap((h) => assets.map((asset) => ({ label: h.label, asset })));
  const results = useQueries({
    queries: pairs.map(({ label, asset }) => ({
      queryKey: ["share_of", id, label, assetKey(asset)],
      queryFn: () => estate(id).shareOf(label, asset),
    })),
  });
  const shares = new Map<string, Map<string, number>>();
  pairs.forEach(({ label, asset }, i) => {
    const bps = results[i]?.data;
    if (bps === undefined) return;
    if (!shares.has(label)) shares.set(label, new Map());
    shares.get(label)!.set(assetKey(asset), bps);
  });
  return shares;
}
