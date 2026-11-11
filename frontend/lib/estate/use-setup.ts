"use client";

import { useCallback, useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { estate } from "@/lib/near/contracts";
import { estateHeader } from "@/lib/near/estate-state";
import { viewAccount } from "@/lib/near/rpc";
import { INTERVAL_PRESETS, GRACE_PRESETS } from "./clock";
import type { SetupDraft, SetupProgress } from "./setup-plan";

export const EMPTY_DRAFT: SetupDraft = {
  label: "",
  intervalSec: INTERVAL_PRESETS[0].seconds,
  graceSec: GRACE_PRESETS[0].seconds,
  heirs: [],
  depositNear: "1",
};

/**
 * The draft, kept in this browser per wallet, so a setup that stopped halfway (a closed tab, a
 * failed transaction) picks up where it was. Holds only public data: heirs' public keys, never
 * private ones.
 */
export function useSetupDraft(accountId: string | null) {
  const key = accountId ? `herit:setup:${accountId}` : null;
  const [draft, setDraft] = useState<SetupDraft>(EMPTY_DRAFT);
  const [loadedFor, setLoadedFor] = useState<string | null>(null);

  // Loading from storage is syncing with an external system, which is what effects are for.
  useEffect(() => {
    if (!key || loadedFor === key) return;
    let saved: SetupDraft | null = null;
    try {
      const raw = window.localStorage.getItem(key);
      saved = raw ? { ...EMPTY_DRAFT, ...JSON.parse(raw) } : null;
    } catch {
      saved = null;
    }
    // eslint-disable-next-line react-hooks/set-state-in-effect -- hydrating from localStorage
    setDraft(saved ?? { ...EMPTY_DRAFT, label: accountId!.split(".")[0].replace(/[^a-z0-9_-]/g, "").slice(0, 32) });
    setLoadedFor(key);
  }, [key, loadedFor, accountId]);

  const update = useCallback(
    (next: SetupDraft) => {
      setDraft(next);
      try {
        if (key) window.localStorage.setItem(key, JSON.stringify(next));
      } catch {
        // Storage off: the draft just won't survive a reload.
      }
    },
    [key],
  );

  const clear = useCallback(() => {
    try {
      if (key) window.localStorage.removeItem(key);
    } catch {
      // nothing to clear
    }
  }, [key]);

  return { draft, update, clear };
}

/** What's on chain for `id` already, and whose it is. Polls while a setup is running. */
export function useSetupProgress(id: string | null) {
  return useQuery({
    queryKey: ["setup_progress", id],
    enabled: id !== null,
    refetchInterval: 8_000,
    queryFn: async (): Promise<SetupProgress & { grantor: string | null; intervalSec?: number; graceSec?: number }> => {
      const account = await viewAccount(id!);
      if (!account) return { exists: false, faceBound: false, heirs: [], vaultNear: BigInt(0), grantor: null };
      const header = await estateHeader(id!).catch(() => null);
      if (!header) return { exists: true, faceBound: false, heirs: [], vaultNear: BigInt(0), grantor: null };
      const [heirs, vault] = await Promise.all([estate(id!).heirsOf(), estate(id!).balanceOf(null)]);
      return {
        exists: true,
        faceBound: header.faceBound,
        heirs: heirs.map((h) => h.label),
        vaultNear: BigInt(vault),
        grantor: header.grantor,
        intervalSec: header.intervalSec,
        graceSec: header.graceSec,
      };
    },
  });
}
