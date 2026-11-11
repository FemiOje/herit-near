"use client";

import { useQueries, useQuery } from "@tanstack/react-query";

import { assetKey } from "@/lib/estate/use-estate";
import { isAccountId } from "@/lib/near/account";
import { FACTORY } from "@/lib/near/config";
import { estate, factory, type Asset } from "@/lib/near/contracts";
import { findSlots, type SlotReaders } from "./slots";

const chainReaders: SlotReaders = {
  estates: (from, limit) => factory.estates(from, limit),
  heirOf: (id, label) => estate(id).heirOf(label),
  credentialsOf: (id, account) => estate(id).nftTokensForOwner(account),
};

/** Every heir slot `account` holds, from the chain. */
export function useHeirSlots(account: string | null) {
  return useQuery({
    queryKey: ["heir_slots", account],
    enabled: account !== null && isAccountId(account),
    queryFn: () => findSlots(account!, FACTORY, chainReaders),
    staleTime: 60_000,
  });
}

/** One heir's share, paid mark and the snapshot for each asset. Polls, so a claim shows up. */
export function useHeirAssets(id: string, label: string, assets: readonly Asset[]) {
  const e = estate(id);
  const snapshotTaken = useQuery({ queryKey: ["snapshot_taken", id], queryFn: () => e.snapshotTaken(), refetchInterval: 15_000 });
  const per = useQueries({
    queries: assets.map((asset) => ({
      queryKey: ["heir_asset", id, label, assetKey(asset)],
      refetchInterval: 15_000,
      queryFn: async () => {
        const [bps, paid, snapshot] = await Promise.all([e.shareOf(label, asset), e.hasClaimed(label, asset), e.snapshotOf(asset)]);
        return { bps, paid, snapshot: BigInt(snapshot) };
      },
    })),
  });
  return { snapshotTaken: snapshotTaken.data, per: per.map((q) => q.data) };
}
