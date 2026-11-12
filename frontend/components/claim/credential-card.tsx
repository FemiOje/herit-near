"use client";

import { CredentialImage, STATE_TONE, useCredential } from "@/components/credential/credential-art";
import { CardHeading } from "@/components/ui/card-heading";
import { DataRow } from "@/components/ui/data-row";
import { ShieldIcon } from "@/components/ui/icons";
import type { HeirView } from "@/lib/near/contracts";

/** The heir's credential: a soulbound NEP-171 token the estate computes from its live state. */
export function CredentialCard({ id, heir }: { id: string; heir: HeirView }) {
  const { token, state, media } = useCredential(id, heir.label);

  return (
    <section className="card p-6">
      <CardHeading icon={ShieldIcon} accent="bg-yellow" title="your credential" body="a soulbound token on your account; it changes as the estate does" />
      <div className="mt-5 flex flex-wrap items-start gap-5">
        <CredentialImage media={media} state={state} size={132} pending={token.isPending} />
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
