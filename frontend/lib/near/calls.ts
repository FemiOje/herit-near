import type { Asset } from "./contracts.ts";
import { parseNear } from "./format.ts";

/**
 * Every transaction the grantor sends, as data: receiver, method, args, gas and deposit. The
 * screens hand these to `useWallet().call`; keeping them here keeps the amounts in one place and
 * lets the tests check them against the contract's rules.
 *
 * Deposits that pay for storage are generous on purpose: the estate refunds the surplus.
 */
export type Call = {
  receiverId: string;
  method: string;
  args: Record<string, unknown>;
  tgas: number;
  /** yoctoNEAR. */
  deposit: bigint;
};

const ONE_YOCTO = BigInt(1);
const NONE = BigInt(0);

/** The estate's attestation arguments, as `/api/attest` returns them. */
export type Signed = {
  attestation: {
    action: number;
    heir_label: string | null;
    commitment: string;
    nonce: string;
    expiry_ns: string;
  };
  signature: string;
};

/** The factory's floor: 0.1 NEAR for the estate's storage plus its 0.01 NEAR fee. */
export const MIN_OPEN_DEPOSIT = parseNear("0.11");
/** Covers a heir record's storage plus the 0.1 NEAR seed sent to the heir's new account. */
export const ADD_HEIR_DEPOSIT = parseNear("0.2");
/** A named heir's record: storage only, no account to create or seed. */
export const NAMED_HEIR_DEPOSIT = parseNear("0.01");
/** The token's listing plus the estate's 0.00125 NEAR registration with it. */
export const ADD_TOKEN_DEPOSIT = parseNear("0.05");
/** A new share override's storage. */
export const SET_SHARE_DEPOSIT = parseNear("0.01");
/** NEP-145 registration with a standard token. */
export const TOKEN_REGISTRATION = parseNear("0.00125");

export const calls = {
  openEstate: (factory: string, label: string, intervalSec: number, graceSec: number, deposit: bigint): Call => ({
    receiverId: factory,
    method: "open_estate",
    args: { label, check_in_interval_sec: intervalSec, grace_sec: graceSec },
    tgas: 100,
    deposit,
  }),

  checkIn: (estate: string, signed: Signed): Call => ({
    receiverId: estate,
    method: "check_in",
    args: signed,
    tgas: 30,
    deposit: NONE,
  }),

  pokeExpiry: (estate: string): Call => ({ receiverId: estate, method: "poke_expiry", args: {}, tgas: 30, deposit: NONE }),

  configure: (estate: string, intervalSec: number, graceSec: number): Call => ({
    receiverId: estate,
    method: "configure",
    args: { check_in_interval_sec: intervalSec, grace_sec: graceSec },
    tgas: 30,
    deposit: NONE,
  }),

  depositNear: (estate: string, yocto: bigint): Call => ({
    receiverId: estate,
    method: "deposit",
    args: {},
    tgas: 30,
    deposit: yocto,
  }),

  addToken: (estate: string, token: string): Call => ({
    receiverId: estate,
    method: "add_token",
    args: { token },
    tgas: 100,
    deposit: ADD_TOKEN_DEPOSIT,
  }),

  /** A token deposit goes through the token: `ft_transfer_call` lands in the estate's `ft_on_transfer`. */
  depositToken: (estate: string, token: string, units: bigint): Call => ({
    receiverId: token,
    method: "ft_transfer_call",
    args: { receiver_id: estate, amount: units.toString(), msg: "" },
    tgas: 100,
    deposit: ONE_YOCTO,
  }),

  /** Registers `accountId` with `token`, so a withdrawal of it can land. */
  registerWithToken: (token: string, accountId: string): Call => ({
    receiverId: token,
    method: "storage_deposit",
    args: { account_id: accountId, registration_only: true },
    tgas: 30,
    deposit: TOKEN_REGISTRATION,
  }),

  /** Exactly 1 yoctoNEAR, which the estate demands: only a full-access key can attach it. */
  withdraw: (estate: string, asset: Asset, amount: bigint, signed: Signed): Call => ({
    receiverId: estate,
    method: "withdraw",
    args: { asset, amount: amount.toString(), ...signed },
    tgas: 100,
    deposit: ONE_YOCTO,
  }),

  addHeir: (estate: string, label: string, publicKey: string, relationship: string, defaultBps: number): Call => ({
    receiverId: estate,
    method: "add_heir",
    args: { label, public_key: publicKey, relationship, default_bps: defaultBps },
    tgas: 100,
    deposit: ADD_HEIR_DEPOSIT,
  }),

  /** An account the heir already has. The estate refuses it on code published before named heirs. */
  addHeirAccount: (estate: string, label: string, accountId: string, relationship: string, defaultBps: number): Call => ({
    receiverId: estate,
    method: "add_heir_account",
    args: { label, account_id: accountId, relationship, default_bps: defaultBps },
    tgas: 30,
    deposit: NAMED_HEIR_DEPOSIT,
  }),

  /** The heir's claim; `cost` from `claimCost`, which sizes it to the payouts it will send. */
  claim: (estate: string, label: string, signed: Signed, cost: { tgas: number; deposit: bigint }): Call => ({
    receiverId: estate,
    method: "claim",
    args: { label, ...signed },
    tgas: cost.tgas,
    deposit: cost.deposit,
  }),

  removeHeir: (estate: string, label: string): Call => ({
    receiverId: estate,
    method: "remove_heir",
    args: { label },
    tgas: 30,
    deposit: NONE,
  }),

  setShare: (estate: string, label: string, asset: Asset, bps: number): Call => ({
    receiverId: estate,
    method: "set_share",
    args: { label, asset, bps },
    tgas: 30,
    deposit: SET_SHARE_DEPOSIT,
  }),
};

/** An ed25519 public key as `add_heir` takes it. */
export function isPublicKey(value: string): boolean {
  return /^ed25519:[1-9A-HJ-NP-Za-km-z]{43,44}$/.test(value.trim());
}

/**
 * Estate code published before `add_heir_account` (Checkpoint 10). Estates keep the code they were
 * opened with, so these can only take heirs by key. Every later code has named heirs.
 */
const CODE_WITHOUT_NAMED_HEIRS: readonly string[] = ["87NJpRzyMsaL4WWYspngjbHoLBKPJSjiyrFEpDKXNgE8"];

/**
 * Whether an estate running `globalContractHash` can name an existing account as an heir. Null is
 * an estate that doesn't exist yet: the factory opens new ones with its current code, which can.
 */
export function supportsNamedHeirs(globalContractHash: string | null | undefined): boolean {
  return !globalContractHash || !CODE_WITHOUT_NAMED_HEIRS.includes(globalContractHash);
}
