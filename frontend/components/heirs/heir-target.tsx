"use client";

import Link from "next/link";

import { isNamed, type HeirTarget } from "@/lib/estate/setup-plan";
import { isImplicitAccount, isLabel, namedHeirProblem } from "@/lib/near/account";
import { isPublicKey } from "@/lib/near/calls";
import { useAccountView } from "@/lib/near/use-account";

/** Where the heir's share goes, and what the estate needs to know about it. */
export type TargetContext = {
  /** The estate's account; null while its name isn't valid yet. */
  estate: string | null;
  /** The heir's label, for the sub-account a key makes: `<label>.<estate>`. */
  label: string;
  grantor: string | null;
  /** Accounts already named as heirs, which can't be named again. */
  taken: readonly string[];
  /** False on an estate running code from before named heirs. */
  namedAllowed: boolean;
};

export const EMPTY_ACCOUNT: HeirTarget = { kind: "account", accountId: "" };
export const EMPTY_KEY: HeirTarget = { kind: "key", publicKey: "" };

/**
 * Why `target` can't be sent yet, or undefined. A named account must exist on chain (an implicit
 * one needn't), so this checks the chain too; "checking…" while it looks.
 */
export function useTargetProblem(target: HeirTarget, ctx: TargetContext): string | undefined {
  const named = isNamed(target);
  const shapeProblem = named
    ? ctx.namedAllowed
      ? namedHeirProblem(target.accountId, ctx.estate, ctx.grantor, ctx.taken)
      : "this estate's code takes heirs by key only"
    : isPublicKey(target.publicKey)
      ? undefined
      : "the heir's public key, ed25519:…";
  const lookup = named && !shapeProblem && !isImplicitAccount(target.accountId) ? target.accountId : null;
  const view = useAccountView(lookup);
  if (shapeProblem) return shapeProblem;
  if (lookup === null) return undefined;
  if (view.isPending) return "checking the account…";
  if (view.isError) return "couldn't reach NEAR to check the account; try again";
  return view.data ? undefined : `${lookup} doesn't exist on NEAR`;
}

/**
 * The heir's side of adding an heir: an account they already have, or the public half of a key
 * they made, which becomes a new account under the estate.
 */
export function HeirTargetInput({ value, onChange, ctx, problem }: { value: HeirTarget; onChange: (t: HeirTarget) => void; ctx: TargetContext; problem: string | undefined }) {
  const named = isNamed(value);
  const subAccount = ctx.estate && isLabel(ctx.label) ? `${ctx.label}.${ctx.estate}` : `<label>.${ctx.estate ?? "<estate>"}`;

  async function makeKey() {
    const { KeyPair } = await import("near-api-js");
    const k = KeyPair.fromRandom("ed25519");
    const publicKey = k.getPublicKey().toString();
    const file = JSON.stringify({ account_id: subAccount.includes("<") ? undefined : subAccount, public_key: publicKey, private_key: k.toString() }, null, 2);
    const url = URL.createObjectURL(new Blob([file], { type: "application/json" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = `herit-heir-${ctx.label || "key"}.json`;
    a.click();
    URL.revokeObjectURL(url);
    onChange({ kind: "key", publicKey });
  }

  return (
    <div className="space-y-2">
      <div className="grid grid-cols-2 gap-2" role="radiogroup" aria-label="how the heir receives">
        <Choice on={named} disabled={!ctx.namedAllowed} onClick={() => onChange(EMPTY_ACCOUNT)} title="their NEAR account" note={ctx.namedAllowed ? "one they already have" : "not on this estate's code"} />
        <Choice on={!named} onClick={() => onChange(EMPTY_KEY)} title="a key they made" note="a new account under the estate" />
      </div>

      {named ? (
        <>
          <input className="input mono" value={value.accountId} onChange={(e) => onChange({ kind: "account", accountId: e.target.value.toLowerCase().trim() })} placeholder="kay.testnet" aria-label="the heir's NEAR account" autoComplete="off" />
          <p className="hint">
            {problem && value.accountId ? problem : "they claim from this account with their own wallet. costs 0.01 NEAR for the record; they pay their claim's gas themselves."}
          </p>
        </>
      ) : (
        <>
          <input className="input mono text-xs" value={value.publicKey} onChange={(e) => onChange({ kind: "key", publicKey: e.target.value.trim() })} placeholder="their public key: ed25519:…" aria-label="the heir's public key" />
          <p className="hint">
            {problem && value.publicKey ? (
              problem
            ) : (
              <>
                the heir makes a key on the <Link href="/heirs#make-a-key" target="_blank" className="underline">heirs page</Link> and sends you the public half; their account will be <span className="mono">{subAccount}</span>, with 0.1 NEAR for gas.
                trying it alone? <button type="button" className="underline" onClick={() => void makeKey()}>make one here</button>: it saves the key file to this device, to hand to the heir.
              </>
            )}
          </p>
        </>
      )}
    </div>
  );
}

function Choice({ on, disabled = false, onClick, title, note }: { on: boolean; disabled?: boolean; onClick: () => void; title: string; note: string }) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={on}
      disabled={disabled}
      onClick={onClick}
      className={`rounded-[8px] border-2 border-ink px-3 py-2 text-left text-sm font-bold disabled:opacity-50 ${on ? "bg-yellow shadow-brut-sm" : "bg-surface"}`}
    >
      {title}
      <span className="block text-[0.68rem] font-medium text-muted">{note}</span>
    </button>
  );
}
