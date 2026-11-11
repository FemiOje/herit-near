"use client";

import { ClockIcon, LockIcon, UnlockIcon } from "@/components/ui/icons";
import { IconCircle } from "@/components/ui/icon-circle";
import type { EstateStatus } from "@/lib/estate";
import { formatCountdown, formatStamp, graceStartsAt, unlocksAt } from "@/lib/estate/clock";
import { useNow } from "@/lib/estate/use-now";
import type { EstateHeader } from "@/lib/near/estate-state";

/** Where the estate stands, from the heir's side: sealed, in grace, or theirs to claim. */
export function UnlockBanner({ header, status }: { header: EstateHeader; status: EstateStatus }) {
  const now = useNow();
  const unlockAt = unlocksAt(header);
  const left = unlockAt === null ? null : Math.max(0, Math.floor((unlockAt - now) / 1000));

  const banner =
    status === "unlocked"
      ? { icon: <UnlockIcon size={20} />, tone: "bg-coral text-white", title: "the estate has unlocked", body: `the grantor stopped checking in; grace ran out ${formatStamp(unlockAt)}. your share can be claimed.`, tag: "claimable" }
      : status === "grace"
        ? { icon: <ClockIcon size={20} />, tone: "bg-yellow", title: "a check-in was missed: grace has started", body: `if the grantor doesn't check in, it unlocks ${formatStamp(unlockAt)}. one selfie check from them cancels it.`, tag: left === null ? "grace" : `unlocks in ${formatCountdown(left)}` }
        : unlockAt === null
          ? { icon: <LockIcon size={20} />, tone: "bg-teal", title: "sealed: the grantor's clock hasn't started", body: "the estate starts counting at the grantor's first selfie check.", tag: "sealed" }
          : { icon: <LockIcon size={20} />, tone: "bg-teal", title: "sealed: the grantor is checking in", body: `their window closes ${formatStamp(graceStartsAt(header))}. a check-in pushes it back; only missing it and the grace after it unlocks the estate.`, tag: left === null ? "sealed" : `at the earliest in ${formatCountdown(left)}` };

  return (
    <div className={`flex flex-wrap items-center gap-4 rounded-[10px] border-2 border-ink px-5 py-4 shadow-brut ${banner.tone}`}>
      <IconCircle size="xl" accent="bg-surface" className="text-ink">{banner.icon}</IconCircle>
      <div className="min-w-[16rem] flex-1">
        <p className="text-sm font-bold">{banner.title}</p>
        <p className="mt-0.5 text-xs opacity-85">{banner.body}</p>
      </div>
      <span className="tag tag-shadow flex-shrink-0 bg-surface text-ink">{banner.tag}</span>
    </div>
  );
}
