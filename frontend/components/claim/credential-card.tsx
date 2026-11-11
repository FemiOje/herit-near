"use client";

import { useQuery } from "@tanstack/react-query";

import { CardHeading } from "@/components/ui/card-heading";
import { DataRow } from "@/components/ui/data-row";
import { ShieldIcon } from "@/components/ui/icons";
import { IPFS_GATEWAY } from "@/lib/near/config";
import { credentialState, estate, type HeirView } from "@/lib/near/contracts";

const STATE_TONE = { locked: "bg-teal", grace: "bg-yellow", claimable: "bg-coral text-white", claimed: "bg-lavender" } as const;

/** The heir's credential: a soulbound NEP-171 token the estate computes from its live state. */
export function CredentialCard({ id, heir }: { id: string; heir: HeirView }) {
  const token = useQuery({ queryKey: ["nft_token", id, heir.label], queryFn: () => estate(id).nftToken(heir.label), refetchInterval: 15_000 });
  const state = credentialState(token.data ?? null);
  const media = token.data?.metadata?.media;

  return (
    <section className="card p-6">
      <CardHeading icon={ShieldIcon} accent="bg-yellow" title="your credential" body="a soulbound token on your account; it changes as the estate does" />
      <div className="mt-5 flex flex-wrap items-start gap-5">
        {media ? (
          // eslint-disable-next-line @next/next/no-img-element -- an IPFS image whose CID changes with the state
          <img src={`${IPFS_GATEWAY}/${media}`} alt={`credential: ${state ?? "unknown"}`} width={132} height={132} className="rounded-[8px] border-2 border-ink bg-cream" />
        ) : (
          <div className="flex h-[132px] w-[132px] items-center justify-center rounded-[8px] border-2 border-ink bg-cream text-xs text-muted">{token.isPending ? "…" : "no image"}</div>
        )}
        <div className="min-w-[12rem] flex-1 space-y-3">
          <span className={`tag ${state ? STATE_TONE[state] : "bg-surface"}`}>{state ?? "…"}</span>
          <dl className="space-y-2 text-sm">
            <DataRow label="token" value={heir.label} />
            <DataRow label="relationship" value={heir.relationship} />
          </dl>
          <p className="mono break-all text-[0.72rem] text-muted">owned by {heir.account_id}</p>
        </div>
      </div>
    </section>
  );
}
