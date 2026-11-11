import { Suspense } from "react";

import { ClaimView } from "@/components/claim/claim-view";

export default function ClaimPage() {
  return (
    <div className="mx-auto max-w-6xl space-y-8">
      {/* The slot comes from `?estate=&heir=`, which only the browser knows. */}
      <Suspense fallback={<p className="text-sm text-muted">loading…</p>}>
        <ClaimView />
      </Suspense>
    </div>
  );
}
