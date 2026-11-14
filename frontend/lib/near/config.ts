/**
 * Which NEAR network and contracts the app talks to. Public values only: they ship to the browser.
 *
 * NEXT_PUBLIC_NEAR_NETWORK   "testnet" (default) or "mainnet"
 * NEXT_PUBLIC_HERIT_FACTORY  the factory account, default herit-near.testnet
 */

export type Network = "testnet" | "mainnet";

export const NETWORK: Network =
  process.env.NEXT_PUBLIC_NEAR_NETWORK === "mainnet" ? "mainnet" : "testnet";

export const FACTORY: string = process.env.NEXT_PUBLIC_HERIT_FACTORY ?? "herit-near.testnet";

/** RPC endpoints, tried in order: FastNEAR first, NEAR's own as the fallback. */
export const RPC_URLS: readonly string[] =
  NETWORK === "mainnet"
    ? ["https://free.rpc.fastnear.com", "https://rpc.mainnet.near.org"]
    : ["https://test.rpc.fastnear.com", "https://rpc.testnet.near.org"];

/** The demo stablecoin (testnet only). */
export const MOCK_USDC = { id: "mock-usdc-kaykay.testnet", symbol: "mUSDC", decimals: 6 } as const;

export const EXPLORER = NETWORK === "mainnet" ? "https://nearblocks.io" : "https://testnet.nearblocks.io";

export const txUrl = (hash: string) => `${EXPLORER}/txns/${hash}`;
export const accountUrl = (id: string) => `${EXPLORER}/address/${id}`;

/** A testnet faucet (community-run) that sends free testnet NEAR to any account. None on mainnet. */
export const FAUCET_URL: string | null = NETWORK === "testnet" ? "https://near-faucet.io" : null;

/** Where the heir credential images are fetched from (the estate's NEP-171 `base_uri`). */
export const IPFS_GATEWAY = "https://gateway.pinata.cloud/ipfs";

/**
 * The image every heir's credential shows while it's `locked`: the same CID the estate contract
 * returns as `media` (`CREDENTIAL_IMAGES` in near/contracts/herit-estate/src/nft.rs), so the setup
 * wizard can show what an heir will get before the estate exists. `credential.test.ts` keeps the two equal.
 */
export const LOCKED_CREDENTIAL_CID = "bafkreibqatv5ibnvmkutd3mt3nf3fhwp5ricnatupszbum3yrwkg6pi2k4";
