"use client";

import { useQuery } from "@tanstack/react-query";

import type { Activity } from "./events";

/** The estate's events from `/api/activity`. Polls, as the rest of the screen does. */
export function useActivity(estate: string) {
  return useQuery({
    queryKey: ["activity", estate],
    refetchInterval: 20_000,
    queryFn: async (): Promise<{ events: Activity[]; more: boolean }> => {
      const res = await fetch(`/api/activity?estate=${encodeURIComponent(estate)}`);
      const body = await res.json();
      if (!res.ok) throw new Error(body.error ?? "couldn't read the activity");
      return body;
    },
  });
}
