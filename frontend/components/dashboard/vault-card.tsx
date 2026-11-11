"use client";

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { TxStatus } from "@/components/estate/tx-status";
import { SelfieCheckModal } from "@/components/selfie-check/selfie-check-modal";
import { CardHeading } from "@/components/ui/card-heading";
import { PlusIcon, VaultIcon } from "@/components/ui/icons";
import { assetKey, type AssetInfo } from "@/lib/estate/use-estate";
import { labelOf } from "@/lib/near/account";
import { calls } from "@/lib/near/calls";
import { MOCK_USDC, NETWORK } from "@/lib/near/config";
import { ft, type Asset } from "@/lib/near/contracts";
import { formatNear, formatUnits, parseUnits } from "@/lib/near/format";
import { viewAccount } from "@/lib/near/rpc";
import { useTx } from "@/lib/near/use-tx";

export type VaultRow = { info: AssetInfo; balance: bigint | undefined };

/**
 * What the estate holds, and the grantor's ways in and out: deposit NEAR, add a token and deposit
 * it, and withdraw anything with a fresh Selfie Check from the face the first check-in bound.
 */
export function VaultCard({
  id,
  grantor,
  rows,
  faceBound,
  open,
  isGrantor,
}: {
  id: string;
  grantor: string;
  rows: readonly VaultRow[];
  faceBound: boolean;
  /** Not unlocked: deposits and withdrawals are refused after unlock. */
  open: boolean;
  isGrantor: boolean;
}) {
  const nearListed = rows.some((r) => r.info.asset === null);
  const display: VaultRow[] = nearListed ? [...rows] : [{ info: { asset: null, symbol: "NEAR", decimals: 24 }, balance: BigInt(0) }, ...rows];
  const writable = isGrantor && open;

  return (
    <section className="card p-6">
      <CardHeading icon={VaultIcon} accent="bg-teal" title="vault" body="what your heirs share at unlock. the estate keeps its own books." />

      <ul className="mt-5 divide-y-2 divide-ink rounded-[8px] border-2 border-ink">
        {display.map((row) => (
          <AssetRow key={assetKey(row.info.asset)} id={id} grantor={grantor} row={row} writable={writable} faceBound={faceBound} />
        ))}
      </ul>

      {writable ? <AddToken id={id} listed={rows.map((r) => r.info.asset)} /> : null}
      {!isGrantor ? <p className="hint">read only: this wallet isn&apos;t the grantor.</p> : !open ? <p className="hint">the estate has unlocked: the vault belongs to the heirs now.</p> : null}
    </section>
  );
}

