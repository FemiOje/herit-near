import { Suspense } from "react";

import { GrantorEstates } from "@/components/dashboard/estate-view";
import { KeyMaker } from "@/components/heirs/key-maker";
import { PageHeader } from "@/components/layout/page-header";

export default function HeirsPage() {
  return (
    <div className="mx-auto max-w-6xl space-y-8">
      <PageHeader
        eyebrow="grantor"
        title="heirs"
        description="An heir is a NEAR account the heir already has, or a new one under the estate holding a key only they have. Shares are in percent of each asset at unlock."
      />
      <div className="grid items-start gap-6 lg:grid-cols-[1.6fr_1fr]">
        <div className="space-y-6">
          <Suspense fallback={<p className="text-sm text-muted">loading…</p>}>
            <GrantorEstates focus="heirs" />
          </Suspense>
        </div>
        <KeyMaker />
      </div>
    </div>
  );
}
