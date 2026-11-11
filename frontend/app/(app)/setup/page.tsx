import { Suspense } from "react";

import { PageHeader } from "@/components/layout/page-header";
import { SetupWizard } from "@/components/setup/setup-wizard";

export default function SetupPage() {
  return (
    <div className="mx-auto max-w-4xl space-y-6">
      <PageHeader
        eyebrow="grantor"
        title="open an estate"
        description="Name it, set the clock, choose your heirs and fund the vault. Then one Selfie Check and one wallet approval make it live."
      />
      {/* `?estate=` resumes a setup, and only the browser knows it. */}
      <Suspense fallback={<p className="text-sm text-muted">loading…</p>}>
        <SetupWizard />
      </Suspense>
    </div>
  );
}
