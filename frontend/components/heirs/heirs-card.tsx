"use client";

import Link from "next/link";
import { useState } from "react";

import { TxStatus } from "@/components/estate/tx-status";
import { EMPTY_ACCOUNT, EMPTY_KEY, HeirTargetInput, useTargetProblem } from "@/components/heirs/heir-target";
import { CardHeading } from "@/components/ui/card-heading";
import { HeirIcon, PlusIcon, TrashIcon } from "@/components/ui/icons";
import { IconCircle } from "@/components/ui/icon-circle";
import { assetKey, type AssetInfo, useShares } from "@/lib/estate/use-estate";
import { isNamed, type HeirTarget } from "@/lib/estate/setup-plan";
import { isLabel } from "@/lib/near/account";
import { calls, supportsNamedHeirs } from "@/lib/near/calls";
import { accountUrl } from "@/lib/near/config";
import type { HeirView } from "@/lib/near/contracts";
import { BPS, bpsToPercent } from "@/lib/near/format";
import { useAccountView } from "@/lib/near/use-account";
import { useTx } from "@/lib/near/use-tx";
import { useWallet } from "@/lib/near/wallet";

const COLORS = ["bg-coral", "bg-purple", "bg-teal", "bg-yellow", "bg-pink", "bg-lavender"];
export const heirColor = (index: number) => COLORS[index % COLORS.length];

/** Up to the contract's MAX_HEIRS. */
const MAX_HEIRS = 10;

/**
 * The estate's heirs: who they are, what each gets, and (for the grantor, before unlock) adding,
 * removing and per-asset shares. `compact` is the dashboard's summary, which links here.
 */
export function HeirsCard({
  id,
  heirs,
  assets,
  writable,
  compact = false,
}: {
  id: string;
  heirs: readonly HeirView[];
  assets: readonly AssetInfo[];
  writable: boolean;
  compact?: boolean;
}) {
  const shares = useShares(id, heirs, assets.map((a) => a.asset));
  const total = heirs.reduce((sum, h) => sum + h.default_bps, 0);

  return (
    <section className="card p-6">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <CardHeading icon={HeirIcon} accent="bg-pink" title="heirs" body={`${heirs.length} of ${MAX_HEIRS} · ${bpsToPercent(total)} of each asset allotted by default`} />
        {compact ? (
          <Link href={`/heirs?estate=${encodeURIComponent(id)}`} className="btn btn-ghost btn-sm">
            manage heirs
          </Link>
        ) : null}
      </div>

      <ShareBar heirs={heirs} />

      {heirs.length === 0 ? (
        <p className="mt-4 text-sm text-muted">no heirs yet. name a NEAR account the heir already has, or give them a new one under this estate from a key only they hold.</p>
      ) : (
        <ul className="mt-5 space-y-3">
          {heirs.map((h, i) => (
            <HeirRow key={h.label} id={id} heir={h} index={i} assets={assets} shares={shares.get(h.label)} writable={writable && !compact} />
          ))}
        </ul>
      )}

      {writable && !compact ? (
        <>
          {heirs.length < MAX_HEIRS ? <AddHeirForm id={id} heirs={heirs} totalBps={total} /> : null}
          {heirs.length > 0 && assets.length > 0 ? <SetShareForm id={id} heirs={heirs} assets={assets} shares={shares} /> : null}
        </>
      ) : null}
    </section>
  );
}

function ShareBar({ heirs }: { heirs: readonly HeirView[] }) {
  const allotted = heirs.reduce((sum, h) => sum + h.default_bps, 0);
  const rest = Math.max(0, BPS - allotted);
  return (
    <div className="mt-5 flex h-7 w-full overflow-hidden rounded-[6px] border-2 border-ink bg-cream" aria-label="default shares">
      {heirs.map((h, i) => (
        <div key={h.label} className={`${heirColor(i)} flex items-center justify-center ${i > 0 ? "border-l-2 border-ink" : ""}`} style={{ width: `${h.default_bps / 100}%` }} title={`${h.label}: ${bpsToPercent(h.default_bps)}`}>
          <span className="truncate px-1 text-[0.65rem] font-bold">{h.default_bps >= 800 ? h.label : ""}</span>
        </div>
      ))}
      {rest > 0 ? (
        <div
          className={`flex items-center justify-center ${heirs.length ? "border-l-2 border-ink" : ""}`}
          style={{ width: `${rest / 100}%`, backgroundImage: "repeating-linear-gradient(45deg, #EDEDDA 0 6px, #FAFADF 6px 12px)" }}
          title={`unallotted: ${bpsToPercent(rest)}`}
        >
          <span className="truncate px-1 text-[0.65rem] font-bold text-muted">{rest >= 1_200 ? "unallotted" : ""}</span>
        </div>
      ) : null}
    </div>
  );
}

