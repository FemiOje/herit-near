"use client";

import Link from "next/link";
import { usePathname, useSearchParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";

import { StatusPill } from "@/components/estate/status-pill";
import { PlusIcon } from "@/components/ui/icons";
import { FACTORY } from "@/lib/near/config";
import { estate, factory } from "@/lib/near/contracts";
import { nsToMs } from "@/lib/near/format";
import { fromContract } from "@/lib/estate";
import { useWallet } from "@/lib/near/wallet";

/**
 * The connected grantor's estates, from the factory, and which one is chosen (`?estate=`, else the
 * first). Renders the picker and hands the chosen id to `children`.
 */
export function MyEstates({ children }: { children: (id: string) => React.ReactNode }) {
  const { accountId, loading, connect } = useWallet();

  if (loading) return <p className="text-sm text-muted">loading your wallet…</p>;

  if (!accountId) {
    return (
      <div className="card max-w-xl p-6">
        <h2 className="text-xl font-bold">connect a wallet to see your estates</h2>
        <p className="mt-2 text-sm text-muted">
          Herit reads the estates you opened from the factory at <span className="mono">{FACTORY}</span>.
        </p>
        <button type="button" className="btn mt-4" onClick={() => void connect()}>
          connect wallet
        </button>
      </div>
    );
  }

  return <EstateList grantor={accountId}>{children}</EstateList>;
}

function EstateList({ grantor, children }: { grantor: string; children: (id: string) => React.ReactNode }) {
  const estates = useQuery({ queryKey: ["estates_of", grantor], queryFn: () => factory.estatesOf(grantor) });
  const params = useSearchParams();
  const pathname = usePathname();

  if (estates.isPending) return <p className="text-sm text-muted">reading the factory…</p>;
  if (estates.isError) return <p className="text-sm text-coral">couldn&apos;t read the factory: {estates.error.message}</p>;
  if (estates.data.length === 0) {
    return (
      <div className="card max-w-xl p-6">
        <h2 className="text-xl font-bold">no estates yet</h2>
        <p className="mt-2 text-sm text-muted">
          <span className="mono">{grantor}</span> hasn&apos;t opened an estate through <span className="mono">{FACTORY}</span>.
        </p>
        <Link href="/setup" className="btn mt-4">
          <PlusIcon size={16} /> open an estate
        </Link>
      </div>
    );
  }

  const requested = params.get("estate");
  const selected = requested && estates.data.includes(requested) ? requested : estates.data[estates.data.length - 1];

  return (
    <>
      <ul className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {estates.data.map((id) => (
          <li key={id}>
            <EstateCard id={id} href={`${pathname}?estate=${encodeURIComponent(id)}`} selected={id === selected} />
          </li>
        ))}
        <li>
          <Link href="/setup" className="card-flat flex h-full min-h-24 items-center justify-center gap-2 border-dashed p-5 text-sm font-bold">
            <PlusIcon size={16} /> open another estate
          </Link>
        </li>
      </ul>
      {requested && !estates.data.includes(requested) ? (
        <p className="text-sm text-muted">
          <span className="mono">{requested}</span> isn&apos;t one of this wallet&apos;s estates; showing your latest.
        </p>
      ) : null}
      <div key={selected}>{children(selected)}</div>
    </>
  );
}

function EstateCard({ id, href, selected }: { id: string; href: string; selected: boolean }) {
  const status = useQuery({ queryKey: ["status_of_raw", id], queryFn: () => estate(id).statusOf() });
  const deadlines = useQuery({ queryKey: ["deadlines_of", id], queryFn: () => estate(id).deadlinesOf() });
  const unlockAt = deadlines.data && deadlines.data[1] !== "0" ? new Date(nsToMs(deadlines.data[1])) : null;

  return (
    <Link href={href} scroll={false} className={`card card-lift block p-5 ${selected ? "bg-yellow" : ""}`} aria-current={selected ? "true" : undefined}>
      <div className="flex items-start justify-between gap-3">
        <span className="mono break-all font-bold">{id}</span>
        {status.data ? <StatusPill status={fromContract(status.data)} size="sm" /> : null}
      </div>
      <p className="mt-3 text-sm text-muted">
        {deadlines.isPending
          ? "reading the clock…"
          : unlockAt
            ? `${status.data === "Unlocked" ? "unlocked" : "unlocks"} ${unlockAt.toLocaleString()}`
            : "the clock starts at the first check-in"}
      </p>
    </Link>
  );
}
