"use client";

import { useQuery } from "@tanstack/react-query";

import { factory, type EstateEntry } from "@/lib/near/contracts";

/**
 * The newest `limit` estates the factory has opened, newest first. The factory keeps every estate
 * in opening order (`estates`), so the newest are read from the end.
 */
export function useNewestEstates(limit: number) {
  return useQuery({
    queryKey: ["newest_estates", limit],
    refetchInterval: 30_000,
    queryFn: async (): Promise<{ total: number; estates: EstateEntry[] }> => {
      const total = await factory.estatesCount();
      const from = Math.max(0, total - limit);
      const estates = total === 0 ? [] : await factory.estates(from, total - from);
      return { total, estates: estates.reverse() };
    },
  });
}
