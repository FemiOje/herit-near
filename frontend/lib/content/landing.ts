import type { ComponentType } from "react";

import { ClockIcon, HeirIcon, type IconProps, PulseIcon, SelfieIcon, ShieldIcon, TreeIcon, UnlockIcon, VaultIcon } from "@/components/ui/icons";
import { FACTORY, MOCK_USDC } from "@/lib/near/config";

type Icon = ComponentType<IconProps>;

export const PROBLEMS: readonly { broken: string; brokenBody: string; fix: string; fixBody: string; icon: Icon; color: string }[] = [
  {
    broken: "proof of life is just a timestamp",
    brokenBody:
      "a classic dead-man's switch resets on any transaction. a bot on a cron, a delegated key, or a thief holding your stolen key can keep it alive forever while you are not.",
    fix: "a Selfie Check, not a signature",
    fixBody:
      "checking in takes a fresh World ID Selfie Check bound to your face at the first one. a private key can't look into a camera, so whoever holds yours can't pretend you're still here, or withdraw.",
    icon: SelfieIcon,
    color: "bg-teal",
  },
  {
    broken: "heirs are opaque addresses",
    brokenBody:
      "a hex string tells a grantor nothing, links to no person, and can be swapped silently. the heir has nothing to show for being named until the day it matters.",
    fix: "heirs hold a credential that wakes up",
    fixBody:
      "each heir is a real NEAR account (their own, or son.alice.herit-near.testnet made for them) holding a soulbound NEP-171 token. it reads locked, then grace, then claimable, then claimed: computed live from the estate.",
    icon: TreeIcon,
    color: "bg-purple",
  },
];

export const JOURNEY_STEPS: readonly { title: string; body: string; icon: Icon; color: string }[] = [
  {
    title: "open an estate",
    body: `the factory creates alice.${FACTORY}: a NEAR account with no keys at all, running immutable estate code published once as a Global Contract. nobody, Herit included, can sign for it.`,
    icon: TreeIcon,
    color: "bg-lavender",
  },
  {
    title: "name your heirs",
    body: "name a NEAR account the heir already has, or give them a new one under the estate from a key only they hold. each gets a share and a soulbound credential at once.",
    icon: HeirIcon,
    color: "bg-pink",
  },
  {
    title: "set the clock and fund the vault",
    body: `pick a check-in interval and a grace period (minutes for a demo, months for real), then deposit NEAR and tokens such as ${MOCK_USDC.symbol}. withdrawing needs a Selfie Check too.`,
    icon: VaultIcon,
    color: "bg-teal",
  },
  {
    title: "keep proving you're alive",
    body: "one Selfie Check per interval resets the clock. miss it and grace begins; miss grace and the estate unlocks. each heir then claims with their own Selfie Check.",
    icon: PulseIcon,
    color: "bg-yellow",
  },
];

export const CLOCK_STAGES: readonly { name: string; color: string; icon: Icon; lead: string; body: string; footer: string }[] = [
  {
    name: "active",
    color: "bg-teal",
    icon: ShieldIcon,
    lead: "you checked in on time",
    body: "the estate is sealed. heirs can see their credential and their share, and can do nothing with either. it reads locked.",
    footer: "credential: locked",
  },
  {
    name: "grace",
    color: "bg-yellow",
    icon: ClockIcon,
    lead: "a window closed with no check-in",
    body: "the safety net for travel, illness or a bad network day. one Selfie Check returns the estate to active with nothing lost.",
    footer: "credential: grace · reversible by you",
  },
  {
    name: "unlocked",
    color: "bg-coral",
    icon: UnlockIcon,
    lead: "grace ran out too",
    body: "the first claim (or anyone's poke) freezes every balance. each heir passes their own Selfie Check and is paid their share straight to their account.",
    footer: "credential: claimable → claimed",
  },
];

export const TRACKS: readonly { accent: string; badge: string; title: string; points: readonly string[]; icon: Icon }[] = [
  {
    accent: "bg-lavender",
    badge: "NFTs That Do Something",
    title: "a credential that carries an access right",
    points: [
      "every heir holds a soulbound NEP-171 token minted by the estate, visible in any NEAR wallet or explorer",
      "its state and IPFS image are computed live: locked, grace, claimable, claimed",
      "the estate is an account that owns money; the token says whether its holder may take their share",
      "heirs can be real sub-accounts of their estate, created by the estate itself",
    ],
    icon: ShieldIcon,
  },
  {
    accent: "bg-teal",
    badge: "Real-World Money",
    title: "inheritance a normal family could use",
    points: [
      "NEAR and NEP-141 stablecoins in one vault, split by per-asset shares",
      "payouts are async-safe: a failed transfer is credited back so the heir can claim again",
      "a thief with the grantor's key can't check in or withdraw: both need the grantor's face",
      "one human, one heir slot: a claim binds the heir's face to it",
    ],
    icon: VaultIcon,
  },
];

/** What's live on testnet, with the hashes a judge can check. */
export const ON_CHAIN: readonly { name: string; account: string; body: string; detail?: string }[] = [
  {
    name: "factory",
    account: FACTORY,
    body: "opens estates as sub-accounts, records them for this page, and points each at the published estate code.",
  },
  {
    name: "estate code",
    account: "herit-code-kaykay.testnet",
    body: "the estate contract, published once as an immutable Global Contract by hash. every estate runs it with no keys of its own.",
    detail: "65rC6sBJGosXHCGC6ityR3poJ68ecHDZsNRAPZQjkX2p",
  },
  {
    name: "demo stablecoin",
    account: MOCK_USDC.id,
    body: `${MOCK_USDC.symbol}, a standard NEP-141 token with ${MOCK_USDC.decimals} decimals, for depositing and inheriting tokens on testnet.`,
  },
];

export const HERO_TRUST_POINTS: readonly string[] = ["a stolen key can't pass a Selfie Check", "one human, one heir slot", "grace for false alarms"];
