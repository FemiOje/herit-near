"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { IDKitRequestWidget, selfieCheckLegacy } from "@worldcoin/idkit";
import type { IDKitDebugReport, IDKitErrorCodes, IDKitResult } from "@worldcoin/idkit";

import { TxStatus } from "@/components/estate/tx-status";
import { Star } from "@/components/ui/deco";
import { FieldRow } from "@/components/ui/data-row";
import { AlertIcon, CheckIcon, SelfieIcon, ShieldIcon } from "@/components/ui/icons";
import { StageList } from "@/components/ui/stage-list";
import type { Call, Signed } from "@/lib/near/calls";
import { nsToMs } from "@/lib/near/format";
import { useTx } from "@/lib/near/use-tx";
import { useWallet } from "@/lib/near/wallet";
import type { SelfieCheckPurpose, SignResponse } from "@/lib/selfie-check";

/**
 * The Selfie Check, then the transaction it unlocks.
 *
 *   1. `POST /api/worldid/sign`  an RP signature, so IDKit will raise the check at all
 *   2. the widget                the person proves liveness in World App; a proof comes back
 *   3. `POST /api/attest`        World verifies the proof; the attestor signs what the estate checks
 *   4. the wallet                the person sends `transaction(signed)` from their own account
 *
 * Step 4 waits for a click: a wallet opens a popup, and a popup opened at the end of an async chain
 * is blocked. The attestation stays valid for 15 minutes, so a failed send can be retried.
 */

const STAGES = [
  { label: "request signed", detail: "Herit asks World ID for a Selfie Check" },
  { label: "selfie check", detail: "scan the code with World App and take the selfie" },
  { label: "verified by World", detail: "World checks the proof; nothing about your face is kept" },
  { label: "attestation signed", detail: "bound to this estate, your account and one nonce" },
  { label: "sent to NEAR", detail: "your wallet signs the transaction" },
] as const;

export type SelfieCheckProps = {
  open: boolean;
  purpose: SelfieCheckPurpose;
  /** The transaction the attestation goes into. */
  transaction: (signed: Signed) => Call;
  /** What the send button says: "check in", "withdraw 1 NEAR". */
  actionLabel: string;
  successCopy: string;
  onClose: () => void;
};

/** Mounted fresh per open, so a run never inherits the last one's state. */
export function SelfieCheckModal({ open, ...props }: SelfieCheckProps) {
  if (!open) return null;
  return <SelfieCheckDialog {...props} />;
}

