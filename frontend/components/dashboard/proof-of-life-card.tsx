"use client";

import { useState } from "react";

import { CountdownRing } from "@/components/estate/countdown-ring";
import { SelfieCheckModal } from "@/components/selfie-check/selfie-check-modal";
import { CardHeading } from "@/components/ui/card-heading";
import { DataRow } from "@/components/ui/data-row";
import { Sparkle } from "@/components/ui/deco";
import { ClockIcon, SelfieIcon } from "@/components/ui/icons";
import { STATUS_COPY, type EstateStatus } from "@/lib/estate";
import {
  formatCountdown,
  formatDuration,
  formatStamp,
  graceStartsAt,
  lastCheckInAt,
  phase,
  unlocksAt,
} from "@/lib/estate/clock";
import { useNow } from "@/lib/estate/use-now";
import { labelOf } from "@/lib/near/account";
import { calls } from "@/lib/near/calls";
import type { EstateHeader } from "@/lib/near/estate-state";

/** The check-in clock and the Selfie Check that resets it. */
export function ProofOfLifeCard({
  id,
  header,
  status,
  isGrantor,
}: {
  id: string;
  header: EstateHeader;
  status: EstateStatus;
  isGrantor: boolean;
}) {
  const [checkingIn, setCheckingIn] = useState(false);
  const now = useNow();
  const started = header.lastCheckInNs !== BigInt(0);
  const unlocked = status === "unlocked";
  const { remainingSec, totalSec, progress } = phase(status, header, now);
  // The local clock says the phase is over, but the chain hasn't been read since.
  const overdue = started && !unlocked && remainingSec === 0;

  const counting = unlocked
    ? "heirs unlocked"
    : !started
      ? "clock not started"
      : status === "grace"
        ? "heirs unlock in"
        : "next check-in in";
  const body = !started ? "the clock starts at your first selfie check" : STATUS_COPY[status].blurb;
  const blocked = !isGrantor
    ? "only the grantor can check in"
    : unlocked
      ? "the estate has unlocked: a check-in can no longer reverse it"
      : undefined;

  return (
    <section className="card relative overflow-hidden p-6">
      <Sparkle className="right-5 top-5" size={22} fill="#C4B5FD" rotate={14} />
      <CardHeading icon={ClockIcon} accent="bg-lavender" title="proof of life" body={body} />

      <div className="my-7 flex justify-center">
        <CountdownRing progress={progress} status={status} size={216}>
          <p className="text-[0.68rem] font-bold tracking-wide text-muted">{counting}</p>
          <p className="mono text-4xl font-extrabold leading-tight">{started && !unlocked ? formatCountdown(remainingSec) : "—"}</p>
          <p className="mt-1 text-[0.68rem] text-muted">{overdue ? "checking the chain…" : `of ${formatDuration(totalSec)}`}</p>
        </CountdownRing>
      </div>

      <button type="button" className="btn btn-pill w-full" disabled={blocked !== undefined} onClick={() => setCheckingIn(true)}>
        <SelfieIcon size={18} />
        {status === "grace" ? "check in and cancel grace" : started ? "check in now" : "first check-in: start the clock"}
      </button>
      {blocked ? <p className="hint text-center">{blocked}</p> : null}
      {!header.faceBound && !blocked ? (
        <p className="hint text-center">your first selfie check binds your face to this estate; later ones must match it.</p>
      ) : null}

      <dl className="mt-6 space-y-2.5 border-t-2 border-ink pt-5 text-sm">
        <DataRow label="last selfie check" value={formatStamp(lastCheckInAt(header))} />
        <DataRow label="window closes" value={formatStamp(graceStartsAt(header))} />
        <DataRow label="heirs unlock at" value={formatStamp(unlocksAt(header))} />
        <DataRow label="interval · grace" value={`${formatDuration(header.intervalSec)} · ${formatDuration(header.graceSec)}`} />
      </dl>

      <SelfieCheckModal
        open={checkingIn}
        purpose={{ kind: "checkin", estateLabel: labelOf(id) }}
        transaction={(signed) => calls.checkIn(id, signed)}
        actionLabel="send the check-in"
        successCopy="checked in. the clock has restarted."
        onClose={() => setCheckingIn(false)}
      />
    </section>
  );
}
