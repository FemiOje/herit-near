"use client";

import { useQuery } from "@tanstack/react-query";

import { IPFS_GATEWAY } from "@/lib/near/config";
import { credentialState, estate, estateCredential, type CredentialState } from "@/lib/near/contracts";

export const STATE_TONE: Record<CredentialState, string> = {
  locked: "bg-teal",
  grace: "bg-yellow",
  claimable: "bg-coral text-white",
  claimed: "bg-lavender",
};

/** One heir's credential, read live from the estate: a soulbound NEP-171 token computed from its state. */
export function useCredential(id: string, label: string, enabled = true) {
  const token = useQuery({
    queryKey: ["nft_token", id, label],
    queryFn: () => estate(id).nftToken(label),
    refetchInterval: 15_000,
    enabled,
  });
  return { token, state: credentialState(token.data ?? null), media: token.data?.metadata?.media ?? null };
}

/** Every heir's credential in an estate, and the one picture that stands for them all. */
export function useEstateCredentials(id: string) {
  const tokens = useQuery({ queryKey: ["nft_tokens", id], queryFn: () => estate(id).nftTokens(), refetchInterval: 30_000 });
  return { tokens, summary: tokens.data ? estateCredential(tokens.data) : null };
}

/** "1 claimable, 1 claimed": the states behind an estate's picture. */
export function describeCounts(counts: Partial<Record<CredentialState, number>>): string {
  return (Object.entries(counts) as [CredentialState, number][]).map(([state, n]) => `${n} ${state}`).join(", ");
}

/** The credential's picture: the IPFS image the contract chose for its state. */
export function CredentialImage({ media, state, size, pending = false }: { media: string | null; state: CredentialState | null; size: number; pending?: boolean }) {
  const box = { width: size, height: size };
  if (!media) {
    return (
      <div style={box} className="flex shrink-0 items-center justify-center rounded-[8px] border-2 border-ink bg-cream text-[0.65rem] text-muted">
        {pending ? "…" : "no image"}
      </div>
    );
  }
  return (
    // eslint-disable-next-line @next/next/no-img-element -- an IPFS image whose CID changes with the state
    <img src={`${IPFS_GATEWAY}/${media}`} alt={`heir credential: ${state ?? "unknown"}`} {...box} className="shrink-0 rounded-[8px] border-2 border-ink bg-cream" />
  );
}
