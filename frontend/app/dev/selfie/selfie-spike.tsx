"use client";

import { useState, type ReactNode } from "react";
import { IDKitRequestWidget, selfieCheckLegacy } from "@worldcoin/idkit";
import type { IDKitDebugReport, IDKitErrorCodes, IDKitResult } from "@worldcoin/idkit";

import {
  estateAccountId,
  type SelfieCheckPurpose,
  type SignResponse,
} from "@/lib/selfie-check";

const ACTION_CHECKIN = 0;
const ACTION_CLAIM = 1;

/** The form, the World ID widget, and the worker input it produces. */
export function SelfieSpike() {
  const [estateLabel, setEstateLabel] = useState("spike");
  const [subject, setSubject] = useState("alice-kaykay.testnet");
  const [heirLabel, setHeirLabel] = useState("");
  const [config, setConfig] = useState<SignResponse | null>(null);
  const [workerInput, setWorkerInput] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const purpose: SelfieCheckPurpose = heirLabel
    ? { kind: "claim", estateLabel, heirLabel }
    : { kind: "checkin", estateLabel };

  async function start() {
    setBusy(true);
    setError(null);
    setWorkerInput(null);
    try {
      const res = await fetch("/api/worldid/sign", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ purpose }),
      });
      const body = await res.json();
      if (!res.ok) throw new Error(body.error ?? `sign route returned ${res.status}`);
      setConfig(body as SignResponse);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  function done(result: IDKitResult) {
    const input = {
      operation: "attest",
      estate_id: estateAccountId(estateLabel),
      subject,
      action: purpose.kind === "claim" ? ACTION_CLAIM : ACTION_CHECKIN,
      heir_label: purpose.kind === "claim" ? purpose.heirLabel : null,
      result,
    };
    setWorkerInput(JSON.stringify({ input }, null, 2));
    setConfig(null);
  }

  return (
    <div className="mt-6 flex flex-col gap-4">
      <Field label="Estate label" hint="estate account = <label>.herit-near.testnet">
        <input className={inputClass} value={estateLabel} onChange={(e) => setEstateLabel(e.target.value)} />
      </Field>
      <Field label="Subject" hint="the NEAR account that will send the transaction">
        <input className={inputClass} value={subject} onChange={(e) => setSubject(e.target.value)} />
      </Field>
      <Field label="Heir label" hint="leave empty for a check-in; set it for a claim">
        <input className={inputClass} value={heirLabel} onChange={(e) => setHeirLabel(e.target.value)} />
      </Field>

      <button
        className="rounded bg-black px-4 py-2 text-white disabled:opacity-50 dark:bg-white dark:text-black"
        onClick={start}
        disabled={busy || config !== null}
      >
        {busy ? "Signing request…" : "Start Selfie Check"}
      </button>

      {error ? <p className="text-sm text-red-600">{error}</p> : null}

      {config ? (
        <IDKitRequestWidget
          open
          onOpenChange={(open) => {
            if (!open) setConfig(null);
          }}
          app_id={config.app_id}
          // The server's string, never recomputed: the RP signature covers this exact action.
          action={config.action}
          rp_context={config.rp_context}
          environment={config.environment}
          // Selfie Check is a World ID 3.0 credential, which is what "Legacy" means here.
          allow_legacy_proofs
          // The heir travels as the signal, so one human claiming two slots gets one nullifier.
          preset={selfieCheckLegacy(purpose.kind === "claim" ? { signal: purpose.heirLabel } : {})}
          onSuccess={done}
          onError={(code: IDKitErrorCodes, debugReport?: IDKitDebugReport) => {
            setError(`World ID error: ${code}`);
            if (debugReport) console.error("[selfie-spike]", debugReport);
            setConfig(null);
          }}
        />
      ) : null}

      {workerInput ? (
        <div className="flex flex-col gap-2">
          <p className="text-sm">
            Worker input. Copy it into <code>near/scripts/spike-input.json</code> (gitignored).
          </p>
          <button
            className="self-start rounded border px-3 py-1 text-sm"
            onClick={() => navigator.clipboard.writeText(workerInput)}
          >
            Copy
          </button>
          <pre className="max-h-96 overflow-auto rounded bg-zinc-100 p-3 text-xs dark:bg-zinc-900">
            {workerInput}
          </pre>
        </div>
      ) : null}
    </div>
  );
}

const inputClass =
  "w-full rounded border border-zinc-300 bg-transparent px-3 py-2 text-sm dark:border-zinc-700";

function Field({ label, hint, children }: { label: string; hint: string; children: ReactNode }) {
  return (
    <label className="flex flex-col gap-1 text-sm">
      <span className="font-medium">{label}</span>
      {children}
      <span className="text-xs text-zinc-500">{hint}</span>
    </label>
  );
}