function HeirRow({
  id,
  heir,
  index,
  assets,
  shares,
  writable,
}: {
  id: string;
  heir: HeirView;
  index: number;
  assets: readonly AssetInfo[];
  shares: Map<string, number> | undefined;
  writable: boolean;
}) {
  const tx = useTx();
  // A key heir's account is `<label>.<estate>` and outlives removal, so its label can't be reused.
  const subAccount = heir.account_id === `${heir.label}.${id}`;
  // Assets where this heir's share differs from their default: an override.
  const overrides = assets.filter((a) => {
    const bps = shares?.get(assetKey(a.asset));
    return bps !== undefined && bps !== heir.default_bps;
  });

  return (
    <li className="rounded-[8px] border-2 border-ink p-3">
      <div className="flex items-center gap-3">
        <IconCircle size="lg" accent={heirColor(index)}>
          <HeirIcon size={16} />
        </IconCircle>
        <div className="min-w-0 flex-1">
          <p className="font-bold">
            {heir.relationship || heir.label} <span className="text-sm font-medium text-muted">· {bpsToPercent(heir.default_bps)}</span>
          </p>
          <a href={accountUrl(heir.account_id)} target="_blank" rel="noreferrer" className="mono block truncate text-[0.72rem] underline decoration-1 underline-offset-2">
            {heir.account_id}
          </a>
        </div>
        {!heir.created ? <span className="tag bg-yellow">creating…</span> : null}
        {writable && heir.created ? (
          <button
            type="button"
            className="btn btn-ghost btn-sm"
            disabled={tx.busy}
            title={subAccount ? "remove this heir. their account stays, so this label can't go to a key again." : "remove this heir. their own account is untouched."}
            aria-label={`remove ${heir.label}`}
            onClick={() => void tx.send(calls.removeHeir(id, heir.label))}
          >
            <TrashIcon size={15} />
          </button>
        ) : null}
      </div>
      {overrides.length ? (
        <p className="mt-2 text-xs text-muted">
          {overrides.map((a) => `${a.symbol}: ${bpsToPercent(shares!.get(assetKey(a.asset))!)}`).join(" · ")}
        </p>
      ) : null}
      <TxStatus tx={tx} success="heir removed" className="mt-2" />
    </li>
  );
}