function AssetRow({ id, grantor, row, writable, faceBound }: { id: string; grantor: string; row: VaultRow; writable: boolean; faceBound: boolean }) {
  const { asset, symbol, decimals } = row.info;
  const [mode, setMode] = useState<"deposit" | "withdraw" | null>(null);
  const [amount, setAmount] = useState("");
  const [withdrawing, setWithdrawing] = useState(false);
  const tx = useTx();

  const wallet = useQuery({
    queryKey: ["wallet_balance", grantor, assetKey(asset)],
    enabled: writable,
    queryFn: async () => (asset === null ? BigInt((await viewAccount(grantor))?.amount ?? 0) : BigInt(await ft(asset).balanceOf(grantor))),
  });

  let units: bigint | null = null;
  try {
    units = amount ? parseUnits(amount, decimals) : null;
  } catch {
    units = null;
  }
  const balance = row.balance ?? BigInt(0);
  const problem =
    units === null || units === BigInt(0)
      ? "enter an amount"
      : mode === "withdraw" && units > balance
        ? "more than the vault holds"
        : mode === "deposit" && wallet.data !== undefined && units > wallet.data
          ? "more than your wallet holds"
          : undefined;

  function submit() {
    if (units === null) return;
    if (mode === "deposit") {
      void tx.send(asset === null ? calls.depositNear(id, units) : calls.depositToken(id, asset, units));
    } else {
      setWithdrawing(true);
    }
  }

  return (
    <li className="p-4">
      <div className="flex items-baseline justify-between gap-3">
        <div>
          <p className="font-bold">{symbol}</p>
          <p className="mono text-[0.7rem] text-muted">{asset ?? "native"}</p>
        </div>
        <p className="mono text-lg font-extrabold">{row.balance === undefined ? "…" : formatUnits(row.balance, decimals)}</p>
      </div>

      {writable ? (
        <div className="mt-3 flex flex-wrap gap-2">
          <button type="button" className={`btn btn-sm ${mode === "deposit" ? "btn-yellow" : "btn-ghost"}`} onClick={() => { setMode(mode === "deposit" ? null : "deposit"); tx.reset(); }}>
            deposit
          </button>
          <button
            type="button"
            className={`btn btn-sm ${mode === "withdraw" ? "btn-yellow" : "btn-ghost"}`}
            disabled={!faceBound || balance === BigInt(0)}
            title={faceBound ? undefined : "check in first: a withdrawal needs the face your first check-in bound"}
            onClick={() => { setMode(mode === "withdraw" ? null : "withdraw"); tx.reset(); }}
          >
            withdraw
          </button>
        </div>
      ) : null}

      {mode ? (
        <div className="mt-3 space-y-2">
          <div className="flex gap-2">
            <input className="input mono" value={amount} onChange={(e) => setAmount(e.target.value)} placeholder={`0.0 ${symbol}`} inputMode="decimal" aria-label={`${mode} amount in ${symbol}`} />
            <button type="button" className="btn btn-sm" disabled={problem !== undefined || tx.busy} onClick={submit}>
              {tx.busy ? "…" : mode}
            </button>
          </div>
          <p className="hint">
            {mode === "deposit"
              ? `your wallet: ${wallet.data === undefined ? "…" : asset === null ? formatNear(wallet.data) : formatUnits(wallet.data, decimals)} ${symbol}${asset === null ? " (keep some for gas)" : ""}`
              : "a withdrawal needs a fresh selfie check from the face your first check-in bound."}
            {problem && amount ? ` · ${problem}` : ""}
          </p>
          {mode === "deposit" && asset !== null && wallet.data === BigInt(0) && asset === MOCK_USDC.id && NETWORK === "testnet" ? (
            <p className="hint">no {symbol} in this wallet yet: the mock token has no faucet, so send some from the account that holds the supply.</p>
          ) : null}
          <TxStatus tx={tx} success={`deposited into the estate`} />
        </div>
      ) : null}

      <SelfieCheckModal
        open={withdrawing}
        purpose={{ kind: "withdraw", estateLabel: labelOf(id) }}
        transaction={(signed) => calls.withdraw(id, asset as Asset, units ?? BigInt(0), signed)}
        actionLabel={`withdraw ${amount} ${symbol}`}
        successCopy={`sent ${amount} ${symbol} back to your wallet.`}
        onClose={() => {
          setWithdrawing(false);
          setAmount("");
          setMode(null);
        }}
      />
    </li>
  );
}

/** Lists a NEP-141 token with the estate, which registers the estate with the token. */
function AddToken({ id, listed }: { id: string; listed: readonly Asset[] }) {
  const [token, setToken] = useState<string>(NETWORK === "testnet" && !listed.includes(MOCK_USDC.id) ? MOCK_USDC.id : "");
  const [openForm, setOpenForm] = useState(false);
  const tx = useTx();
  const already = listed.includes(token);

  if (!openForm) {
    return (
      <button type="button" className="btn btn-ghost btn-sm mt-4" onClick={() => setOpenForm(true)}>
        <PlusIcon size={15} /> add a token
      </button>
    );
  }
  return (
    <div className="mt-4 space-y-2 rounded-[8px] border-2 border-ink bg-cream p-4">
      <label className="label" htmlFor="add-token">token contract</label>
      <div className="flex gap-2">
        <input id="add-token" className="input mono" value={token} onChange={(e) => setToken(e.target.value.trim())} placeholder="usdc.testnet" />
        <button type="button" className="btn btn-sm" disabled={!token || already || tx.busy} onClick={() => void tx.send(calls.addToken(id, token))}>
          {tx.busy ? "…" : "add"}
        </button>
      </div>
      <p className="hint">{already ? "already in the vault" : "attaches 0.05 NEAR for the listing and the estate's registration with the token; the rest is refunded."}</p>
      <TxStatus tx={tx} success="token added. deposit it from its row above." />
    </div>
  );
}
