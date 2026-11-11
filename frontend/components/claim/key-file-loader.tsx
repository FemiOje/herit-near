"use client";

import { useState } from "react";

import { ShieldIcon } from "@/components/ui/icons";
import { KeyFileError, keyProblem, parseKeyFile } from "@/lib/near/key-file";
import { keySigner, type KeySigner } from "@/lib/near/key-signer";

/**
 * Opens a key heir's key file and checks it signs for `account`. The file is read here, in the
 * browser, and the key kept in memory only: it is never stored, and never sent anywhere. Only the
 * transactions it signs leave the page.
 */
export function KeyFileLoader({ account, signer, onChange }: { account: string; signer: KeySigner | null; onChange: (s: KeySigner | null) => void }) {
  const [error, setError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);

  async function open(file: File) {
    setError(null);
    setChecking(true);
    try {
      const key = parseKeyFile(await file.text());
      const problem = await keyProblem(key, account);
      if (problem) throw new KeyFileError(problem);
      onChange(keySigner(account, key));
    } catch (cause) {
      onChange(null);
      setError(cause instanceof KeyFileError ? cause.message : "couldn't check the key against NEAR; try again");
    } finally {
      setChecking(false);
    }
  }

  if (signer) {
    return (
      <div className="flex items-center gap-2.5 rounded-[8px] border-2 border-ink bg-teal px-3 py-2">
        <ShieldIcon size={16} />
        <p className="flex-1 text-xs font-bold">key for <span className="mono">{account}</span> open in this tab only</p>
        <button type="button" className="btn btn-ghost btn-sm" onClick={() => onChange(null)}>forget it</button>
      </div>
    );
  }

  return (
    <div className="space-y-1.5">
      <label className="btn btn-ghost btn-sm w-full cursor-pointer">
        <ShieldIcon size={15} /> {checking ? "checking the key…" : "open your key file"}
        <input
          type="file"
          accept="application/json,.json"
          className="sr-only"
          disabled={checking}
          onChange={(e) => {
            const file = e.target.files?.[0];
            e.target.value = "";
            if (file) void open(file);
          }}
        />
      </label>
      <p className="hint">{error ?? "the file you saved when you made your key. it stays on this device; only the signed claim is sent."}</p>
    </div>
  );
}
