"use client";

import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useState } from "react";

import { CoHeirsCard } from "@/components/claim/co-heirs-card";
import { CredentialCard } from "@/components/claim/credential-card";
import { ShareCard } from "@/components/claim/share-card";
import { UnlockBanner } from "@/components/claim/unlock-banner";
import { StatusPill } from "@/components/estate/status-pill";
import { PageHeader } from "@/components/layout/page-header";
import { ArrowRightIcon, HeirIcon } from "@/components/ui/icons";
import { IconCircle } from "@/components/ui/icon-circle";
import { useEstate } from "@/lib/estate/use-estate";
import type { HeirSlot } from "@/lib/heir/slots";
import { useHeirSlots } from "@/lib/heir/use-heir";
import { isAccountId, isLabel } from "@/lib/near/account";
import { accountUrl } from "@/lib/near/config";
import { useWallet } from "@/lib/near/wallet";

export const slotLink = (s: { estate: string; label: string }) => `/claim?estate=${encodeURIComponent(s.estate)}&heir=${encodeURIComponent(s.label)}`;

/** `/claim`: one heir slot when `?estate=&heir=` names it, otherwise finding yours. */
export function ClaimView() {
  const params = useSearchParams();
  const id = params.get("estate");
  const label = params.get("heir");
  if (id && label && isAccountId(id) && isLabel(label)) return <HeirView id={id} label={label} />;
  return <FindSlot />;
}

/**
 * Finding a slot: the connected wallet's, or any account typed in (a key heir's
 * `<label>.<estate>`, say). Viewing is public, so no wallet is needed to look.
 */
function FindSlot() {
  const { accountId, connect } = useWallet();
  const [typed, setTyped] = useState("");
  const [looking, setLooking] = useState<string | null>(null);
  const account = looking ?? accountId;
  const slots = useHeirSlots(account);

  return (
    <>
      <PageHeader eyebrow="heir" title="what you inherit" description="Find the estates that name you as an heir: by your NEAR account, or the account made from your key." />
      <div className="grid items-start gap-6 lg:grid-cols-[1.4fr_1fr]">
        <section className="card p-6">
          <h2 className="text-lg">{account ? <>estates naming <span className="mono">{account}</span></> : "connect, or look up an account"}</h2>
          {!account ? (
            <p className="mt-2 text-sm text-muted">connect the wallet the grantor named, or type an heir account on the right.</p>
          ) : slots.isPending ? (
            <p className="mt-3 text-sm text-muted">asking each estate the factory opened…</p>
          ) : slots.isError ? (
            <p className="mt-3 text-sm text-coral">couldn&apos;t search: {slots.error.message}</p>
          ) : slots.data.length === 0 ? (
            <p className="mt-3 text-sm text-muted">no estate names this account as an heir.</p>
          ) : (
            <ul className="mt-4 space-y-2">
              {slots.data.map((s) => <SlotRow key={`${s.estate}#${s.label}`} slot={s} />)}
            </ul>
          )}
        </section>

        <section className="card space-y-3 p-6">
          {!accountId ? (
            <button type="button" className="btn w-full" onClick={() => void connect()}>connect wallet</button>
          ) : looking ? (
            <button type="button" className="btn btn-ghost btn-sm" onClick={() => setLooking(null)}>back to {accountId}</button>
          ) : null}
          <form
            className="space-y-2"
            onSubmit={(e) => {
              e.preventDefault();
              if (isAccountId(typed)) setLooking(typed);
            }}
          >
            <label className="label" htmlFor="heir-account">look up an heir account</label>
            <input id="heir-account" className="input mono" value={typed} onChange={(e) => setTyped(e.target.value.toLowerCase().trim())} placeholder="son.alice.herit-near.testnet" autoComplete="off" />
            <button type="submit" className="btn btn-sm btn-ghost" disabled={!isAccountId(typed)}>look up</button>
            <p className="hint">a key heir&apos;s account is <span className="mono">&lt;label&gt;.&lt;estate&gt;</span>; it&apos;s in the key file you saved.</p>
          </form>
        </section>
      </div>
    </>
  );
}

function SlotRow({ slot }: { slot: HeirSlot }) {
  return (
    <li>
      <Link href={slotLink(slot)} className="flex items-center gap-3 rounded-[8px] border-2 border-ink px-3 py-2.5 hover:bg-cream">
        <IconCircle size="sm" accent="bg-pink"><HeirIcon size={16} /></IconCircle>
        <span className="min-w-0 flex-1">
          <span className="block font-bold">{slot.relationship}</span>
          <span className="mono block truncate text-xs text-muted">{slot.label} · {slot.estate}</span>
        </span>
        <ArrowRightIcon size={16} />
      </Link>
    </li>
  );
}

/** One heir slot, as its heir sees it: where the estate stands, their share, their credential. */
function HeirView({ id, label }: { id: string; label: string }) {
  const { header, status, heirs, vault } = useEstate(id);
  const heir = heirs.data?.find((h) => h.label === label);

  if (header.isError) return <p className="text-sm text-coral">couldn&apos;t read {id}: {header.error.message}</p>;
  if (!header.data || !status.data || !heirs.data) return <p className="text-sm text-muted">reading {id}…</p>;
  if (!heir) {
    return (
      <div className="card max-w-xl space-y-3 p-6">
        <p><span className="mono">{id}</span> has no heir <span className="mono">{label}</span>.</p>
        <Link href="/claim" className="btn btn-ghost btn-sm">find your estates</Link>
      </div>
    );
  }

  return (
    <>
      <PageHeader
        eyebrow="heir view"
        title={`${heir.relationship || label} · ${id}`}
        mono
        starFill="#F9A8B8"
        description={
          <p className="text-xs">
            heir <span className="mono">{label}</span>, paid to{" "}
            <a href={accountUrl(heir.account_id)} target="_blank" rel="noreferrer" className="mono underline">{heir.account_id}</a> · grantor{" "}
            <span className="mono">{header.data.grantor}</span>
          </p>
        }
      >
        <StatusPill status={status.data} />
      </PageHeader>

      <UnlockBanner header={header.data} status={status.data} />

      <div className="grid items-start gap-6 lg:grid-cols-[1.1fr_1fr]">
        <ShareCard id={id} heir={heir} status={status.data} vault={vault} />
        <div className="space-y-6">
          <CredentialCard id={id} heir={heir} />
          <CoHeirsCard heirs={heirs.data} label={label} />
        </div>
      </div>
    </>
  );
}
