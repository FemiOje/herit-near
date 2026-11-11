"use client";

import Link from "next/link";

import { MyEstates } from "@/components/dashboard/my-estates";
import { ProofOfLifeCard } from "@/components/dashboard/proof-of-life-card";
import { VaultCard } from "@/components/dashboard/vault-card";
import { ActivityCard } from "@/components/estate/activity-card";
import { StatusPill } from "@/components/estate/status-pill";
import { HeirsCard } from "@/components/heirs/heirs-card";
import { PageHeader } from "@/components/layout/page-header";
import { isSetUp } from "@/lib/estate/setup-plan";
import { useEstate } from "@/lib/estate/use-estate";
import { accountUrl } from "@/lib/near/config";
import { useWallet } from "@/lib/near/wallet";

/** One estate, as its grantor manages it. `focus` picks the page: the dashboard or the heirs. */
export function EstateView({ id, focus }: { id: string; focus: "dashboard" | "heirs" }) {
  const { accountId } = useWallet();
  const { header, status, heirs, vault } = useEstate(id);

  if (header.isError) return <p className="text-sm text-coral">couldn&apos;t read {id}: {header.error.message}</p>;
  if (!header.data || !status.data) return <p className="text-sm text-muted">reading {id}…</p>;

  const isGrantor = accountId === header.data.grantor;
  const open = status.data !== "unlocked";
  const assets = vault.map((v) => v.info);
  // Not live until the grantor's face is bound and there's someone to inherit.
  const unfinished = heirs.data && !isSetUp({ exists: true, faceBound: header.data.faceBound, heirs: heirs.data.map((h) => h.label) });

  if (unfinished) {
    return (
      <section className="mt-6">
        <div className="card max-w-2xl space-y-4 bg-yellow p-6">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <h2 className="mono text-xl">{id}</h2>
            <span className="tag">setup unfinished</span>
          </div>
          <ul className="space-y-1 text-sm">
            <li>{header.data.faceBound ? "✓" : "✗"} the grantor&apos;s Selfie Check, which binds their face and starts the clock</li>
            <li>{heirs.data!.length ? "✓" : "✗"} at least one heir</li>
          </ul>
          <p className="text-sm">until both are done the estate isn&apos;t live: nothing is counting down and nobody can inherit.</p>
          {isGrantor ? (
            <Link href={`/setup?estate=${encodeURIComponent(id)}`} className="btn">
              finish the setup
            </Link>
          ) : (
            <p className="hint">only its grantor, {header.data.grantor}, can finish it.</p>
          )}
        </div>
      </section>
    );
  }

  return (
    <section className="mt-6 space-y-6">
      <PageHeader
        level={2}
        eyebrow="viewing"
        title={id}
        mono
        description={
          <p className="text-xs">
            grantor <span className="mono">{header.data.grantor}</span> ·{" "}
            <a href={accountUrl(id)} target="_blank" rel="noreferrer" className="underline">
              on NearBlocks
            </a>
          </p>
        }
      >
        <StatusPill status={status.data} />
      </PageHeader>

      {focus === "dashboard" ? (
        <div className="grid gap-6 lg:grid-cols-[1fr_1.1fr]">
          <div className="space-y-6">
            <ProofOfLifeCard id={id} header={header.data} status={status.data} isGrantor={isGrantor} />
            <ActivityCard id={id} />
          </div>
          <div className="space-y-6">
            <VaultCard id={id} grantor={header.data.grantor} rows={vault} faceBound={header.data.faceBound} open={open} isGrantor={isGrantor} />
            {heirs.data ? <HeirsCard id={id} heirs={heirs.data} assets={assets} writable={isGrantor && open} compact /> : null}
          </div>
        </div>
      ) : heirs.data ? (
        <HeirsCard id={id} heirs={heirs.data} assets={assets} writable={isGrantor && open} />
      ) : (
        <p className="text-sm text-muted">reading the heirs…</p>
      )}
    </section>
  );
}

/** The picker plus the chosen estate, as one client component the server pages can render. */
export function GrantorEstates({ focus }: { focus: "dashboard" | "heirs" }) {
  return <MyEstates>{(id) => <EstateView id={id} focus={focus} />}</MyEstates>;
}
