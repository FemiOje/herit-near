"use client";

import Link from "next/link";
import { useState } from "react";

import { CredentialImage, STATE_TONE, describeCounts, useEstateCredentials } from "@/components/credential/credential-art";
import { StatusPill } from "@/components/estate/status-pill";
import { heirColor } from "@/components/heirs/heirs-card";
import { Blob, Star } from "@/components/ui/deco";
import { ChevronDownIcon, HeirIcon } from "@/components/ui/icons";
import { IconCircle } from "@/components/ui/icon-circle";
import { Reveal } from "@/components/ui/reveal";
import { formatStamp, unlocksAt } from "@/lib/estate/clock";
import { isSetUp } from "@/lib/estate/setup-plan";
import { useEstate } from "@/lib/estate/use-estate";
import { useNewestEstates } from "@/lib/estate/use-explorer";
import { accountUrl } from "@/lib/near/config";
import { credentialState, type EstateEntry } from "@/lib/near/contracts";
import { bpsToPercent, formatNear, nsToMs } from "@/lib/near/format";

const PAGE = 10;
const COLUMNS = "lg:grid-cols-[minmax(0,1.6fr)_7rem_minmax(0,1fr)_5rem_minmax(0,0.9fr)_1.5rem]";

/**
 * Every estate the factory has opened, for anyone: no wallet needed. The factory says which
 * estates exist and who opened them; each row's status, clock, heirs and vault are read from the
 * estate itself as you look, so nothing here is a stale copy of a countdown.
 */
export function EstatesExplorer() {
  const [limit, setLimit] = useState(PAGE);
  const newest = useNewestEstates(limit);

  return (
    <section id="estates" className="relative scroll-mt-24 overflow-hidden px-4 py-14 sm:px-6 sm:py-20">
      <Star className="left-[5%] top-[10%] hidden sm:block" size={34} fill="#4ECDC4" rotate={-8} />
      <Blob className="-right-8 bottom-[18%]" size={76} fill="#FFE566" />

      <div className="relative z-10 mx-auto max-w-6xl">
        <Reveal>
          <span className="tag tag-shadow bg-teal">live on NEAR testnet</span>
          <h2 className="mt-5 max-w-2xl">every estate opened so far</h2>
          <p className="mt-4 max-w-xl text-base text-muted">
            listed by the factory and filled in from each estate as you look: where its clock stands, what its vault holds, and who
            is named. open a row for the heirs, and each heir&apos;s view.
          </p>
        </Reveal>

        <div className="card mt-10 overflow-hidden">
          <div className={`hidden gap-4 border-b-2 border-ink bg-cream px-5 py-3 text-[0.7rem] font-bold text-muted lg:grid ${COLUMNS}`}>
            <span>estate</span>
            <span>status</span>
            <span>unlocks</span>
            <span>heirs</span>
            <span>vault</span>
            <span />
          </div>
          {newest.isPending ? (
            <p className="px-5 py-6 text-sm text-muted">reading the factory…</p>
          ) : newest.isError ? (
            <p className="px-5 py-6 text-sm text-coral">couldn&apos;t read the factory: {newest.error.message}</p>
          ) : newest.data.total === 0 ? (
            <p className="px-5 py-6 text-sm text-muted">no estates yet. be the first.</p>
          ) : (
            <ul className="divide-y-2 divide-ink">
              {newest.data.estates.map((e) => <Row key={e.account_id} entry={e} />)}
            </ul>
          )}
        </div>
        {newest.data && newest.data.total > newest.data.estates.length ? (
          <button type="button" className="btn btn-ghost btn-sm mt-4" onClick={() => setLimit(limit + PAGE)}>
            show older estates ({newest.data.total - newest.data.estates.length} more)
          </button>
        ) : null}
      </div>
    </section>
  );
}

