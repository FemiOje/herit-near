"use client";

import { useQuery } from "@tanstack/react-query";
import { StatusPill } from "@/components/estate/status-pill";
import { accountUrl, FACTORY } from "@/lib/near/config";
import { estate, factory } from "@/lib/near/contracts";
import { nsToMs } from "@/lib/near/format";
import { fromContract } from "@/lib/estate";
import { useWallet } from "@/lib/near/wallet";

/** The connected grantor's estates, read from the factory, each with its live status. */
export function MyEstates() {
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

  return <EstateList grantor={accountId} />;
}

function EstateList({ grantor }: { grantor: string }) {
  const estates = useQuery({ queryKey: ["estates_of", grantor], queryFn: () => factory.estatesOf(grantor) });

  if (estates.isPending) return <p className="text-sm text-muted">reading the factory…</p>;
  if (estates.isError) return <p className="text-sm text-coral">couldn&apos;t read the factory: {estates.error.message}</p>;
  if (estates.data.length === 0) {
    return (
      <div className="card max-w-xl p-6">
        <h2 className="text-xl font-bold">no estates yet</h2>
        <p className="mt-2 text-sm text-muted">
          <span className="mono">{grantor}</span> hasn&apos;t opened an estate through{" "}
          <span className="mono">{FACTORY}</span>.
        </p>
      </div>
    );
  }

  return (
    <ul className="grid gap-4 sm:grid-cols-2">
      {estates.data.map((id) => (
        <li key={id}>
          <EstateCard id={id} />
        </li>
      ))}
    </ul>
  );
}

function EstateCard({ id }: { id: string }) {
  const status = useQuery({ queryKey: ["status_of", id], queryFn: () => estate(id).statusOf() });
  const deadlines = useQuery({ queryKey: ["deadlines_of", id], queryFn: () => estate(id).deadlinesOf() });

  const unlockAt = deadlines.data && deadlines.data[1] !== "0" ? new Date(nsToMs(deadlines.data[1])) : null;

  return (
    <article className="card p-5">
      <div className="flex items-start justify-between gap-3">
        <a href={accountUrl(id)} target="_blank" rel="noreferrer" className="mono break-all font-bold underline decoration-2 underline-offset-4">
          {id}
        </a>
        {status.data ? <StatusPill status={fromContract(status.data)} size="sm" /> : null}
      </div>
      <p className="mt-3 text-sm text-muted">
        {deadlines.isPending
          ? "reading the clock…"
          : unlockAt
            ? `${status.data === "Unlocked" ? "unlocked" : "unlocks"} ${unlockAt.toLocaleString()} unless the grantor checks in`
            : "the clock starts at the first check-in"}
      </p>
    </article>
  );
}
