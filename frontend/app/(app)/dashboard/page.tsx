import { Suspense } from "react";

import { GrantorEstates } from "@/components/dashboard/estate-view";
import { PageHeader } from "@/components/layout/page-header";

export default function DashboardPage() {
  return (
    <div className="mx-auto max-w-6xl space-y-8">
      <PageHeader eyebrow="grantor" title="your estates" description="Each estate is its own keyless NEAR account, opened through the Herit factory." />
      {/* The chosen estate comes from `?estate=`, which only the browser knows. */}
      <Suspense fallback={<p className="text-sm text-muted">loading…</p>}>
        <GrantorEstates focus="dashboard" />
      </Suspense>
    </div>
  );
}