function Row({ entry }: { entry: EstateEntry }) {
  const id = entry.account_id;
  const [open, setOpen] = useState(false);
  const { header, status, heirs, vault } = useEstate(id);
  const credentials = useEstateCredentials(id);
  const tokenOf = (label: string) => credentials.tokens.data?.find((t) => t.token_id === label) ?? null;
  const near = vault.find((v) => v.info.asset === null)?.balance;
  const tokens = vault.filter((v) => v.info.asset !== null && (v.balance ?? BigInt(0)) > BigInt(0)).length;
  const live = header.data && heirs.data ? isSetUp({ exists: true, faceBound: header.data.faceBound, heirs: heirs.data.map((h) => h.label) }) : null;
  const unlock = header.data ? unlocksAt(header.data) : null;

  return (
    <li>
      <button type="button" onClick={() => setOpen(!open)} aria-expanded={open} className={`grid w-full grid-cols-2 items-center gap-x-4 gap-y-3 px-5 py-4 text-left hover:bg-cream ${COLUMNS}`}>
        <span className="col-span-2 flex min-w-0 items-center gap-3 lg:col-span-1">
          <span title={credentials.summary ? `heir credentials (NFTs): ${describeCounts(credentials.summary.counts)}` : "no heir credentials yet"}>
            {credentials.summary ? (
              <CredentialImage media={credentials.summary.media} state={credentials.summary.state} size={40} />
            ) : (
              <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-[8px] border-2 border-dashed border-ink/40 text-[0.6rem] text-muted">
                {credentials.tokens.isPending ? "…" : "none"}
              </span>
            )}
          </span>
          <span className="min-w-0">
            <span className="mono block truncate font-bold">{id}</span>
            <span className="block truncate text-[0.7rem] text-muted">by {entry.grantor} · opened {formatStamp(nsToMs(entry.opened_at_ns))}</span>
          </span>
        </span>
        <span>{live === false ? <span className="tag bg-yellow">setup unfinished</span> : status.data ? <StatusPill status={status.data} size="sm" /> : "…"}</span>
        <Cell caption="unlocks" value={live === false ? "—" : status.data === "unlocked" ? "unlocked" : unlock ? formatStamp(unlock) : "clock not started"} />
        <Cell caption="heirs" value={heirs.data ? String(heirs.data.length) : "…"} />
        <Cell caption="vault" value={near === undefined ? "…" : `${formatNear(near, 2)} NEAR${tokens ? ` + ${tokens} token${tokens > 1 ? "s" : ""}` : ""}`} />
        <ChevronDownIcon size={18} className={`hidden justify-self-end transition-transform lg:block ${open ? "rotate-180" : ""}`} />
      </button>
      {open ? (
        <div className="space-y-2 border-t-2 border-dashed border-ink/30 bg-cream/50 px-5 py-4">
          {heirs.data?.length ? (
            heirs.data.map((h, i) => {
              const token = tokenOf(h.label);
              const state = credentialState(token);
              return (
              <Link key={h.label} href={`/claim?estate=${encodeURIComponent(id)}&heir=${encodeURIComponent(h.label)}`} className="flex items-center gap-3 rounded-[8px] border-2 border-ink bg-surface px-3 py-2 hover:bg-cream">
                {token ? (
                  <CredentialImage media={token.metadata?.media ?? null} state={state} size={32} />
                ) : (
                  <IconCircle size="sm" accent={heirColor(i)}><HeirIcon size={14} /></IconCircle>
                )}
                <span className="min-w-0 flex-1">
                  <span className="block text-sm font-bold">{h.relationship || h.label}</span>
                  <span className="mono block truncate text-[0.7rem] text-muted">{h.account_id}</span>
                </span>
                {state ? <span className={`tag hidden sm:inline-flex ${STATE_TONE[state]}`}>credential: {state}</span> : null}
                <span className="tag">{bpsToPercent(h.default_bps)}</span>
                <span className="text-xs font-bold underline">heir view</span>
              </Link>
              );
            })
          ) : (
            <p className="text-sm text-muted">no heirs named yet.</p>
          )}
          <a href={accountUrl(id)} target="_blank" rel="noopener noreferrer" className="inline-block text-xs font-bold underline">
            {id} on NearBlocks ↗
          </a>
        </div>
      ) : null}
    </li>
  );
}

function Cell({ caption, value }: { caption: string; value: string }) {
  return (
    <span className="min-w-0">
      <span className="mono block truncate text-sm font-bold">{value}</span>
      <span className="block text-[0.65rem] text-muted lg:hidden">{caption}</span>
    </span>
  );
}
