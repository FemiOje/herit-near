"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { useQuery } from "@tanstack/react-query";

import { TxStatus } from "@/components/estate/tx-status";
import { CardHeading } from "@/components/ui/card-heading";
import { ClockIcon, TreeIcon } from "@/components/ui/icons";
import { estateId, isLabel } from "@/lib/near/account";
import { MIN_OPEN_DEPOSIT, calls } from "@/lib/near/calls";
import { FACTORY } from "@/lib/near/config";
import { formatNear, parseNear } from "@/lib/near/format";
import { viewAccount } from "@/lib/near/rpc";
import { useTx } from "@/lib/near/use-tx";
import { useWallet } from "@/lib/near/wallet";
import { GRACE_PRESETS, INTERVAL_PRESETS, formatDuration, type Preset } from "@/lib/estate/clock";

/**
 * Opens an estate through the factory: one transaction that creates `<label>.<factory>` as a
 * keyless account running the published estate code, with the connected wallet as its grantor.
 */
export function OpenEstateForm() {
  const { accountId, connect } = useWallet();
  const router = useRouter();
  const tx = useTx();
  const [label, setLabel] = useState("");
  const [interval, setIntervalSec] = useState(INTERVAL_PRESETS[0].seconds);
  const [grace, setGrace] = useState(GRACE_PRESETS[0].seconds);
  const [funding, setFunding] = useState("0.2");

  // Suggest the wallet's own name, which is usually what people want.
  useEffect(() => {
    if (accountId && label === "") {
      const first = accountId.split(".")[0].replace(/[^a-z0-9_-]/g, "");
      if (first) setLabel(first.slice(0, 32));
    }
  }, [accountId, label]);

  const id = isLabel(label) ? estateId(label, FACTORY) : null;
  const taken = useQuery({
    queryKey: ["account_exists", id],
    enabled: id !== null,
    queryFn: async () => (await viewAccount(id!)) !== null,
  });

  let deposit: bigint | null = null;
  try {
    deposit = parseNear(funding);
  } catch {
    deposit = null;
  }

  const problem = !accountId
    ? "connect a wallet first"
    : !id
      ? "lowercase letters, digits, - and _ only, one segment"
      : taken.data
        ? `${id} already exists`
        : deposit === null
          ? "not an amount"
          : deposit < MIN_OPEN_DEPOSIT
            ? `at least ${formatNear(MIN_OPEN_DEPOSIT)} NEAR`
            : undefined;

  async function open() {
    if (!id || deposit === null) return;
    const result = await tx.send(calls.openEstate(FACTORY, label, interval, grace, deposit));
    if (!result) return;
    // The factory refunds and returns false if the account couldn't be made.
    const made = await viewAccount(id).catch(() => null);
    if (made) router.push(`/dashboard?estate=${encodeURIComponent(id)}`);
  }

  return (
    <div className="grid gap-6 lg:grid-cols-[1.2fr_1fr]">
      <section className="card space-y-6 p-6">
        <CardHeading icon={TreeIcon} accent="bg-yellow" title="name it" body="the estate is its own NEAR account under the factory" />

        <div>
          <label className="label" htmlFor="estate-label">estate name</label>
          <div className="flex items-stretch">
            <input
              id="estate-label"
              className="input mono rounded-r-none"
              value={label}
              onChange={(e) => setLabel(e.target.value.toLowerCase().trim())}
              placeholder="alice"
              autoComplete="off"
            />
            <span className="mono flex items-center whitespace-nowrap rounded-r-[6px] border-2 border-l-0 border-ink bg-cream px-3 text-sm">
              .{FACTORY}
            </span>
          </div>
          <p className="hint">
            {id && taken.isFetching
              ? "checking…"
              : id && taken.data === false
                ? `${id} is free`
                : (problem ?? "")}
          </p>
        </div>

        <CardHeading icon={ClockIcon} accent="bg-lavender" title="set the clock" body="check in within the interval; miss it and grace starts; miss grace and heirs can claim" />
        <PresetGroup legend="check in every" presets={INTERVAL_PRESETS} value={interval} onChange={setIntervalSec} />
        <PresetGroup legend="then grace for" presets={GRACE_PRESETS} value={grace} onChange={setGrace} />

        <div>
          <label className="label" htmlFor="estate-funding">opening deposit (NEAR)</label>
          <input id="estate-funding" className="input mono" value={funding} onChange={(e) => setFunding(e.target.value)} inputMode="decimal" />
          <p className="hint">
            pays for the estate account&apos;s own storage; 0.01 NEAR goes to the factory. at least 0.11. the money for your heirs goes in the vault afterwards.
          </p>
        </div>
      </section>

      <aside className="card h-fit space-y-4 p-6">
        <h2 className="text-xl">review</h2>
        <dl className="space-y-2 text-sm">
          <Row label="estate" value={id ?? "—"} />
          <Row label="grantor" value={accountId ?? "connect a wallet"} />
          <Row label="check-in interval" value={formatDuration(interval)} />
          <Row label="grace" value={formatDuration(grace)} />
          <Row label="deposit" value={deposit !== null ? `${formatNear(deposit)} NEAR` : "—"} />
        </dl>
        <p className="text-xs leading-relaxed text-muted">
          the estate gets no keys: only its code can move what it holds. the clock starts at your first selfie check.
        </p>
        {accountId ? (
          <button type="button" className="btn w-full" disabled={problem !== undefined || tx.busy || taken.isFetching} onClick={() => void open()}>
            {tx.busy ? "confirm in your wallet…" : "open the estate"}
          </button>
        ) : (
          <button type="button" className="btn w-full" onClick={() => void connect()}>connect wallet</button>
        )}
        {problem && accountId ? <p className="hint">{problem}</p> : null}
        <TxStatus tx={tx} success="estate opened, taking you to it…" />
      </aside>
    </div>
  );
}

function PresetGroup({ legend, presets, value, onChange }: { legend: string; presets: readonly Preset[]; value: number; onChange: (s: number) => void }) {
  return (
    <fieldset>
      <legend className="label">{legend}</legend>
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        {presets.map((p) => (
          <button
            key={p.seconds}
            type="button"
            onClick={() => onChange(p.seconds)}
            className={`rounded-[8px] border-2 border-ink px-3 py-2 text-left text-sm font-bold ${value === p.seconds ? "bg-yellow shadow-brut-sm" : "bg-surface"}`}
            aria-pressed={value === p.seconds}
          >
            {p.label}
            {p.note ? <span className="block text-[0.68rem] font-medium text-muted">{p.note}</span> : null}
          </button>
        ))}
      </div>
    </fieldset>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-3">
      <dt className="text-muted">{label}</dt>
      <dd className="mono break-all text-right font-bold">{value}</dd>
    </div>
  );
}
