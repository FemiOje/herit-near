"use client";

import { useState } from "react";

import { KeyFileLoader } from "@/components/claim/key-file-loader";
import { SelfieCheckModal } from "@/components/selfie-check/selfie-check-modal";
import { CountdownRing } from "@/components/estate/countdown-ring";
import { CardHeading } from "@/components/ui/card-heading";
import { Sparkle } from "@/components/ui/deco";
import { AlertIcon, CheckIcon, SelfieIcon, VaultIcon } from "@/components/ui/icons";
import { IconCircle } from "@/components/ui/icon-circle";
import type { EstateStatus } from "@/lib/estate";
import { assetKey, type AssetInfo } from "@/lib/estate/use-estate";
import { claimCost, spendable } from "@/lib/heir/claim-cost";
import { hasSomethingToClaim, shareLine, type LineState } from "@/lib/heir/share";
import { useHeirAssets } from "@/lib/heir/use-heir";
import { labelOf } from "@/lib/near/account";
import { calls } from "@/lib/near/calls";
import type { HeirView } from "@/lib/near/contracts";
import type { KeySigner } from "@/lib/near/key-signer";
import { FAUCET_URL } from "@/lib/near/config";
import { bpsToPercent, formatNear, formatUnits } from "@/lib/near/format";
import { useAccountView } from "@/lib/near/use-account";
import { useWallet } from "@/lib/near/wallet";

const LINE_COPY: Record<LineState, { tag: string; tone: string }> = {
  none: { tag: "no share", tone: "bg-surface text-muted" },
  expected: { tag: "if it unlocked now", tone: "bg-cream" },
  claimable: { tag: "claimable", tone: "bg-coral text-white" },
  paid: { tag: "paid", tone: "bg-teal" },
};

/** The heir's share of each asset, and what a claim needs. */
export function ShareCard({ id, heir, status, vault }: { id: string; heir: HeirView; status: EstateStatus; vault: { info: AssetInfo; balance: bigint | undefined }[] }) {
  const { accountId } = useWallet();
  const assets = vault.map((v) => v.info.asset);
  const { snapshotTaken, per } = useHeirAssets(id, heir.label, assets);
  const unlocked = status === "unlocked";
  const lines = vault.map((v, i) => {
    const p = per[i];
    if (!p || v.balance === undefined || snapshotTaken === undefined) return { info: v.info, line: null };
    return { info: v.info, line: shareLine({ bps: p.bps, balance: v.balance, snapshot: p.snapshot, snapshotTaken, paid: p.paid, unlocked }) };
  });
  const shared = lines.filter((l) => l.line && l.line.state !== "none");
  const paid = shared.filter((l) => l.line!.state === "paid").length;
  const named = heir.account_id !== `${heir.label}.${id}`;
  // A key heir signs with their key file, opened on this card; a named heir with their wallet.
  const [signer, setSigner] = useState<KeySigner | null>(null);
  const controls = named ? accountId === heir.account_id : signer !== null;
  const [claiming, setClaiming] = useState(false);

  // Sized to what this claim will pay: only unpaid assets with something in them send a transfer.
  const toPay = lines.filter((l) => l.line?.state === "claimable" && l.line.amount > BigInt(0));
  const cost = claimCost(toPay.some((l) => l.info.asset === null) ? 1 : 0, toPay.filter((l) => l.info.asset !== null).length);
  const heirAccount = useAccountView(heir.account_id);
  const balance = heirAccount.data ? spendable(heirAccount.data) : undefined;
  const shortBy = balance !== undefined && balance < cost.held ? cost.held - balance : BigInt(0);
  const allPaid = shared.length > 0 && paid === shared.length;
  const gasRefusal = shortBy > BigInt(0) ? `${heir.account_id} needs ${formatNear(shortBy)} more NEAR for gas` : undefined;
  const refusal = !unlocked
    ? "claims open once the estate unlocks"
    : !controls
      ? !named
        ? "open your key file first"
        : accountId
          ? `connected as ${accountId}; connect ${heir.account_id}`
          : `connect ${heir.account_id}`
        : !hasSomethingToClaim(lines.flatMap((l) => (l.line ? [l.line] : [])))
          ? allPaid
            ? "your share has been paid out"
            : "there is nothing to claim yet"
          : gasRefusal;
  // Shown only when gas is the one thing left in the way.
  const needsGas = gasRefusal !== undefined && refusal === gasRefusal;

  return (
    <section className="card relative overflow-hidden p-6">
      <Sparkle className="right-5 top-5" size={22} fill="#FFE566" rotate={-14} />
      <CardHeading icon={VaultIcon} accent="bg-coral text-white" title="your share" body="paid straight to your account when you claim" />

      <div className="my-7 flex justify-center">
        <CountdownRing progress={1} status={status} size={200}>
          <p className="text-[0.68rem] font-bold tracking-wide text-muted">your share</p>
          <p className="mono text-4xl font-extrabold leading-tight">{bpsToPercent(heir.default_bps)}</p>
          <p className="mt-0.5 text-[0.68rem] text-muted">of each asset, unless set per asset</p>
        </CountdownRing>
      </div>

      <ul className="space-y-2">
        {vault.length === 0 ? <li className="text-center text-xs text-muted">the vault holds nothing yet</li> : null}
        {lines.map(({ info, line }) => (
          <li key={assetKey(info.asset)} className="flex items-center gap-3 rounded-[8px] border-2 border-ink px-3 py-2">
            <span className="w-16 font-bold">{info.symbol}</span>
            <span className="mono flex-1 text-sm">{line ? `${formatUnits(line.amount, info.decimals)} ${info.symbol}` : "…"}</span>
            {line ? <span className={`tag ${LINE_COPY[line.state].tone}`}>{LINE_COPY[line.state].tag}</span> : null}
          </li>
        ))}
      </ul>
      {!unlocked && vault.length ? <p className="hint mt-2">amounts move with the vault until it unlocks; then they freeze.</p> : null}

      {!named && unlocked ? (
        <div className="mt-6">
          <KeyFileLoader account={heir.account_id} signer={signer} onChange={setSigner} />
        </div>
      ) : null}

      <button type="button" className={`btn btn-pill w-full ${!named && unlocked ? "mt-3" : "mt-6"}`} disabled={refusal !== undefined} onClick={() => setClaiming(true)}>
        <SelfieIcon size={18} />
        selfie check and claim
      </button>
      {refusal ? <p className="hint text-center">{refusal}</p> : null}
      {needsGas ? <FundGas account={heir.account_id} shortBy={shortBy} held={cost.held} /> : null}

      <ul className="mt-6 space-y-3 border-t-2 border-ink pt-5">
        <Requirement met={unlocked} label="the estate is unlocked" detail={unlocked ? "the grantor stopped checking in and grace ran out" : "only if the grantor stops checking in and grace runs out"} />
        <Requirement
          met={controls}
          label={named ? "signed from your account" : "signed with your key file"}
          detail={
            named
              ? controls
                ? `connected as ${accountId}`
                : `connect ${heir.account_id}`
              : controls
                ? `your key for ${heir.account_id} is open; it signs the claim in this browser`
                : `the key made for ${heir.account_id} signs the claim, in this browser; no wallet needed`
          }
        />
        <Requirement met={shared.length > 0 && paid < shared.length} label="your share is unclaimed" detail={shared.length === 0 ? "nothing in the vault has a share for you yet" : paid === shared.length ? "every asset has been paid out" : paid ? `${paid} of ${shared.length} assets paid` : "nothing paid out yet"} />
        <Requirement
          met={balance !== undefined && shortBy === BigInt(0)}
          label="NEAR for the gas"
          detail={
            balance === undefined
              ? "reading the account…"
              : shortBy > BigInt(0)
                ? `the claim holds ${formatNear(cost.held)} NEAR up front (${cost.tgas} Tgas of gas and the records), almost all refunded; ${heir.account_id} has ${formatNear(balance)}. anyone can send it the rest.`
                : `${cost.tgas} Tgas for ${toPay.length || "its"} payout${toPay.length === 1 ? "" : "s"}: ${formatNear(cost.held)} NEAR held up front, almost all refunded`
          }
        />
        <Requirement met={false} label="your selfie check" detail="one World ID check binds your face to this slot: one human, one heir slot. the grantor's face can't claim." pending />
      </ul>

      <SelfieCheckModal
        open={claiming}
        purpose={{ kind: "claim", estateLabel: labelOf(id), heirLabel: heir.label }}
        // Sized when sent, from what is still unpaid by then.
        transaction={(signed) => calls.claim(id, heir.label, signed, cost)}
        signer={named ? null : signer}
        actionLabel="send the claim"
        successCopy="claimed. your share is on its way to your account; each payout lands in a few seconds."
        onClose={() => setClaiming(false)}
      />
    </section>
  );
}

