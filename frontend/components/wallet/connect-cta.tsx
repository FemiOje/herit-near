"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { type MouseEvent, useEffect, useState } from "react";

import { useWallet } from "@/lib/near/wallet";

/** A call to action that asks for a wallet first, then goes where it points. */
export function ConnectCta({ href, className, children }: { href: string; className?: string; children: React.ReactNode }) {
  const router = useRouter();
  const { accountId, connect, loading } = useWallet();
  const [armed, setArmed] = useState(false);

  useEffect(() => {
    if (armed && accountId) router.push(href);
  }, [armed, accountId, href, router]);

  const onClick = (event: MouseEvent<HTMLAnchorElement>) => {
    if (accountId || loading || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    setArmed(true);
    void connect().catch(() => setArmed(false));
  };

  return (
    <Link href={href} className={className} onClick={onClick}>
      {children}
    </Link>
  );
}
