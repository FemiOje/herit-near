"use client";

import { useState } from "react";

import { CardHeading } from "@/components/ui/card-heading";
import { PulseIcon } from "@/components/ui/icons";
import { describe } from "@/lib/activity/events";
import { useActivity } from "@/lib/activity/use-activity";
import { formatStamp } from "@/lib/estate/clock";
import { accountUrl, txUrl } from "@/lib/near/config";

const TONE = { good: "bg-teal", warn: "bg-yellow", bad: "bg-coral", plain: "bg-surface" } as const;
const SHOWN = 8;

/** What has happened to the estate, from its own event logs, newest first. */
export function ActivityCard({ id }: { id: string }) {
  const activity = useActivity(id);
  const [all, setAll] = useState(false);
  const events = activity.data?.events ?? [];
  const shown = all ? events : events.slice(0, SHOWN);

  return (
    <section className="card p-6">
      <CardHeading icon={PulseIcon} accent="bg-lavender" title="activity" body="from the estate's own event logs, through FastNEAR" />
      {activity.isPending ? (
        <p className="mt-4 text-sm text-muted">reading the estate&apos;s history…</p>
      ) : activity.isError ? (
        <p className="mt-4 text-sm text-muted">{activity.error.message}</p>
      ) : events.length === 0 ? (
        <p className="mt-4 text-sm text-muted">nothing yet.</p>
      ) : (
        <ol className="mt-5 space-y-3">
          {shown.map((e, i) => {
            const line = describe(e);
            return (
              <li key={`${e.txHash}-${i}`} className="flex items-start gap-3">
                <span className={`mt-1.5 h-2.5 w-2.5 flex-shrink-0 rounded-full border-2 border-ink ${TONE[line.tone]}`} aria-hidden="true" />
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-bold">{line.text}</p>
                  <p className="text-[0.7rem] text-muted">
                    {formatStamp(e.at)} ·{" "}
                    <a href={txUrl(e.txHash)} target="_blank" rel="noopener noreferrer" className="underline">
                      tx
                    </a>{" "}
                    · by{" "}
                    <a href={accountUrl(e.by)} target="_blank" rel="noopener noreferrer" className="mono underline">
                      {e.by}
                    </a>
                  </p>
                </div>
              </li>
            );
          })}
        </ol>
      )}
      {events.length > SHOWN ? (
        <button type="button" className="btn btn-ghost btn-sm mt-4" onClick={() => setAll(!all)}>
          {all ? "show fewer" : `show all ${events.length}`}
        </button>
      ) : null}
      {activity.data?.more && all ? <p className="hint mt-2">the latest 50 transactions; older ones are on NearBlocks.</p> : null}
    </section>
  );
}
