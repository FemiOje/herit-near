"use client";

import { useState } from "react";

import { CardHeading } from "@/components/ui/card-heading";
import { ShieldIcon } from "@/components/ui/icons";

/**
 * For heirs: makes an ed25519 key pair in this browser. The heir sends the grantor only the public
 * half, which `add_heir` puts on their new account; the private half is saved to a file here and
 * never leaves the device. That is what lets the heir, and only the heir, claim later.
 */
export function KeyMaker() {
  const [pair, setPair] = useState<{ publicKey: string; secret: string } | null>(null);
  const [saved, setSaved] = useState(false);
  const [copied, setCopied] = useState(false);

  async function make() {
    const { KeyPair } = await import("near-api-js");
    const k = KeyPair.fromRandom("ed25519");
    setPair({ publicKey: k.getPublicKey().toString(), secret: k.toString() });
    setSaved(false);
    setCopied(false);
  }

  function save() {
    if (!pair) return;
    // The NEAR CLI's key file shape; the account id is filled in once the grantor adds you.
    const file = JSON.stringify({ public_key: pair.publicKey, private_key: pair.secret }, null, 2);
    const url = URL.createObjectURL(new Blob([file], { type: "application/json" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = `herit-heir-key-${pair.publicKey.slice(8, 16)}.json`;
    a.click();
    URL.revokeObjectURL(url);
    setSaved(true);
  }

  return (
    <section id="make-a-key" className="card scroll-mt-28 p-6">
      <CardHeading icon={ShieldIcon} accent="bg-yellow" title="for heirs: make your key" body="only if you have no NEAR account to give the grantor. they never hold this key; you'll need it to claim." />
      <ol className="mt-5 list-decimal space-y-1 pl-5 text-sm text-muted">
        <li>make a key here and save the file somewhere safe</li>
        <li>send the grantor the public key only</li>
        <li>they add you; your account and credential appear under their estate</li>
      </ol>

      {!pair ? (
        <button type="button" className="btn mt-5" onClick={() => void make()}>
          make a key
        </button>
      ) : (
        <div className="mt-5 space-y-3">
          <div>
            <p className="label">public key: send this to the grantor</p>
            <div className="flex gap-2">
              <code className="input mono block break-all text-xs">{pair.publicKey}</code>
              <button
                type="button"
                className="btn btn-sm btn-ghost"
                onClick={() => void navigator.clipboard.writeText(pair.publicKey).then(() => setCopied(true))}
              >
                {copied ? "copied" : "copy"}
              </button>
            </div>
          </div>
          <button type="button" className="btn btn-sm" onClick={save}>
            {saved ? "saved, save again" : "save the private key file"}
          </button>
          <p className="hint">
            the file holds your private key. anyone with it can claim your share; without it, nobody can, including you. it is never sent anywhere.
          </p>
        </div>
      )}
    </section>
  );
}
