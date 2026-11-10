/** Where the two navbar variants point, and what the footer lists. */

export type NavLink = { href: string; label: string };

export const MARKETING_LINKS: readonly NavLink[] = [
  { href: "/#how", label: "how it works" },
  { href: "/#estates", label: "live estates" },
  { href: "/dashboard", label: "app" },
];

export const APP_LINKS: readonly NavLink[] = [
  { href: "/dashboard", label: "dashboard" },
  { href: "/heirs", label: "heirs" },
  { href: "/claim", label: "claim" },
];

export const FOOTER_COLUMNS: readonly { title: string; links: readonly NavLink[] }[] = [
  {
    title: "product",
    links: [
      { href: "/setup", label: "open an estate" },
      { href: "/dashboard", label: "grantor dashboard" },
      { href: "/heirs", label: "manage heirs" },
      { href: "/claim", label: "heir claim" },
    ],
  },
  {
    title: "built on",
    links: [
      { href: "https://near.org", label: "NEAR" },
      { href: "https://github.com/hot-dao/near-connect", label: "HOT near-connect" },
      { href: "https://world.org/world-id", label: "World ID" },
      { href: "https://testnet.nearblocks.io/address/herit-near.testnet", label: "the factory on NearBlocks" },
      { href: "https://github.com/FemiOje/herit-near", label: "source" },
    ],
  },
];

export const FOOTER_BLURB =
  "next-of-kin inheritance gated by proof of life. estates are keyless NEAR accounts; heirs hold soulbound credentials.";

export const FOOTER_DISCLAIMER = "herit. hackathon build, NEAR testnet only. not financial or legal advice.";