/** How to put the claim's gas money on the heir's account: copy its address, or use the faucet. */
function FundGas({ account, shortBy, held }: { account: string; shortBy: bigint; held: bigint }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="mt-4 space-y-3 rounded-[8px] border-2 border-ink bg-cream p-4">
      <p className="text-sm">
        <strong>your account needs a little NEAR to send the claim.</strong> the claim holds {formatNear(held)} NEAR up front for gas and
        your records, and almost all of it comes back. send at least {formatNear(shortBy)} NEAR to it from any wallet
        {FAUCET_URL ? ", or get free testnet NEAR from the faucet" : ""}. this button turns on by itself once it lands.
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <span className="mono min-w-0 flex-1 break-all rounded-[8px] border-2 border-ink bg-surface px-3 py-1.5 text-sm">{account}</span>
        <button
          type="button"
          className="btn btn-sm btn-ghost"
          onClick={() => void navigator.clipboard.writeText(account).then(() => setCopied(true))}
        >
          {copied ? (
            <>
              <CheckIcon size={14} /> copied
            </>
          ) : (
            "copy address"
          )}
        </button>
        {FAUCET_URL ? (
          <a className="btn btn-sm btn-yellow" href={FAUCET_URL} target="_blank" rel="noreferrer">
            testnet faucet
          </a>
        ) : null}
      </div>
    </div>
  );
}

function Requirement({ met, label, detail, pending = false }: { met: boolean; label: string; detail: string; pending?: boolean }) {
  return (
    <li className="flex items-start gap-3">
      <IconCircle size="2xs" accent={met ? "bg-teal" : "bg-surface"}>{met ? <CheckIcon size={10} /> : pending ? null : <AlertIcon size={10} />}</IconCircle>
      <div className="text-sm">
        <p className="font-bold">{label}</p>
        <p className="text-xs text-muted">{detail}</p>
      </div>
    </li>
  );
}
