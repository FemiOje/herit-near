import { notFound } from "next/navigation";

import { SelfieSpike } from "./selfie-spike";

/**
 * DEV ONLY: runs a real Selfie Check and shows the worker input it produces, for the Checkpoint 5
 * spike (herit-docs/checkpoints/05-outlayer-selfie-check.md). Not served in production builds.
 */
export default function SelfieSpikePage() {
  if (process.env.NODE_ENV === "production") notFound();
  return (
    <main className="mx-auto w-full max-w-2xl px-4 py-10">
      <h1 className="text-2xl font-semibold">Selfie Check spike</h1>
      <p className="mt-2 text-sm text-zinc-600 dark:text-zinc-400">
        Runs a World ID Selfie Check, then shows the input for the OutLayer worker. Save it to a
        file and pass it to <code>near/scripts/spike-check-in.sh</code>.
      </p>
      <SelfieSpike />
    </main>
  );
}