function AddHeirForm({ id, heirs, totalBps }: { id: string; heirs: readonly HeirView[]; totalBps: number }) {
  const { accountId: grantor } = useWallet();
  const estateView = useAccountView(id);
  const namedAllowed = supportsNamedHeirs(estateView.data?.global_contract_hash);
  const [open, setOpen] = useState(false);
  const [label, setLabel] = useState("");
  const [relationship, setRelationship] = useState("");
  const [percent, setPercent] = useState("");
  const [target, setTarget] = useState<HeirTarget>(EMPTY_ACCOUNT);
  const shown = isNamed(target) && !namedAllowed ? EMPTY_KEY : target;
  const tx = useTx();

  const ctx = { estate: id, label, grantor, taken: heirs.map((h) => h.account_id), namedAllowed };
  const targetProblem = useTargetProblem(shown, ctx);
  const bps = Math.round(Number(percent) * 100);
  const problem = !isLabel(label)
    ? "the label is one lowercase segment: son, daughter, ada"
    : heirs.some((h) => h.label === label)
      ? "that label is taken"
      : relationship.length > 32
        ? "relationship: 32 characters at most"
        : !(bps >= 0 && bps <= BPS) || percent === ""
          ? "a share between 0 and 100%"
          : totalBps + bps > BPS
            ? `only ${bpsToPercent(BPS - totalBps)} left to allot`
            : targetProblem;

  if (!open) {
    return (
      <button type="button" className="btn btn-ghost btn-sm mt-5" onClick={() => setOpen(true)}>
        <PlusIcon size={15} /> add an heir
      </button>
    );
  }

  const named = isNamed(shown);
  const send = () =>
    void tx.send(named ? calls.addHeirAccount(id, label, shown.accountId, relationship || label, bps) : calls.addHeir(id, label, shown.publicKey, relationship || label, bps));

  return (
    <div className="mt-5 space-y-3 rounded-[8px] border-2 border-ink bg-cream p-4">
      <p className="text-sm font-bold">add an heir</p>
      <div className="grid gap-3 sm:grid-cols-3">
        <div>
          <label className="label" htmlFor="heir-label">label</label>
          <input id="heir-label" className="input mono" value={label} onChange={(e) => setLabel(e.target.value.toLowerCase().trim())} placeholder="son" />
        </div>
        <div>
          <label className="label" htmlFor="heir-rel">relationship</label>
          <input id="heir-rel" className="input" value={relationship} onChange={(e) => setRelationship(e.target.value)} placeholder="son" />
        </div>
        <div>
          <label className="label" htmlFor="heir-share">default share (%)</label>
          <input id="heir-share" className="input mono" value={percent} onChange={(e) => setPercent(e.target.value)} placeholder="60" inputMode="decimal" />
        </div>
      </div>
      <HeirTargetInput value={shown} onChange={setTarget} ctx={ctx} problem={targetProblem} />
      <button type="button" className="btn btn-sm" disabled={problem !== undefined || tx.busy} onClick={send}>
        {tx.busy ? "confirm in your wallet…" : named ? "add heir · 0.01 NEAR" : "add heir · 0.2 NEAR"}
      </button>
      {problem && problem !== targetProblem ? <p className="hint">{problem}</p> : null}
      <TxStatus tx={tx} success={named ? "heir added: their credential is on their account now" : "heir added: their account and credential exist now"} />
    </div>
  );
}

function SetShareForm({ id, heirs, assets, shares }: { id: string; heirs: readonly HeirView[]; assets: readonly AssetInfo[]; shares: Map<string, Map<string, number>> }) {
  const [label, setLabel] = useState(heirs[0]?.label ?? "");
  const [assetIndex, setAssetIndex] = useState(0);
  const [percent, setPercent] = useState("");
  const tx = useTx();
  const asset = assets[assetIndex];
  const bps = Math.round(Number(percent) * 100);
  // The contract's prospective total: this heir's current share swapped for the new one.
  const others = heirs.filter((h) => h.label !== label).reduce((sum, h) => sum + (shares.get(h.label)?.get(assetKey(asset?.asset ?? null)) ?? 0), 0);
  const problem = percent === "" || !(bps >= 0 && bps <= BPS) ? "a share between 0 and 100%" : others + bps > BPS ? `the others already take ${bpsToPercent(others)} of ${asset?.symbol}` : undefined;

  return (
    <div className="mt-4 space-y-3 rounded-[8px] border-2 border-ink p-4">
      <p className="text-sm font-bold">a different share of one asset</p>
      <div className="grid gap-3 sm:grid-cols-3">
        <select className="input" value={label} onChange={(e) => setLabel(e.target.value)} aria-label="heir">
          {heirs.map((h) => <option key={h.label} value={h.label}>{h.relationship || h.label}</option>)}
        </select>
        <select className="input" value={assetIndex} onChange={(e) => setAssetIndex(Number(e.target.value))} aria-label="asset">
          {assets.map((a, i) => <option key={assetKey(a.asset)} value={i}>{a.symbol}</option>)}
        </select>
        <input className="input mono" value={percent} onChange={(e) => setPercent(e.target.value)} placeholder="25" inputMode="decimal" aria-label="share in percent" />
      </div>
      <button type="button" className="btn btn-sm btn-ghost" disabled={problem !== undefined || tx.busy || !asset} onClick={() => void tx.send(calls.setShare(id, label, asset.asset, bps))}>
        {tx.busy ? "confirm in your wallet…" : "set share"}
      </button>
      <p className="hint">{problem ?? "overrides the heir's default for this asset only. a new override attaches 0.01 NEAR for storage."}</p>
      <TxStatus tx={tx} success="share set" />
    </div>
  );
}
