"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { ConnectWallet } from "@/components/wallet/connect-wallet";
import { APP_LINKS, MARKETING_LINKS } from "@/lib/content/navigation";
import { Logo } from "./logo";
import { MobileNav, type ResolvedLink } from "./mobile-nav";

export function SiteNav({ variant = "marketing" }: { variant?: "marketing" | "app" }) {
  const pathname = usePathname();
  const app = variant === "app";
  const links: readonly ResolvedLink[] = (app ? APP_LINKS : MARKETING_LINKS).map((link) => ({
    href: link.href,
    label: link.label,
    active: pathname === link.href,
  }));

  return (
    <div className="sticky top-3 z-50 px-3 sm:top-4 sm:px-6">
      <nav className="mx-auto flex max-w-6xl items-center justify-between gap-2 rounded-[10px] border-2 border-ink bg-surface py-2.5 pl-3 pr-3.5 shadow-brut sm:gap-4 sm:py-3 sm:pl-6 sm:pr-6">
        <Logo />

        <div className="hidden items-center gap-1 md:flex">
          {links.map((link) => (
            <Link
              key={link.href}
              href={link.href}
              className={`nav-link ${link.active ? "nav-link-active" : ""}`}
              aria-current={link.active ? "page" : undefined}
            >
              {link.label}
            </Link>
          ))}
        </div>

        <div className="flex items-center gap-2">
          {app ? (
            <ConnectWallet />
          ) : (
            <>
              <Link href="/claim" className="btn btn-ghost btn-sm hidden sm:inline-flex">
                i am an heir
              </Link>
              <Link href="/setup" className="btn btn-sm whitespace-nowrap">
                open an estate
              </Link>
            </>
          )}
          <MobileNav links={links} className="md:hidden" />
        </div>
      </nav>
    </div>
  );
}
