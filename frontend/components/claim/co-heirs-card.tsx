"use client";

import { heirColor } from "@/components/heirs/heirs-card";
import { CardHeading } from "@/components/ui/card-heading";
import { HeirIcon } from "@/components/ui/icons";
import { IconCircle } from "@/components/ui/icon-circle";
import type { HeirView } from "@/lib/near/contracts";
import { bpsToPercent } from "@/lib/near/format";

/** The others named in the same estate. */
export function CoHeirsCard({ heirs, label }: { heirs: readonly HeirView[]; label: string }) {
  const others = heirs.map((h, index) => ({ h, index })).filter(({ h }) => h.label !== label);
  return (
    <section className="card p-6">
      <CardHeading icon={HeirIcon} accent="bg-pink" title="co-heirs" body="the others named in this estate" />
      {others.length === 0 ? (
        <p className="mt-4 text-sm text-muted">no one else is named.</p>
      ) : (
        <ul className="mt-4 space-y-2">
          {others.map(({ h, index }) => (
            <li key={h.label} className="flex items-center gap-3">
              <IconCircle size="sm" accent={heirColor(index)}><HeirIcon size={14} /></IconCircle>
              <span className="min-w-0 flex-1">
                <span className="block text-sm font-bold">{h.relationship || h.label}</span>
                <span className="mono block truncate text-[0.7rem] text-muted">{h.account_id}</span>
              </span>
              <span className="tag">{bpsToPercent(h.default_bps)}</span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
