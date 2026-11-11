"use client";

import { useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";

import { TxStatus } from "@/components/estate/tx-status";
import { EMPTY_ACCOUNT, EMPTY_KEY, HeirTargetInput, useTargetProblem } from "@/components/heirs/heir-target";
import { SelfieCheckModal } from "@/components/selfie-check/selfie-check-modal";
import { CardHeading } from "@/components/ui/card-heading";
import { DataRow } from "@/components/ui/data-row";
import { IconCircle } from "@/components/ui/icon-circle";
import { AlertIcon, ArrowRightIcon, CheckIcon, ClockIcon, HeirIcon, PlusIcon, SelfieIcon, TrashIcon, TreeIcon, VaultIcon } from "@/components/ui/icons";
import { GRACE_PRESETS, INTERVAL_PRESETS, formatDuration, type Preset } from "@/lib/estate/clock";
import { NOTHING_YET, OPENING_DEPOSIT, isNamed, setupPlan, setupProblems, vaultDeposit, type HeirTarget, type SetupDraft, type SetupProgress } from "@/lib/estate/setup-plan";
import { useSetupDraft, useSetupProgress } from "@/lib/estate/use-setup";
import { estateId, isLabel, labelOf } from "@/lib/near/account";
import { supportsNamedHeirs } from "@/lib/near/calls";
import { FACTORY } from "@/lib/near/config";
import { BPS, bpsToPercent, formatNear } from "@/lib/near/format";
import { useAccountView } from "@/lib/near/use-account";
import { useTx } from "@/lib/near/use-tx";
import { useWallet } from "@/lib/near/wallet";

const STEPS = [
  { title: "name & clock", icon: TreeIcon },
  { title: "heirs", icon: HeirIcon },
  { title: "vault", icon: VaultIcon },
  { title: "proof of life", icon: SelfieIcon },
] as const;

/**
 * Setup, start to finish. Nothing is sent until the last step, and then everything is: a Selfie
 * Check binds the grantor's face, and one wallet approval opens the estate, checks in, adds every
 * heir and funds the vault. `?estate=` resumes a setup that stopped halfway.
 */
export function SetupWizard() {
  const { accountId, connect, loading } = useWallet();
  const { draft, update, clear } = useSetupDraft(accountId);
  const params = useSearchParams();
  const resuming = params.get("estate");
  const [step, setStep] = useState(0);

  // Resuming fixes the name to that estate's.
  const label = resuming ? labelOf(resuming) : draft.label;
  const d: SetupDraft = { ...draft, label };
  const id = isLabel(label) ? estateId(label, FACTORY) : null;
  const progress = useSetupProgress(id);
  const onChain = progress.data;
  const theirs = onChain?.exists && onChain.grantor !== null && onChain.grantor !== accountId;
  const exists = onChain?.exists ?? false;
  // An estate that exists keeps its timers; they're shown, not chosen.
  const shown = exists && onChain?.intervalSec ? { ...d, intervalSec: onChain.intervalSec, graceSec: onChain.graceSec! } : d;

  if (loading) return <p className="text-sm text-muted">loading your wallet…</p>;
  if (!accountId) {
    return (
      <div className="card max-w-xl p-6">
        <h2 className="text-xl">connect the wallet that will own the estate</h2>
        <p className="mt-2 text-sm text-muted">it becomes the grantor: the one who checks in, and the only one who can change the estate.</p>
        <button type="button" className="btn mt-4" onClick={() => void connect()}>connect wallet</button>
      </div>
    );
  }

  const problems = [
    ...setupProblems(shown, { factory: FACTORY, grantor: accountId }),
    ...(theirs ? [`${id} belongs to someone else; pick another name`] : []),
  ];

  return (
    <>
      {resuming ? (
        <p className="rounded-[8px] border-2 border-ink bg-yellow px-4 py-3 text-sm font-bold">
          finishing the setup of <span className="mono">{resuming}</span>. it isn&apos;t live until your face is bound and it has heirs.
        </p>
      ) : null}

      <ol className="grid grid-cols-2 gap-3 sm:grid-cols-4">
        {STEPS.map((s, i) => (
          <li key={s.title}>
            <button
              type="button"
              onClick={() => setStep(i)}
              className={`flex w-full items-center gap-2.5 rounded-[8px] border-2 border-ink px-3 py-2.5 text-left ${i === step ? "bg-yellow shadow-brut" : i < step ? "bg-teal" : "bg-surface"}`}
              aria-current={i === step ? "step" : undefined}
            >
              <IconCircle size="sm">{i < step ? <CheckIcon size={14} /> : <s.icon size={18} />}</IconCircle>
              <span className="min-w-0">
                <span className="mono block text-[0.65rem] font-bold text-muted">0{i + 1}</span>
                <span className="block truncate text-sm font-bold">{s.title}</span>
              </span>
            </button>
          </li>
        ))}
      </ol>

      <div className="card p-6 sm:p-8">
        {step === 0 ? (
          <NameStep draft={shown} locked={exists && !theirs} resuming={resuming !== null} checking={progress.isFetching} taken={!!theirs} onChange={update} />
        ) : step === 1 ? (
          <HeirsStep draft={d} added={onChain?.heirs ?? []} estate={id} grantor={accountId} onChange={update} />
        ) : step === 2 ? (
          <VaultStep draft={d} funded={(onChain?.vaultNear ?? BigInt(0)) > BigInt(0)} onChange={update} />
        ) : (
          <ReviewStep draft={shown} problems={problems} progress={onChain ?? null} onDone={clear} />
        )}
      </div>

      {step < STEPS.length - 1 ? (
        <div className="flex items-center justify-between gap-3">
          <button type="button" className="btn btn-ghost" disabled={step === 0} onClick={() => setStep(step - 1)}>back</button>
          <button type="button" className="btn" onClick={() => setStep(step + 1)}>
            continue <ArrowRightIcon size={16} />
          </button>
        </div>
      ) : (
        <button type="button" className="btn btn-ghost" onClick={() => setStep(step - 1)}>back</button>
      )}
    </>
  );
}

function NameStep({ draft, locked, resuming, checking, taken, onChange }: { draft: SetupDraft; locked: boolean; resuming: boolean; checking: boolean; taken: boolean; onChange: (d: SetupDraft) => void }) {
  const id = isLabel(draft.label) ? estateId(draft.label, FACTORY) : null;
  return (
    <div className="space-y-6">
      <CardHeading icon={TreeIcon} accent="bg-yellow" title="name it" body="the estate is its own NEAR account under the factory, with no keys" />
      <div>
        <label className="label" htmlFor="estate-label">estate name</label>
        <div className="flex items-stretch">
          <input id="estate-label" className="input mono rounded-r-none" value={draft.label} disabled={resuming} onChange={(e) => onChange({ ...draft, label: e.target.value.toLowerCase().trim() })} placeholder="alice" autoComplete="off" />
          <span className="mono flex items-center whitespace-nowrap rounded-r-[6px] border-2 border-l-0 border-ink bg-cream px-3 text-sm">.{FACTORY}</span>
        </div>
        <p className="hint">
          {!id ? "lowercase letters, digits, - and _ only" : checking ? "checking…" : taken ? `${id} belongs to someone else` : locked ? `${id} is yours already: setup will finish it` : `${id} is free`}
        </p>
      </div>

      <CardHeading icon={ClockIcon} accent="bg-lavender" title="set the clock" body="check in within the interval; miss it and grace starts; miss grace and your heirs can claim" />
      {locked ? (
        <dl className="space-y-2 text-sm">
          <DataRow label="check in every" value={formatDuration(draft.intervalSec)} />
          <DataRow label="then grace for" value={formatDuration(draft.graceSec)} />
        </dl>
      ) : (
        <>
          <PresetGroup legend="check in every" presets={INTERVAL_PRESETS} value={draft.intervalSec} onChange={(s) => onChange({ ...draft, intervalSec: s })} />
          <PresetGroup legend="then grace for" presets={GRACE_PRESETS} value={draft.graceSec} onChange={(s) => onChange({ ...draft, graceSec: s })} />
        </>
      )}
    </div>
  );
}

function HeirsStep({ draft, added, estate, grantor, onChange }: { draft: SetupDraft; added: readonly string[]; estate: string | null; grantor: string; onChange: (d: SetupDraft) => void }) {
  // A new estate gets the factory's current code; one being resumed may run older code.
  const estateView = useAccountView(estate);
  const namedAllowed = supportsNamedHeirs(estateView.data?.global_contract_hash);
  const [label, setLabel] = useState("");
  const [relationship, setRelationship] = useState("");
  const [percent, setPercent] = useState("");
  const [target, setTarget] = useState<HeirTarget>(EMPTY_ACCOUNT);
  const shown = isNamed(target) && !namedAllowed ? EMPTY_KEY : target;
  const total = draft.heirs.reduce((s, h) => s + h.bps, 0);
  const bps = Math.round(Number(percent) * 100);
  const taken = draft.heirs.filter(isNamed).map((h) => h.accountId);
  const targetProblem = useTargetProblem(shown, { estate, label, grantor, taken, namedAllowed });
  const problem = !isLabel(label)
    ? "a label: son, daughter, ada"
    : draft.heirs.some((h) => h.label === label)
      ? "that label is taken"
      : !(bps > 0 && bps <= BPS)
        ? "a share above 0%"
        : total + bps > BPS
          ? `only ${bpsToPercent(BPS - total)} left`
          : targetProblem;

  return (
    <div className="space-y-5">
      <CardHeading icon={HeirIcon} accent="bg-pink" title="who inherits" body="name a NEAR account the heir already has, or give them a new one under the estate from a key only they hold. at least one." />

      {draft.heirs.length ? (
        <ul className="space-y-2">
          {draft.heirs.map((h) => {
            const done = added.includes(h.label);
            return (
              <li key={h.label} className="flex items-center gap-3 rounded-[8px] border-2 border-ink px-3 py-2">
                <span className="font-bold">{h.relationship || h.label}</span>
                <span className="mono text-xs text-muted">{h.label} · {bpsToPercent(h.bps)}</span>
                <span className="mono hidden flex-1 truncate text-[0.68rem] text-muted sm:block">{isNamed(h) ? h.accountId : `${h.label}.${estate ?? "…"} · key ${h.publicKey.slice(8, 16)}…`}</span>
                {done ? (
                  <span className="tag bg-teal">added</span>
                ) : (
                  <button type="button" className="btn btn-ghost btn-sm" aria-label={`remove ${h.label}`} onClick={() => onChange({ ...draft, heirs: draft.heirs.filter((x) => x.label !== h.label) })}>
                    <TrashIcon size={14} />
                  </button>
                )}
              </li>
            );
          })}
        </ul>
      ) : (
        <p className="text-sm text-muted">no heirs yet.</p>
      )}
      <p className="text-xs font-bold">{bpsToPercent(total)} of each asset allotted{total < BPS ? `; the other ${bpsToPercent(BPS - total)} stays unclaimed` : ""}</p>

      <div className="space-y-3 rounded-[8px] border-2 border-ink bg-cream p-4">
        <div className="grid gap-3 sm:grid-cols-3">
          <input className="input mono" value={label} onChange={(e) => setLabel(e.target.value.toLowerCase().trim())} placeholder="label: son" aria-label="label" />
          <input className="input" value={relationship} onChange={(e) => setRelationship(e.target.value)} placeholder="relationship: son" aria-label="relationship" />
          <input className="input mono" value={percent} onChange={(e) => setPercent(e.target.value)} placeholder="share %: 60" inputMode="decimal" aria-label="share in percent" />
        </div>
        <HeirTargetInput value={shown} onChange={setTarget} ctx={{ estate, label, grantor, taken, namedAllowed }} problem={targetProblem} />
        <button
          type="button"
          className="btn btn-sm"
          disabled={problem !== undefined}
          onClick={() => {
            onChange({ ...draft, heirs: [...draft.heirs, { label, relationship: relationship || label, bps, ...shown }] });
            setLabel("");
            setRelationship("");
            setPercent("");
            setTarget(namedAllowed ? EMPTY_ACCOUNT : EMPTY_KEY);
          }}
        >
          <PlusIcon size={15} /> add heir
        </button>
        {problem && problem !== targetProblem && (label || percent) ? <p className="hint">{problem}</p> : null}
      </div>
    </div>
  );
}

function VaultStep({ draft, funded, onChange }: { draft: SetupDraft; funded: boolean; onChange: (d: SetupDraft) => void }) {
  return (
    <div className="space-y-5">
      <CardHeading icon={VaultIcon} accent="bg-teal" title="fund the vault" body="what your heirs will share. tokens such as mUSDC can be added from the dashboard afterwards." />
      {funded ? <p className="text-sm font-bold">the vault already holds NEAR; nothing more is sent here.</p> : null}
      <div>
        <label className="label" htmlFor="vault-near">NEAR into the vault</label>
        <input id="vault-near" className="input mono" value={draft.depositNear} onChange={(e) => onChange({ ...draft, depositNear: e.target.value })} inputMode="decimal" disabled={funded} />
        <p className="hint">optional; leave empty to fund later. separately, opening the estate costs {formatNear(OPENING_DEPOSIT)} NEAR for its own storage, and each heir 0.01 NEAR if you named their account, or 0.2 NEAR if they gave a key (their record, and 0.1 NEAR on their new account for gas); surpluses come back.</p>
      </div>
    </div>
  );
}

function ReviewStep({ draft, problems, progress, onDone }: { draft: SetupDraft; problems: string[]; progress: ReturnType<typeof useSetupProgress>["data"] | null; onDone: () => void }) {
  const router = useRouter();
  const [checking, setChecking] = useState(false);
  const tx = useTx();
  const id = isLabel(draft.label) ? estateId(draft.label, FACTORY) : "";
  const p: SetupProgress = progress ?? NOTHING_YET;
  const needsFace = !p.faceBound;
  const ready = problems.length === 0 && progress !== null;
  const deposit = vaultDeposit(draft) ?? BigInt(0);
  const finish = () => {
    onDone();
    router.push(`/dashboard?estate=${encodeURIComponent(id)}`);
  };

  return (
    <div className="space-y-5">
      <CardHeading icon={SelfieIcon} accent="bg-lavender" title="prove you're alive, and open it" body="a World ID Selfie Check binds your face to the estate and starts the clock. then one approval in your wallet sends everything." />
      <dl className="space-y-2 text-sm">
        <DataRow label="estate" value={id || "—"} />
        <DataRow label="clock" value={`${formatDuration(draft.intervalSec)} + ${formatDuration(draft.graceSec)} grace`} />
        <DataRow label="heirs" value={draft.heirs.map((h) => `${h.label} ${bpsToPercent(h.bps)}`).join(", ") || "—"} />
        <DataRow label="vault" value={deposit > BigInt(0) ? `${formatNear(deposit)} NEAR` : "later"} />
      </dl>

      <ol className="space-y-1.5 text-sm">
        <Todo done={p.exists} text="open the estate" />
        <Todo done={p.faceBound} text="first check-in: your Selfie Check binds your face" />
        {draft.heirs.map((h) => <Todo key={h.label} done={p.heirs.includes(h.label)} text={`add ${h.label} (${bpsToPercent(h.bps)})`} />)}
        {deposit > BigInt(0) ? <Todo done={p.vaultNear > BigInt(0)} text={`deposit ${formatNear(deposit)} NEAR`} /> : null}
      </ol>

      {problems.length ? (
        <ul className="space-y-1.5">
          {problems.map((problem) => (
            <li key={problem} className="flex items-start gap-2 text-xs font-bold text-muted">
              <AlertIcon size={15} /> {problem}
            </li>
          ))}
        </ul>
      ) : null}

      {needsFace ? (
        <button type="button" className="btn btn-pill w-full" disabled={!ready} onClick={() => setChecking(true)}>
          <SelfieIcon size={18} /> selfie check, then open the estate
        </button>
      ) : (
        <button type="button" className="btn btn-pill w-full" disabled={!ready || tx.busy} onClick={() => void tx.send(setupPlan(FACTORY, draft, p, null)).then((r) => r && finish())}>
          {tx.busy ? "confirm in your wallet…" : "finish the setup"}
        </button>
      )}
      <TxStatus tx={tx} success="set up" />

      <SelfieCheckModal
        open={checking}
        purpose={{ kind: "checkin", estateLabel: draft.label }}
        // Worked out when the button is pressed, from what's on chain by then.
        transaction={(signed) => setupPlan(FACTORY, draft, p, signed)}
        actionLabel="approve the setup in your wallet"
        successCopy="your estate is live: face bound, clock running, heirs added."
        onDone={finish}
        onClose={() => setChecking(false)}
      />
    </div>
  );
}

function Todo({ done, text }: { done: boolean; text: string }) {
  return (
    <li className={`flex items-center gap-2 ${done ? "text-muted line-through" : ""}`}>
      <IconCircle size="2xs" accent={done ? "bg-teal" : "bg-surface"}>{done ? <CheckIcon size={10} /> : null}</IconCircle>
      {text}
    </li>
  );
}

function PresetGroup({ legend, presets, value, onChange }: { legend: string; presets: readonly Preset[]; value: number; onChange: (s: number) => void }) {
  return (
    <fieldset>
      <legend className="label">{legend}</legend>
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        {presets.map((p) => (
          <button key={p.seconds} type="button" onClick={() => onChange(p.seconds)} aria-pressed={value === p.seconds} className={`rounded-[8px] border-2 border-ink px-3 py-2 text-left text-sm font-bold ${value === p.seconds ? "bg-yellow shadow-brut-sm" : "bg-surface"}`}>
            {p.label}
            {p.note ? <span className="block text-[0.68rem] font-medium text-muted">{p.note}</span> : null}
          </button>
        ))}
      </div>
    </fieldset>
  );
}
