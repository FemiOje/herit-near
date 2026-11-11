import { CountdownRing } from "@/components/estate/countdown-ring";
import { StatusPill } from "@/components/estate/status-pill";
import { Blob, Star } from "@/components/ui/deco";
import { HeirIcon, LockIcon } from "@/components/ui/icons";
import { IconCircle } from "@/components/ui/icon-circle";
import { FACTORY } from "@/lib/near/config";

/** A sample estate as the dashboard draws it: a fixed picture, so it never drifts into grace. */
const SAMPLE = {
  estate: `alice.${FACTORY}`,
  heirs: [
    { account: `son.alice.${FACTORY}`, relationship: "son · made from his key", share: "60%", color: "bg-coral" },
    { account: "ada.near", relationship: "daughter · her own account", share: "40%", color: "bg-purple" },
  ],
};

export function EstatePreviewCard() {
  return (
    <div className="relative">
      <Star className="-left-5 -top-5 z-20" size={34} fill="#F9A8B8" rotate={18} />

      <div className="card relative z-10 overflow-hidden p-5 sm:p-6">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <p className="text-[0.72rem] font-bold tracking-wide text-muted">estate</p>
            <p className="mono truncate text-lg font-extrabold">{SAMPLE.estate}</p>
          </div>
          <StatusPill status="active" />
        </div>

        <div className="my-6 flex justify-center">
          <CountdownRing progress={0.62} status="active" size={190}>
            <p className="text-[0.68rem] font-bold tracking-wide text-muted">next check-in</p>
            <p className="mono text-3xl font-extrabold leading-tight">18d 14h</p>
            <p className="mt-0.5 text-[0.68rem] text-muted">of a 30d window</p>
          </CountdownRing>
        </div>

        <div className="space-y-2">
          {SAMPLE.heirs.map((h) => (
            <div key={h.account} className="card-flat flex items-center gap-3 px-3 py-2.5">
              <IconCircle size="sm" accent={h.color}><HeirIcon size={14} /></IconCircle>
              <div className="min-w-0 flex-1">
                <p className="mono truncate text-[0.78rem] font-bold">{h.account}</p>
                <p className="text-[0.68rem] text-muted">{h.relationship}</p>
              </div>
              <span className="tag">{h.share}</span>
            </div>
          ))}
        </div>

        <div className="mt-4 flex items-center justify-between rounded-[8px] border-2 border-ink bg-cream px-3.5 py-2.5">
          <span className="flex items-center gap-2 text-xs font-bold">
            <LockIcon size={16} />
            heir credentials (NEP-171)
          </span>
          <span className="mono text-xs font-bold">locked</span>
        </div>
      </div>

      <Blob className="-bottom-6 -right-6 z-0" size={72} fill="#4ECDC4" />
    </div>
  );
}