function SelfieCheckDialog({ purpose, transaction, actionLabel, successCopy, onClose }: Omit<SelfieCheckProps, "open">) {
  const { accountId } = useWallet();
  const tx = useTx();
  const [stage, setStage] = useState(0);
  const [config, setConfig] = useState<SignResponse | null>(null);
  const [signed, setSigned] = useState<Signed | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    fetch("/api/worldid/sign", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ purpose }),
    })
      .then(async (response) => {
        const body = await response.json();
        if (!response.ok) throw new Error(body.error ?? "could not start the check");
        return body as SignResponse;
      })
      .then((body) => {
        if (!live) return;
        setConfig(body);
        setStage(1);
      })
      .catch((cause: unknown) => {
        if (live) setError(cause instanceof Error ? cause.message : "could not reach the verification service");
      });
    return () => {
      live = false;
    };
    // Once per mount: the dialog remounts on every open.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** Why our own route refused, kept across IDKit's error boundary, which drops it. */
  const hostRejection = useRef<string | null>(null);

  // Throwing here rejects the proof inside IDKit, so one our server refuses never "succeeds".
  const handleVerify = useCallback(
    async (result: IDKitResult) => {
      if (!accountId) throw new Error("connect a wallet before verifying");
      setStage(2);
      hostRejection.current = null;
      const response = await fetch("/api/attest", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        // The account that will send the transaction: the estate checks the attestation names it.
        body: JSON.stringify({ purpose, subject: accountId, result }),
      });
      const body = await response.json();
      if (!response.ok) {
        hostRejection.current = body.error ?? "the proof was rejected";
        throw new Error(hostRejection.current!);
      }
      setSigned(body as Signed);
      setStage(4);
    },
    [accountId, purpose],
  );

  const sent = tx.phase === "success";
  const current = sent ? STAGES.length : stage;
  const failure = error;
  const closable = !tx.busy;

  useEffect(() => {
    if (!closable) return;
    const onKey = (event: KeyboardEvent) => event.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [closable, onClose]);

  const title = failure ? "selfie check failed" : sent ? "done" : signed ? "selfie check passed" : "selfie check";
  const body = failure
    ? failure
    : sent
      ? successCopy
      : signed
        ? "send it from your wallet within 15 minutes."
        : "proving a live, unique human is behind this request.";

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-ink/40 px-4 py-8" role="dialog" aria-modal="true" aria-label="World ID Selfie Check">
      {config && !signed && !error ? (
        <IDKitRequestWidget
          open
          onOpenChange={(next) => {
            if (!next && !signed) setError("the check was cancelled");
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
          handleVerify={handleVerify}
          onSuccess={() => {}}
          onError={(code: IDKitErrorCodes, debugReport?: IDKitDebugReport) => {
            setError(hostRejection.current ?? errorMessage(code, config.environment));
            if (debugReport) console.error("[selfie-check]", debugReport);
          }}
        />
      ) : null}

      <div className="relative w-full max-w-md">
        <Star className="-left-4 -top-4 z-20" size={34} fill="#FFE566" rotate={-12} />
        <div className="card max-h-[88vh] overflow-y-auto p-6">
          <div className="flex items-start gap-3">
            <span className={`icon-box ${failure ? "bg-coral" : sent ? "bg-teal" : "bg-lavender"}`}>
              {failure ? <AlertIcon size={22} /> : sent ? <CheckIcon size={22} /> : <SelfieIcon size={22} />}
            </span>
            <div>
              <h2 className="text-xl">{title}</h2>
              <p className="mt-1 text-sm text-muted">{body}</p>
            </div>
          </div>

          <div className="mt-5 rounded-[8px] border-2 border-ink bg-cream px-4 py-3">
            <FieldRow label="World action" value={config?.action ?? "…"} />
            <FieldRow label="account" value={accountId ?? "…"} />
            <FieldRow label="nonce" value={signed?.attestation.nonce ?? "…"} />
            <FieldRow label="expires" value={signed ? expiresIn(signed.attestation.expiry_ns) : "…"} />
          </div>

          <StageList stages={STAGES} current={current} failed={failure !== null || tx.phase === "error"} className="mt-5" />

          {signed && !sent ? (
            <div className="mt-5 flex items-start gap-2.5 rounded-[8px] border-2 border-ink bg-teal px-4 py-3">
              <ShieldIcon size={18} />
              <p className="text-xs font-medium leading-relaxed">
                only <span className="mono">{accountId}</span> can use this attestation, only on this estate, and only once.
              </p>
            </div>
          ) : null}

          <TxStatus tx={tx} success="confirmed on NEAR testnet" className="mt-4" />

          {signed && !sent ? (
            <button type="button" className="btn mt-6 w-full" disabled={tx.busy} onClick={() => void tx.send(transaction(signed))}>
              {tx.busy ? "confirm in your wallet…" : tx.phase === "error" ? `try again: ${actionLabel}` : actionLabel}
            </button>
          ) : null}

          <button type="button" className={`btn btn-ghost w-full ${signed && !sent ? "mt-3" : "mt-6"}`} onClick={onClose} disabled={!closable}>
            {sent || failure ? "close" : "cancel"}
          </button>
        </div>
      </div>
    </div>
  );
}

function expiresIn(expiryNs: string): string {
  const seconds = Math.floor((nsToMs(expiryNs) - Date.now()) / 1000);
  return seconds > 0 ? `in ${Math.ceil(seconds / 60)} min` : "expired";
}

/** Failures a person can act on. */
function errorMessage(code: IDKitErrorCodes, environment: string): string {
  switch (code) {
    case "user_rejected":
    case "verification_rejected":
      return "the check was declined in World App";
    case "credential_unavailable":
    case "feature_unavailable":
    case "world_id_3_not_available":
      return "Selfie Check isn't enabled for this app: a portal setting, not a retry";
    case "invalid_rp_signature":
    case "rp_signature_expired":
    case "timestamp_too_old":
      return "the signed request expired before you finished; start again";
    case "unknown_rp":
    case "inactive_rp":
      return `this app isn't registered in World's ${environment} environment`;
    case "connection_failed":
      return "couldn't reach World App; check the connection and try again";
    default:
      return `World ID couldn't complete the check (${code})`;
  }
}
