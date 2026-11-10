"use client";

import { useEffect, useRef, useState } from "react";
import { CheckIcon, WalletIcon } from "@/components/ui/icons";
import { accountUrl, NETWORK } from "@/lib/near/config";
import { useWallet } from "@/lib/near/wallet";

/** The navbar's wallet control: a connect button, or the connected account with its menu. */
export function ConnectWallet() {
  const { accountId, loading, connect } = useWallet();

  if (loading) {
    return (
      <span className="tag tag-shadow bg-cream" aria-live="polite">
        <WalletIcon size={14} />
        <span className="mono">loading…</span>
      </span>
    );
  }

  if (!accountId) {
    return (
      <button type="button" className="btn btn-sm whitespace-nowrap" onClick={() => void connect()}>
        <WalletIcon size={14} />
        <span className="hidden min-[380px]:inline">connect wallet</span>
        <span className="min-[380px]:hidden">connect</span>
      </button>
    );
  }

  return <ConnectedPill accountId={accountId} />;
}

function ConnectedPill({ accountId }: { accountId: string }) {
  const { disconnect } = useWallet();
  const [open, setOpen] = useState(false);
  const wrapper = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    function handlePointer(event: MouseEvent) {
      if (!wrapper.current?.contains(event.target as Node)) setOpen(false);
    }
    function handleKey(event: KeyboardEvent) {
      if (event.key === "Escape") setOpen(false);
    }
    document.addEventListener("mousedown", handlePointer);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handlePointer);
      document.removeEventListener("keydown", handleKey);
    };
  }, [open]);

  return (
    <div className="relative" ref={wrapper}>
      <button
        type="button"
        className="tag tag-shadow bg-teal transition-transform hover:translate-y-[1px]"
        onClick={() => setOpen((previous) => !previous)}
        aria-expanded={open}
        aria-haspopup="menu"
      >
        <WalletIcon size={14} />
        <span className="mono max-w-[11rem] truncate">{accountId}</span>
      </button>

      {open ? (
        <div className="card absolute right-0 top-[calc(100%+10px)] w-64 p-3" role="menu" aria-label="wallet">
          <p className="mono break-all px-1 pb-2 text-[0.7rem] text-muted">
            {accountId} · {NETWORK}
          </p>
          <CopyAccount accountId={accountId} />
          <a href={accountUrl(accountId)} target="_blank" rel="noreferrer" className="menu-row" role="menuitem">
            view on nearblocks
          </a>
          <button
            type="button"
            className="menu-row text-coral"
            role="menuitem"
            onClick={() => {
              setOpen(false);
              void disconnect();
            }}
          >
            disconnect
          </button>
        </div>
      ) : null}
    </div>
  );
}

function CopyAccount({ accountId }: { accountId: string }) {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), 1_500);
    return () => window.clearTimeout(timer);
  }, [copied]);

  return (
    <button
      type="button"
      className="menu-row"
      role="menuitem"
      onClick={() => {
        void navigator.clipboard.writeText(accountId).then(() => setCopied(true));
      }}
    >
      {copied ? (
        <>
          <CheckIcon size={14} /> copied
        </>
      ) : (
        "copy account id"
      )}
    </button>
  );
}
