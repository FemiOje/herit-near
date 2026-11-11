"use client";

import { useQuery } from "@tanstack/react-query";

import { isAccountId } from "./account";
import { viewAccount } from "./rpc";

/** An account as the chain has it, or null if it doesn't exist. Disabled for a malformed id. */
export function useAccountView(id: string | null) {
  return useQuery({
    queryKey: ["view_account", id],
    enabled: id !== null && isAccountId(id),
    queryFn: () => viewAccount(id!),
    staleTime: 30_000,
  });
}
