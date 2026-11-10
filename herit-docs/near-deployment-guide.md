# Herit on NEAR — track choice and deployment guide

> **Status:** plan + starter code. The contract code in §6 type-checks against near-sdk **5.30.1** for
> `wasm32-unknown-unknown`, and the §7 worker type-checks against OutLayer's WIT on a native target
> (both on 2026-11-09). **Nothing has been deployed or run end to end**, and the §6.5 checklist is still to
> write. Treat this as a typed starting point, not finished contracts.
>
> **Deadline:** submissions close **2026-11-14 23:59 UTC**. Plan to submit by **14 Nov 12:00 UTC**.

---

## 0. TL;DR

| Decision | Choice | Why |
| --- | --- | --- |
| **Primary track** | **#10 NFTs That Do Something** (`nfts`) | Its hints, *Dynamic NFTs · NEP-171 · Sub-accounts*, describe Herit's design almost word for word: an estate is an account that **owns money**, and each heir holds a token that **carries an access right** which wakes up when the grantor stops proving they are alive. |
| Also select | **#8 Real-World Money** (`realworld`), **#13 Open Innovation** (`open`) | Multi-select is allowed, and it is still one submission. Inheritance is money a normal family could use tomorrow. Supporting stablecoins (NEP-141 USDC) fits the realworld hints. |
| Skip | security, social, defi, agents, swarms, business, games, memecoins, thesis, privacy | These tracks are AI-agent-centric, require Sputnik DAO/Trezu, or don't fit Herit. Ticking them dilutes the story. |
| Chain path | **Native Rust port** (near-sdk + cargo-near), **not Aurora** | Only the **NEAR integration built in the window is judged** (rules Q-002/Q-004). An EVM redeploy on Aurora counts as "the old project as-is". It would also break: the ENSv2 frozen set exists only on Sepolia. |
| Network | **NEAR testnet** for contracts. A **mainnet** account only for the prize payout. | Testnet is accepted for every track (Q-010). Mainnet Global Contracts burn real NEAR. |
| Shape | **1 factory + 1 estate contract per estate**, with the estate code published once as a **Global Contract (by hash)** | NEAR's account model replaces ENS names and the five-contract immutable ring. |
| Naming | **`herit-near.testnet` → `alice.herit-near.testnet` → `son.alice.herit-near.testnet`** | The same three levels as the Sepolia ENS tree, except that every level is a real NEAR account. |
| Attestor | **OutLayer** TEE worker, with a local signer as fallback | Partner tech, and it removes the private key from the backend (§7). |
| Rule | **Partner tech first, where a feature needs it** | See §2.4. Custom code or a non-partner service only when no partner covers the need. |

---

## 1. Hackathon facts that constrain the build

Source: <https://nearhackathon.com> and <https://nearhackathon.com/HACKATHON.md>, read 2026-11-08.

- **Window:** 1 Nov – **14 Nov 2026 23:59 UTC**. Judging 16 Nov. Results by 18 Nov.
- **Prizes:** Grand prize $55k. Every one of the 13 tracks pays $10k / $3k / $2k for 1st/2nd/3rd, in NEAR, to the **mainnet** account you enter on the form.
- **Judging:** **Claude Code and Codex screen every repo first** (reproducibility, feature verification, preliminary score). Humans decide after that. The site points to past criteria: technical implementation, design, impact, originality, sustainability. A **machine-reproducible README** ("do X → see Y") is therefore a scoring surface in itself.
- **Prior work rules:**
  - Code from before the announcement (**2026-10-19**) is baseline and **is not judged** (Q-001).
  - Existing product on another chain: allowed. **Only the NEAR integration is judged.** You must **disclose** this in the README (Q-002).
  - Separate the baseline from the new work with tags **`pre-hackathon`** and **`hackathon-submission`**, plus a README link to the diff (Q-003).
  - List libraries, templates and AI-generated code in the README (Q-005).
- **Required for submission:** a public repo, a **LICENSE file**, a public **X post** on your own account (tag @NEARProtocol and #NEARHackathon, attach a video), the form at <https://nearhackathon.com/#apply>, and a NEAR mainnet payout account. **Submit once.** Make later edits through `/request-changes`, never by resubmitting.
- **Demo:** a hosted demo is a plus, and it should stay up until at least 18 Nov (ideally 25 Nov). If the demo is local-only, the X post must include a video.

### Gaps in this repo today

| Gap | Fix |
| --- | --- |
| **No `LICENSE` file**, although the README says MIT | Add `LICENSE` (MIT) at the repo root. |
| **No git tags** | Every commit up to `55115bf` is dated ≤ 2026-09-18, before the announcement. Tag that as the baseline: `git tag pre-hackathon 55115bf`. |
| The README describes ETHOnline/Sepolia only | Add a "Built at NEAR HACKATHON" section: what's new, the diff link, disclosure of the ETHOnline origin, tools used. |

The repo's `NEAR-HACKATHON.md` is an older copy of the rules. The live file adds the NEAR Business partner and the
request-changes process, so the live file wins where they differ.

---

## 2. Architecture: Herit (Sepolia) → Herit (NEAR)

### 2.1 Why NEAR fits Herit better than the original chain did

| Herit on Sepolia | Herit on NEAR | What gets better |
| --- | --- | --- |
| ENS hierarchy `herit.eth → alice.herit.eth → son.alice.herit.eth` in a separate registry | **Native account hierarchy**: `herit-near.testnet → alice.herit-near.testnet → son.alice.herit-near.testnet`. Each heir is a **sub-account of their estate**, created by the estate and controlled by the heir's own key. | Human-readable identity is built into the protocol, with no frozen-registry dependency. The Sepolia problem "names invisible in the ENS app" disappears. |
| `ROLE_HEIR_CLAIM` bit granted on an ENSv2 subname | **Soulbound dynamic NEP-171 heir credential** minted by the estate. Its metadata is computed live: `locked → claimable → claimed`. | Every NEAR wallet and explorer renders the credential, which is the core story for the NFT track. |
| Five contracts holding each other as `immutable`s, so rotating the attestor key means redeploying all five | **One estate contract per estate account**, deployed **keyless** (no access keys), from immutable Global Contract code | Immutability comes from NEAR's account model. There is no cross-contract trust ring, and estates are isolated from each other and sharded. |
| `HeritVault` with ETH + ERC20 | The estate account escrows **NEAR + NEP-141** tokens (e.g. USDC) | Stablecoins support the Real-World Money track. |
| EIP-712 + `ecrecover` attestation signed by a key in the backend's environment | **ed25519 attestation** over a Borsh payload, signed by an **OutLayer TEE worker** and checked with `env::ed25519_verify` | Same narrow-signer model and replay rules, but no private key on your server. |
| `pokeExpiry()` is permissionless | `poke_expiry()` is permissionless | Unchanged. |
| The Graph subgraph | Factory `estates()` view for the estate list, plus NEP-297 `EVENT_JSON` logs read through the FastNEAR Transactions API for the feed | No indexer to deploy or keep in sync. No hackathon partner offers indexing (§2.4). |

**Aurora was considered and rejected.** It would let `forge script` run nearly as-is, but the ENS layer would not
exist there, World ID's router is not there, and the judges would see an EVM redeploy, which is the one thing
the rules say is not judged.

### 2.2 Contracts

```
herit-near.testnet                     ← herit-factory  (regular deploy; upgradable during the hackathon)
│   open_estate("alice", ...)  ── creates ──►  alice.herit-near.testnet
│                                              │  uses Global Contract <estate code hash>
│                                              │  herit-estate: state machine + vault + attestor + NEP-171
│                                              │
│                                              │  add_heir("son", <heir public key>, ...)  ── creates ──►
│                                              └─ son.alice.herit-near.testnet
│                                                 the heir's own account; only the heir holds a key to it
herit-code-xyz.testnet            ← publisher of the Global Contract (by hash ⇒ immutable code)
mock-usdc-xyz.testnet             ← NEP-141 test token for the demo (testnet)
```

**Each level has to create the next.** NEAR only lets an account create its own direct sub-accounts, so the
factory creates `alice.herit-near.testnet`, and `son.alice.herit-near.testnet` can only come from the estate contract at
`alice.herit-near.testnet`. A keyless estate can still do this, because the contract issues the create-account action
itself. The grantor signs in from their normal wallet (`alice-xyz.testnet` in this guide), not from the estate.

**`herit-factory`**
- `new(estate_code_hash, attestor_pk)` sets up the factory.
- `open_estate(label, check_in_interval_sec, grace_sec)` is `#[payable]`. It creates the sub-account, funds its storage, attaches the global code, initialises it with `grantor = predecessor`, and indexes it. A failure callback refunds the deposit.
- `set_attestor(attestor_pk)` is signed by the factory's own key and only affects estates opened afterwards.
- Views: `estates_of(grantor)`, `estates(from_index, limit)`, `estates_count()`, `config()`. No heir lookup is needed: an heir's estate is their account id minus its first segment.

**`herit-estate`** (one per estate; replaces `HeritRegistry` + `HeritVault` + `ClaimManager` + `LivenessAttestor` + `AccessControlGate`)
- Grantor-only, before unlock:
  - `add_heir(label, public_key, relationship, default_bps)` (payable; creates `<label>.<estate>` with the heir's key, seeds it with NEAR for gas, and mints the credential to it)
  - `configure(check_in_interval_sec, grace_sec)` (Active only)
  - `set_share(label, asset, bps)`, `add_token(token)` (payable; registers estate storage on the FT)
  - `deposit()` (payable NEAR), `ft_on_transfer` (NEP-141 deposits), `withdraw(asset, amount)`
- `check_in(attestation, signature)`: only the grantor, only a matching face commitment.
- `poke_expiry()`: permissionless. On the `Unlocked` transition it snapshots the escrow.
- `claim(label, attestation, signature)`: signed by the heir's sub-account, plus the heir's own Selfie Check. It pays each asset once, with **rollback callbacks**.
- NEP-171 views: `nft_metadata`, `nft_token`, `nft_tokens`, `nft_tokens_for_owner`, `nft_supply_for_owner`, `nft_total_supply`. `nft_transfer*` panics because the credential is soulbound.
- Views: `status_of`, `deadlines_of`, `heirs_of`, `share_of`, `claimable_of`, `claimable_all`, `has_claimed`, `tokens_of`, `escrow_of`, `snapshot_of`, `snapshot_taken`, `nonce_used`, `commitment_of`, `claim_commitment_used`.

**Where each Solidity function went.** `HeritRegistry`, `HeritVault`, `ClaimManager` and `LivenessAttestor` all merge
into `herit-estate`, and `estateId` disappears because each estate is its own contract. The guards that only let the
five contracts trust each other (`_onlyGate`, `_onlyAttestor`, `_onlyHeritRegistry`, `_onlyClaimManager`) go with it.
From `AccessControlGate`, `openEstate` and `estatesOfGrantor` move to the factory and `registerHeir` becomes `add_heir`.
`unlockHeir`, `writeShareRecord`, `renewEstate`, `predictEstateRegistry` and `estateRegistryOf` have no equivalent:
unlock is computed from `pending_status`, shares live in contract state, NEAR accounts don't expire, and an account's
name is its address. `getMessageHash` moves off-chain into the attestor, and `isAvailable` becomes a frontend
`view_account` on `<label>.herit-near.testnet`.

### 2.3 Work for the hackathon window (what the judges see as new)

1. NEAR-native estate factory with keyless, immutable estate accounts built from Global Contract code, and heirs as real sub-accounts of their estate.
2. Dynamic, soulbound NEP-171 heir credentials whose state tracks the estate's liveness.
3. NEP-141 stablecoin inheritance with async-safe payouts.
4. World ID Selfie Check attestor rebuilt as an OutLayer TEE worker, so no server holds the signing key.
5. *(Stretch, strongly recommended: a small change with a large story)* **Liveness-gated withdrawals.** On Sepolia, a thief holding the grantor's key could still call `withdraw`. On NEAR, `withdraw` also requires a fresh check-in attestation, so a stolen key alone can move nothing. The original README's threat model ("the thief who stole her key") then holds end to end.
6. *(Stretch checkpoint, agreed 2026-11-10; starts only once the core estate works on testnet)* **BTC, ETH and SOL inheritance through NEAR Intents.** The grantor deposits BTC, ETH or SOL into `intents.near`, credited to the estate account. The estate holds these as NEP-245 multi-token balances. On unlock, each heir's claim moves their share to them inside Intents, in the same split as NEAR and NEP-141, and the heir withdraws the real coin to any address on its native chain.
   - **Coins must be deposited into the estate first.** Herit can't reach coins that stay in the grantor's own Bitcoin, Ethereum or Solana wallet.
   - **Intents is mainnet only** (brief Q-007), so this part of the demo runs on mainnet with tiny real amounts. Everything else stays on testnet.
   - **Not Chain Signatures.** Building and relaying raw Bitcoin, EVM and Solana transactions is too much for the window.
   - **Verify before writing its checkpoint guide:** a contract can hold an Intents balance and transfer it by calling `intents.near` directly, without a signed intent.

### 2.4 Partner tech

**Rule for this build: use a partner's tech where a feature needs it, before building it or reaching for a
non-partner service.** The list is the Ecosystem table in <https://nearhackathon.com/HACKATHON.md>. No integration is
mandatory (Q-011), so a partner only goes in if it does a job Herit actually has.

| Need | Partner | Decision |
| --- | --- | --- |
| Wallet connection | **HOT** (`@hot-labs/near-connect`) | **Use** (§9) |
| Attestor signing and verification | **OutLayer** | **Use** (§7), with a local-signer fallback |
| Cross-chain inheritance (BTC, ETH, SOL) | **NEAR Intents** | **Use** in the §2.3 stretch checkpoint (mainnet only) |
| Guardian committee, multisig admin | **Trezu**, **SputnikDAO** | After the hackathon (`ARCHITECTURE.md` §10) |
| Treasury | **NEAR Business** | No: the vault has to be gated by contract rules, not by signers' approval |
| Running Herit on EVM | **Aurora** | No: an EVM redeploy isn't judged (§2.1) |
| Activity feed, estate list | *No partner offers this* | FastNEAR (non-partner) and a factory view (§9) |

Rhea, meme.cooking, HEX, GoMining, Agent Market, BRRRAIN, NEAR.AI and SVRN do jobs Herit doesn't have. NEAR Builders
is a community and Gulf Labs an investor.

---

## 3. Toolchain (pinned versions, checked 2026-11-08)

| Tool | Version | Install |
| --- | --- | --- |
| Rust | ≥ 1.95 (the near-sdk examples use edition 2024) | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| wasm targets | — | `rustup target add wasm32-unknown-unknown wasm32-wasip2` (the second is for the OutLayer worker) |
| near-cli-rs | **0.31.1** | `curl --proto '=https' --tlsv1.2 -LsSf https://github.com/near/near-cli-rs/releases/latest/download/near-cli-rs-installer.sh \| sh` |
| cargo-near | **0.23.0** | `curl --proto '=https' --tlsv1.2 -LsSf https://github.com/near/cargo-near/releases/latest/download/cargo-near-installer.sh \| sh` |
| Docker | any recent | Required only for `cargo near build reproducible-wasm` |
| near-sdk / near-contract-standards | **5.30.1** | Cargo |
| near-workspaces | **0.24.0** | Cargo (dev-dependency) |
| near-api-js | **7.4.0** | npm (frontend + backend signer) |
| @hot-labs/near-connect | **0.12.0** | npm (wallet connection; this replaces wagmi) |
| borsh (JS) | **2.0.0** | npm (backend attestation payload) |

```bash
near --version && cargo near --version && rustc --version
```

Optional: `npx skills add near/agent-skills` adds NEAR's own agent skills. `near-smart-contracts`, `near-dapp`,
`near-api-js` and `near-contract-audit` are the relevant ones here.

---

## 4. Accounts

Testnet named accounts are first-come, so **claim `herit-near.testnet` first**. It was unclaimed on 2026-11-09. If
someone takes it, every name below it changes. The helper accounts aren't part of the hierarchy, so pick a team
suffix for them once, e.g. `xyz`.

```bash
export NET=testnet
export FACTORY=herit-near.testnet              # factory contract; estates become <label>.herit-near.testnet
export CODE=herit-code-xyz.testnet        # publishes the global estate code
export TOKEN=mock-usdc-xyz.testnet        # NEP-141 test token
export GRANTOR=alice-xyz.testnet          # demo grantor's wallet (the estate itself is alice.herit-near.testnet)
export FUNDER1=herit-funder1-xyz.testnet  # spare faucet accounts for topping up $CODE
export FUNDER2=herit-funder2-xyz.testnet

# Heir accounts. Do NOT create these here: add_heir creates them (§8.4).
export HEIR1=son.alice.$FACTORY           # son.alice.herit-near.testnet
export HEIR2=daughter.alice.$FACTORY      # daughter.alice.herit-near.testnet

for A in $FACTORY $CODE $TOKEN $GRANTOR $FUNDER1 $FUNDER2; do
  near account create-account sponsor-by-faucet-service "$A" \
    autogenerate-new-keypair save-to-keychain network-config $NET create
done

near account view-account-summary $CODE network-config $NET now
```

**Budget the Global Contract.** Publishing a Global Contract **burns 10 NEAR per 100 KB** of wasm. That is 10× the normal storage
rate, and the NEAR is not recoverable. An estate contract of about 200 KB therefore burns about **20 NEAR**, which is more than one faucet
drop. Build first (§6.1), read the wasm size, then top up `$CODE` from the spare faucet accounts:

```bash
near tokens $FUNDER1 send-near $CODE '8 NEAR' network-config $NET sign-with-keychain send
near tokens $FUNDER2 send-near $CODE '8 NEAR' network-config $NET sign-with-keychain send
```

If the budget doesn't work, use the fallback in §6.4: a regular per-estate deploy, which locks 1 NEAR/100 KB per estate and is refundable.

**Mainnet payout account** (needed for the form, not for the build): create one in a wallet such as HOT, Meteor or Intear. Write it down and never
reuse it as a deployer.

---

## 5. Workspace layout

The Foundry project stays on `main` as the baseline and isn't on `kaykay`. Add a Cargo workspace:

```
near/
├── Cargo.toml                 # [workspace]
├── contracts/
│   ├── herit-estate/          # Cargo.toml, src/lib.rs
│   ├── herit-factory/
│   └── mock-ft/               # copy of near-sdk-rs/examples/fungible-token, renamed
├── attestor-worker/           # OutLayer TEE worker (§7); separate from the workspace
├── tests/                     # its own package: near-workspaces sandbox and fork tests
│   ├── Cargo.toml
│   └── lifecycle.rs
└── scripts/
    └── code-hash.py           # base58(sha256(wasm)) for use_global_contract

frontend/scripts/sign-attestation.mjs   # DEV ONLY: mints test attestations with a local key (§7.5)
```

`near/Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["contracts/herit-estate", "contracts/herit-factory", "contracts/mock-ft", "tests"]
exclude = ["attestor-worker"]   # different target (wasm32-wasip2)

[workspace.dependencies]
near-sdk = "5.30.1"
near-contract-standards = "5.30.1"

[profile.release]
codegen-units = 1
opt-level = "z"        # size matters: Global Contract cost is per byte
lto = true
debug = false
panic = "abort"
overflow-checks = true # never ship without this
```

`near/contracts/herit-estate/Cargo.toml`:

```toml
[package]
name = "herit-estate"
version = "0.1.0"
edition = "2024"
rust-version = "1.95"
repository = "https://github.com/FemiOje/herit-near"   # cargo-near embeds this in contract metadata

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
near-sdk = { workspace = true }
near-contract-standards = { workspace = true }

[dev-dependencies]
near-sdk = { workspace = true, features = ["unit-testing"] }
proptest = "1"

[package.metadata.near.reproducible_build]
# Copy the current image + digest from near-sdk-rs/examples/*/Cargo.toml at tag near-sdk-v5.30.1
# (or run `cargo near new` once and copy its block). Pin the digest; the tag is descriptive only.
image = "sourcescan/cargo-near:<version>-rust-<version>"
image_digest = "sha256:<digest>"
passed_env = []
container_build_command = ["cargo", "near", "build", "non-reproducible-wasm", "--locked"]
```

`herit-factory/Cargo.toml` is the same without `near-contract-standards`.

`near/tests/Cargo.toml` is a separate package with `publish = false`, `autotests = false`, one `[[test]]` entry per
test file, and `near-workspaces = "0.24.0"`, `tokio`, `anyhow` and `serde_json` as dev-dependencies. A workspace root
can't own tests, so without this package nothing in `near/tests/` compiles.

---

## 6. Contracts

### 6.1 `herit-estate` — core (starter code)

```rust
// near/contracts/herit-estate/src/lib.rs
use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::json_types::{Base64VecU8, U128, U64};
use near_sdk::store::{IterableMap, LookupMap, LookupSet};
use near_sdk::{
    env, ext_contract, is_promise_success, near, require, AccountId, BorshStorageKey, Gas,
    NearToken, PanicOnDefault, Promise, PromiseOrValue, PublicKey,
};

mod nft; // NEP-171 views, §6.2

pub const ACTION_CHECKIN: u8 = 0;
pub const ACTION_CLAIM: u8 = 1;
pub const ACTION_WITHDRAW: u8 = 2; // §2.3 item 5
const DOMAIN: &str = "herit-near:v1";
const MAX_ATTESTATION_LIFETIME_NS: u64 = 30 * 60 * 1_000_000_000; // same 30-min cap as Sepolia
const BPS: u128 = 10_000;
const MAX_HEIRS: u32 = 10;
const MAX_TOKENS: usize = 5; // keeps claim() under the 300 Tgas tx ceiling (§10)
const HEIR_ACCOUNT_SEED: NearToken = NearToken::from_millinear(100); // account storage + the heir's claim gas
// Same bounds as HeritRegistry.
const MIN_CHECK_IN_INTERVAL_SEC: u32 = 120;
const MAX_CHECK_IN_INTERVAL_SEC: u32 = 90 * 86_400;
const MIN_GRACE_SEC: u32 = 60;
const MAX_GRACE_SEC: u32 = 14 * 86_400;

/// `None` = native NEAR, `Some(token)` = a NEP-141 contract.
pub type Asset = Option<AccountId>;

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Status { Active, Grace, Unlocked }

#[near(serializers = [borsh, json])]
#[derive(Clone)]
pub struct Heir { pub account_id: AccountId, pub relationship: String, pub default_bps: u16 }

/// What the World ID backend signs. `estate_id` and `subject` are NOT passed in: the contract fills
/// them from `current_account_id` / `predecessor_account_id`, so a signature only verifies for this
/// estate and this caller.
#[near(serializers = [borsh])]
struct SignedPayload {
    domain: String,
    estate_id: AccountId,
    subject: AccountId,
    action: u8,
    heir_label: Option<String>,
    commitment: [u8; 32],
    nonce: u64,
    expiry_ns: u64,
}

#[near(serializers = [json])]
pub struct Attestation {
    pub action: u8,
    pub heir_label: Option<String>,
    pub commitment: Base64VecU8, // 32 bytes: keccak/sha256(nullifier, salt), derived as today
    pub nonce: U64,
    pub expiry_ns: U64,
}

#[derive(BorshStorageKey)]
#[near]
enum Key { Heirs, Shares, Escrow, Snapshot, Paid, Nonces, ClaimCommitments }

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Estate {
    factory: AccountId,
    grantor: AccountId,
    attestor_pk: [u8; 32],
    grantor_commitment: Option<[u8; 32]>,
    check_in_interval_ns: u64,
    grace_ns: u64,
    last_check_in_ns: u64, // 0 = no check-in yet, so the clock has not started
    status: Status, // cache only; guards read pending_status() (same rule as _pendingStatus on Sepolia)
    heirs: IterableMap<String, Heir>,
    shares: LookupMap<(String, Asset), u16>, // per-asset override of default_bps
    tokens: Vec<AccountId>,                  // whitelisted NEP-141s, <= MAX_TOKENS
    escrow: IterableMap<Asset, u128>,        // explicit accounting; NEVER use env::account_balance()
    snapshot: LookupMap<Asset, u128>,
    paid: LookupSet<(String, Asset)>,
    used_nonces: LookupSet<u64>,
    claim_commitments: LookupMap<[u8; 32], String>, // commitment -> label it claimed for
}

#[ext_contract(ext_ft)]
trait FungibleToken {
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
    fn storage_deposit(&mut self, account_id: Option<AccountId>, registration_only: Option<bool>);
}

#[near]
impl Estate {
    /// Called once by the factory in the same batch that creates this account.
    #[init]
    pub fn new(grantor: AccountId, attestor_pk: String, check_in_interval_sec: u32, grace_sec: u32) -> Self {
        validate_timers(check_in_interval_sec, grace_sec);
        log_event("estate_opened", &format!(r#"{{"grantor":"{grantor}"}}"#));
        Self {
            factory: env::predecessor_account_id(),
            grantor,
            attestor_pk: decode_ed25519(&attestor_pk),
            grantor_commitment: None,
            check_in_interval_ns: check_in_interval_sec as u64 * 1_000_000_000,
            grace_ns: grace_sec as u64 * 1_000_000_000,
            last_check_in_ns: 0, // the first check_in starts the clock, as on Sepolia
            status: Status::Active,
            heirs: IterableMap::new(Key::Heirs),
            shares: LookupMap::new(Key::Shares),
            tokens: vec![],
            escrow: IterableMap::new(Key::Escrow),
            snapshot: LookupMap::new(Key::Snapshot),
            paid: LookupSet::new(Key::Paid),
            used_nonces: LookupSet::new(Key::Nonces),
            claim_commitments: LookupMap::new(Key::ClaimCommitments),
        }
    }

    // ---------- liveness ----------

    pub fn check_in(&mut self, attestation: Attestation, signature: Base64VecU8) {
        self.only_grantor();
        require!(self.pending_status() != Status::Unlocked, "estate unlocked");
        let c = self.verify(&attestation, &signature, ACTION_CHECKIN);
        match self.grantor_commitment {
            None => self.grantor_commitment = Some(c),               // bind to the first face seen
            Some(bound) => require!(bound == c, "wrong human"),      // = LivenessAttestor__WrongHuman
        }
        self.last_check_in_ns = env::block_timestamp();
        self.status = Status::Active;
        log_event("checked_in", &format!(r#"{{"at":"{}"}}"#, self.last_check_in_ns));
    }

    /// Permissionless, like pokeExpiry(). Idempotent.
    pub fn poke_expiry(&mut self) -> Status {
        let s = self.pending_status();
        if s == self.status { return s; }
        match s {
            Status::Grace => log_event("entered_grace", &format!(r#"{{"at":"{}"}}"#, env::block_timestamp())),
            Status::Unlocked => {
                for (asset, bal) in self.escrow.iter() {
                    self.snapshot.insert(asset.clone(), *bal); // freeze at unlock, not at first claim
                }
                log_event("unlocked", &format!(r#"{{"heirs":{}}}"#, self.heirs.len()));
            }
            Status::Active => {}
        }
        self.status = s;
        s
    }

    // ---------- grantor setup (before unlock) ----------

    /// Retunes the timers. Active only: from Grace, a longer interval would recompute back to
    /// Active, which is recovery without a Selfie Check. Never moves `last_check_in_ns`.
    pub fn configure(&mut self, check_in_interval_sec: u32, grace_sec: u32) {
        self.only_grantor();
        require!(self.pending_status() == Status::Active, "estate not active");
        validate_timers(check_in_interval_sec, grace_sec);
        self.check_in_interval_ns = check_in_interval_sec as u64 * 1_000_000_000;
        self.grace_ns = grace_sec as u64 * 1_000_000_000;
        // A shorter interval moves the deadline back, maybe into the past. Never out of Active.
        require!(self.pending_status() == Status::Active, "timers would end Active");
        log_event("configured", &format!(r#"{{"interval_sec":{check_in_interval_sec},"grace_sec":{grace_sec}}}"#));
    }

    /// Creates `<label>.<this estate>` (son.alice.herit-near.testnet) as a real account the heir controls.
    /// The heir generates `public_key` themselves, so the grantor never holds the private half.
    #[payable] // attached NEAR pays the credential's storage + HEIR_ACCOUNT_SEED; the surplus is refunded
    pub fn add_heir(&mut self, label: String, public_key: PublicKey, relationship: String, default_bps: u16) -> Promise {
        self.only_grantor();
        self.not_unlocked();
        require!(!label.contains('.'), "label must be a single segment");
        require!(self.heirs.len() < MAX_HEIRS, "too many heirs");
        require!(!self.heirs.contains_key(&label), "label taken");
        let account_id: AccountId = format!("{label}.{}", env::current_account_id()).parse().expect("invalid label");
        let before = env::storage_usage();
        self.heirs.insert(label.clone(), Heir { account_id: account_id.clone(), relationship, default_bps });
        self.heirs.flush(); // make storage_usage() see the write
        require!(self.total_bps(&None) <= BPS, "shares exceed 100%");
        let seed = HEIR_ACCOUNT_SEED.as_yoctonear();
        let storage_cost = refund_storage_surplus(before, seed);
        Promise::new(account_id)
            .create_account()
            .transfer(HEIR_ACCOUNT_SEED)
            .add_full_access_key(public_key)
            .then(
                Self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(10))
                    .on_heir_created(label, U128(storage_cost + seed)),
            )
    }

    /// Create-account fails if the name already exists. The seed comes back to this estate, so undo
    /// the heir record and forward the money to the grantor. The credential is only minted on success.
    #[private]
    pub fn on_heir_created(&mut self, label: String, refund: U128) -> bool {
        if is_promise_success() {
            let owner = self.heirs.get(&label).expect("no such heir").account_id.clone();
            nft::emit_mint(&owner, &label);
            log_event("heir_added", &format!(r#"{{"label":"{label}","account":"{owner}"}}"#));
            true
        } else {
            self.heirs.remove(&label);
            Promise::new(self.grantor.clone()).transfer(NearToken::from_yoctonear(refund.0)).detach();
            false
        }
    }

    #[payable]
    pub fn deposit(&mut self) {
        self.only_grantor();
        self.not_unlocked();
        let amt = env::attached_deposit().as_yoctonear();
        require!(amt > 0, "zero amount");
        *self.escrow.entry(None).or_insert(0) += amt;
        log_event("deposited", &format!(r#"{{"asset":"near","amount":"{amt}"}}"#));
    }

    // add_token / set_share / withdraw follow the same pattern (see §6.5 checklist)

    // ---------- heir payout ----------

    pub fn claim(&mut self, label: String, attestation: Attestation, signature: Base64VecU8) -> Promise {
        let heir_id = env::predecessor_account_id();
        require!(attestation.heir_label.as_deref() == Some(label.as_str()), "label mismatch");
        let c = self.verify(&attestation, &signature, ACTION_CLAIM);
        require!(self.poke_expiry() == Status::Unlocked, "not unlocked"); // pokes for the heir, like ClaimManager
        let owner = self.heirs.get(&label).map(|h| h.account_id.clone()).expect("no such heir");
        require!(owner == heir_id, "not entitled");
        require!(Some(c) != self.grantor_commitment, "grantor cannot be heir");
        // One human, one heir slot. Re-use is allowed only for the SAME label, so a failed payout can be retried.
        if let Some(prev) = self.claim_commitments.get(&c) {
            require!(prev == &label, "commitment already used");
        }
        self.claim_commitments.insert(c, label.clone());

        let mut assets: Vec<Asset> = vec![None];
        assets.extend(self.tokens.iter().cloned().map(Some));
        let mut out: Option<Promise> = None;
        for asset in assets {
            let key = (label.clone(), asset.clone());
            if self.paid.contains(&key) { continue; }
            let snap = self.snapshot.get(&asset).copied().unwrap_or(0);
            let amount = snap * self.share_of(&label, &asset) as u128 / BPS; // rounds down, like payOut
            if amount == 0 { continue; }
            self.paid.insert(key); // effect before interaction
            *self.escrow.get_mut(&asset).expect("no escrow") -= amount; // on_payout re-credits on failure
            let pay = match &asset {
                None => Promise::new(heir_id.clone()).transfer(NearToken::from_yoctonear(amount)),
                Some(t) => ext_ft::ext(t.clone())
                    .with_attached_deposit(NearToken::from_yoctonear(1))
                    .with_static_gas(Gas::from_tgas(15))
                    .ft_transfer(heir_id.clone(), U128(amount), Some("herit claim".into())),
            };
            let p = pay.then(
                Self::ext(env::current_account_id())
                    .with_static_gas(Gas::from_tgas(10))
                    .on_payout(label.clone(), asset, U128(amount)),
            );
            out = Some(match out { None => p, Some(acc) => acc.and(p) });
        }
        out.expect("nothing to claim")
    }

    /// NEAR has no cross-receipt revert: if the transfer fails, undo the `paid` mark and the debit here.
    #[private]
    pub fn on_payout(&mut self, label: String, asset: Asset, amount: U128) -> bool {
        let asset_name = asset.as_ref().map_or("near".to_string(), |t| t.to_string());
        if is_promise_success() {
            log_event("paid", &format!(r#"{{"label":"{label}","asset":"{asset_name}","amount":"{}"}}"#, amount.0));
            true
        } else {
            self.paid.remove(&(label.clone(), asset.clone()));
            *self.escrow.entry(asset).or_insert(0) += amount.0;
            log_event("payout_failed", &format!(r#"{{"label":"{label}","asset":"{asset_name}"}}"#));
            false
        }
    }

    // ---------- views ----------

    pub fn status_of(&self) -> Status { self.pending_status() }
    /// `(0, 0)` until the first check-in.
    pub fn deadlines_of(&self) -> (U64, U64) {
        if self.last_check_in_ns == 0 { return (U64(0), U64(0)); }
        let g = self.last_check_in_ns + self.check_in_interval_ns;
        (U64(g), U64(g + self.grace_ns))
    }
    pub fn tokens_of(&self) -> Vec<AccountId> { self.tokens.clone() }
    pub fn snapshot_taken(&self) -> bool { self.status == Status::Unlocked } // set in the same call
    pub fn has_claimed(&self, label: String, asset: Asset) -> bool { self.paid.contains(&(label, asset)) }
    pub fn nonce_used(&self, nonce: U64) -> bool { self.used_nonces.contains(&nonce.0) }
    pub fn commitment_of(&self) -> Option<Base64VecU8> {
        self.grantor_commitment.map(|c| Base64VecU8(c.to_vec()))
    }
    pub fn claim_commitment_used(&self, commitment: Base64VecU8) -> bool {
        let c: [u8; 32] = commitment.0.as_slice().try_into().expect("commitment != 32 bytes");
        self.claim_commitments.contains_key(&c)
    }
    pub fn claimable_all(&self, label: String) -> Vec<(Asset, U128)> {
        let mut assets: Vec<Asset> = vec![None];
        assets.extend(self.tokens.iter().cloned().map(Some));
        assets.into_iter().map(|a| { let v = self.claimable_of(label.clone(), a.clone()); (a, v) }).collect()
    }
    pub fn claimable_of(&self, label: String, asset: Asset) -> U128 {
        if self.paid.contains(&(label.clone(), asset.clone())) { return U128(0); }
        let base = if self.status == Status::Unlocked {
            self.snapshot.get(&asset).copied().unwrap_or(0)
        } else {
            self.escrow.get(&asset).copied().unwrap_or(0)
        };
        U128(base * self.share_of(&label, &asset) as u128 / BPS)
    }

    // ---------- internals ----------

    fn pending_status(&self) -> Status {
        if self.status == Status::Unlocked { return Status::Unlocked; }
        if self.last_check_in_ns == 0 { return Status::Active; } // not started: must never read as lapsed
        let now = env::block_timestamp();
        let grace_at = self.last_check_in_ns + self.check_in_interval_ns;
        if now >= grace_at + self.grace_ns { Status::Unlocked }
        else if now >= grace_at { Status::Grace }
        else { Status::Active }
    }

    fn verify(&mut self, a: &Attestation, sig: &Base64VecU8, action: u8) -> [u8; 32] {
        require!(a.action == action, "wrong action");
        let now = env::block_timestamp();
        require!(a.expiry_ns.0 > now, "attestation expired");
        require!(a.expiry_ns.0 <= now + MAX_ATTESTATION_LIFETIME_NS, "attestation lives too long");
        require!(!self.used_nonces.contains(&a.nonce.0), "nonce used");
        let commitment: [u8; 32] = a.commitment.0.as_slice().try_into().expect("commitment != 32 bytes");
        require!(commitment != [0u8; 32], "zero commitment");
        let msg = near_sdk::borsh::to_vec(&SignedPayload {
            domain: DOMAIN.into(),
            estate_id: env::current_account_id(),
            subject: env::predecessor_account_id(),
            action,
            heir_label: a.heir_label.clone(),
            commitment,
            nonce: a.nonce.0,
            expiry_ns: a.expiry_ns.0,
        })
        .unwrap();
        let sig: [u8; 64] = sig.0.as_slice().try_into().expect("signature != 64 bytes");
        require!(env::ed25519_verify(&sig, &msg, &self.attestor_pk), "invalid signature");
        self.used_nonces.insert(a.nonce.0);
        commitment
    }

    fn share_of(&self, label: &str, asset: &Asset) -> u16 {
        self.shares.get(&(label.to_string(), asset.clone())).copied()
            .unwrap_or_else(|| self.heirs.get(label).map(|h| h.default_bps).unwrap_or(0))
    }
    fn total_bps(&self, asset: &Asset) -> u128 {
        self.heirs.keys().map(|l| self.share_of(l, asset) as u128).sum()
    }
    fn only_grantor(&self) { require!(env::predecessor_account_id() == self.grantor, "not the grantor"); }
    fn not_unlocked(&self) { require!(self.pending_status() != Status::Unlocked, "estate unlocked"); }
}

/// NEP-141 deposits: grantor calls `ft_transfer_call` on the token with receiver = this estate.
#[near]
impl FungibleTokenReceiver for Estate {
    fn ft_on_transfer(&mut self, sender_id: AccountId, amount: U128, msg: String) -> PromiseOrValue<U128> {
        let _ = msg;
        let token = env::predecessor_account_id();
        require!(sender_id == self.grantor, "only the grantor deposits");
        require!(self.tokens.contains(&token), "token not whitelisted");
        require!(self.pending_status() != Status::Unlocked, "estate unlocked");
        *self.escrow.entry(Some(token)).or_insert(0) += amount.0;
        PromiseOrValue::Value(U128(0)) // keep all of it
    }
}

fn validate_timers(check_in_interval_sec: u32, grace_sec: u32) {
    require!((MIN_CHECK_IN_INTERVAL_SEC..=MAX_CHECK_IN_INTERVAL_SEC).contains(&check_in_interval_sec), "bad interval");
    require!((MIN_GRACE_SEC..=MAX_GRACE_SEC).contains(&grace_sec), "bad grace");
}

fn decode_ed25519(s: &str) -> [u8; 32] {
    let b58 = s.strip_prefix("ed25519:").expect("expected ed25519:<base58>");
    near_sdk::bs58::decode(b58).into_vec().expect("bad base58")
        .try_into().expect("ed25519 key must be 32 bytes")
}

/// Charges the caller for new storage plus `reserved` (NEAR this call forwards elsewhere), refunds
/// the rest, and returns the storage cost.
fn refund_storage_surplus(before: u64, reserved: u128) -> u128 {
    let used = env::storage_usage().saturating_sub(before) as u128;
    let storage_cost = env::storage_byte_cost().as_yoctonear() * used;
    let cost = storage_cost + reserved;
    let paid = env::attached_deposit().as_yoctonear();
    require!(paid >= cost, "attach more NEAR for storage");
    if paid > cost {
        Promise::new(env::predecessor_account_id()).transfer(NearToken::from_yoctonear(paid - cost)).detach();
    }
    storage_cost
}

fn log_event(event: &str, data: &str) {
    env::log_str(&format!(r#"EVENT_JSON:{{"standard":"herit","version":"1.0.0","event":"{event}","data":[{data}]}}"#));
}
```

### 6.2 Heir credential: soulbound dynamic NEP-171 (`src/nft.rs`)

The NFT views are implemented directly rather than through `near_contract_standards::NonFungibleToken`. The
standard's macros are deprecated, the approval machinery is dead weight for a soulbound token, and every
byte counts toward the Global Contract burn. The **JSON shapes** come from the standard types, so wallets
render the credentials.

```rust
use crate::*;
use near_contract_standards::non_fungible_token::metadata::{NFTContractMetadata, TokenMetadata, NFT_METADATA_SPEC};
use near_contract_standards::non_fungible_token::events::NftMint;
use near_contract_standards::non_fungible_token::Token;

pub fn emit_mint(owner: &AccountId, label: &str) {
    NftMint { owner_id: owner, token_ids: &[label], memo: Some("herit heir credential") }.emit();
}

#[near]
impl Estate {
    pub fn nft_metadata(&self) -> NFTContractMetadata {
        NFTContractMetadata {
            spec: NFT_METADATA_SPEC.into(),
            name: format!("Herit estate {}", env::current_account_id()),
            symbol: "HEIR".into(),
            icon: None, base_uri: None, reference: None, reference_hash: None,
        }
    }

    /// Metadata is computed from live state, so the token *is* the claim right.
    pub fn nft_token(&self, token_id: String) -> Option<Token> {
        let h = self.heirs.get(&token_id)?;
        let state = match self.pending_status() {
            Status::Unlocked if self.paid.contains(&(token_id.clone(), None)) => "claimed",
            Status::Unlocked => "claimable",
            Status::Grace => "grace",
            Status::Active => "locked",
        };
        Some(Token {
            token_id: token_id.clone(),
            owner_id: h.account_id.clone(),
            metadata: Some(TokenMetadata {
                title: Some(h.account_id.to_string()), // son.alice.herit-near.testnet
                description: Some(format!("Heir credential · {} · {state}", h.relationship)),
                extra: Some(format!(
                    r#"{{"relationship":"{}","share_bps":{},"state":"{state}"}}"#,
                    h.relationship, h.default_bps
                )),
                media: None, media_hash: None, copies: Some(1), issued_at: None, expires_at: None,
                starts_at: None, updated_at: None, reference: None, reference_hash: None,
            }),
            approved_account_ids: None,
        })
    }

    pub fn nft_tokens_for_owner(&self, account_id: AccountId, from_index: Option<U128>, limit: Option<u64>) -> Vec<Token> {
        self.heirs.iter()
            .filter(|(_, h)| h.account_id == account_id)
            .skip(from_index.map(|i| i.0 as usize).unwrap_or(0))
            .take(limit.unwrap_or(50) as usize)
            .filter_map(|(l, _)| self.nft_token(l.clone()))
            .collect()
    }
    // nft_tokens, nft_total_supply, nft_supply_for_owner: same pattern over self.heirs.

    #[payable]
    pub fn nft_transfer(&mut self, receiver_id: AccountId, token_id: String, approval_id: Option<u64>, memo: Option<String>) {
        let _ = (receiver_id, token_id, approval_id, memo);
        env::panic_str("heir credentials are soulbound");
    }
}
```

### 6.3 `herit-factory`

```rust
// near/contracts/herit-factory/src/lib.rs
use near_sdk::json_types::U64;
use near_sdk::store::{LookupMap, Vector};
use near_sdk::{env, is_promise_success, near, require, AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise};

const MIN_ESTATE_DEPOSIT: NearToken = NearToken::from_millinear(500); // tune after measuring storage
const INIT_GAS: Gas = Gas::from_tgas(30);
const MAX_PAGE: u32 = 100;

#[derive(BorshStorageKey)]
#[near]
enum Key { Estates, AllEstates }

#[near(serializers = [borsh, json])]
#[derive(Clone)]
pub struct EstateEntry { pub account_id: AccountId, pub grantor: AccountId, pub opened_at_ns: U64 }

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Factory {
    estate_code_hash: [u8; 32],
    attestor_pk: String, // "ed25519:<base58>", forwarded to each NEW estate
    estates: LookupMap<AccountId, Vec<AccountId>>, // grantor -> estates
    all_estates: Vector<EstateEntry>, // the public explorer; replaces the subgraph's estate list
}

#[near]
impl Factory {
    #[init]
    pub fn new(estate_code_hash: String, attestor_pk: String) -> Self {
        let h: [u8; 32] = near_sdk::bs58::decode(&estate_code_hash).into_vec().expect("bad base58")
            .try_into().expect("hash must be 32 bytes");
        Self {
            estate_code_hash: h,
            attestor_pk,
            estates: LookupMap::new(Key::Estates),
            all_estates: Vector::new(Key::AllEstates),
        }
    }

    /// Signed by the factory's own key. Only affects estates opened afterwards: existing estates
    /// are keyless and keep the attestor they were created with.
    #[private]
    pub fn set_attestor(&mut self, attestor_pk: String) {
        require!(attestor_pk.starts_with("ed25519:"), "expected ed25519:<base58>");
        self.attestor_pk = attestor_pk;
    }

    #[payable]
    pub fn open_estate(&mut self, label: String, check_in_interval_sec: u32, grace_sec: u32) -> Promise {
        require!(!label.contains('.'), "label must be a single segment");
        let grantor = env::predecessor_account_id();
        let deposit = env::attached_deposit();
        require!(deposit >= MIN_ESTATE_DEPOSIT, "attach at least 0.5 NEAR");
        let estate: AccountId = format!("{label}.{}", env::current_account_id()).parse().expect("invalid label");
        let args = near_sdk::serde_json::json!({
            "grantor": grantor, "attestor_pk": self.attestor_pk,
            "check_in_interval_sec": check_in_interval_sec, "grace_sec": grace_sec,
        });
        // No add_full_access_key: the estate account is created KEYLESS, so nobody, not even
        // the grantor or the factory, can redeploy, delete, or drain it outside the contract's rules.
        Promise::new(estate.clone())
            .create_account()
            .transfer(deposit)
            .use_global_contract(self.estate_code_hash)
            .function_call("new".to_string(), args.to_string().into_bytes(), NearToken::from_near(0), INIT_GAS)
            .then(Self::ext(env::current_account_id()).with_static_gas(Gas::from_tgas(10))
                .on_estate_created(estate, grantor, deposit))
    }

    #[private]
    pub fn on_estate_created(&mut self, estate: AccountId, grantor: AccountId, deposit: NearToken) -> bool {
        if is_promise_success() {
            let mut list = self.estates.get(&grantor).cloned().unwrap_or_default();
            list.push(estate.clone());
            self.estates.insert(grantor.clone(), list);
            self.all_estates.push(EstateEntry { account_id: estate, grantor, opened_at_ns: U64(env::block_timestamp()) });
            true
        } else {
            // A failed batch refunds the attached NEAR to the factory, so forward it back to the grantor.
            Promise::new(grantor).transfer(deposit).detach();
            false
        }
    }

    pub fn estates_of(&self, grantor: AccountId) -> Vec<AccountId> {
        self.estates.get(&grantor).cloned().unwrap_or_default()
    }

    /// Oldest first. The frontend reads `estates_count` and pages back from the end for newest first.
    pub fn estates(&self, from_index: Option<u32>, limit: Option<u32>) -> Vec<EstateEntry> {
        self.all_estates.iter()
            .skip(from_index.unwrap_or(0) as usize)
            .take(limit.unwrap_or(MAX_PAGE).min(MAX_PAGE) as usize)
            .cloned()
            .collect()
    }

    pub fn estates_count(&self) -> u32 { self.all_estates.len() }

    pub fn config(&self) -> (String, String) {
        (near_sdk::bs58::encode(self.estate_code_hash).into_string(), self.attestor_pk.clone())
    }
}
```

> The factory's own storage writes (`estates.insert`) are paid out of the factory's balance. Keep a
> couple of NEAR on `$FACTORY`, or charge the cost to the grantor. That is fine for a hackathon.

### 6.4 Fallback if the Global Contract burn is unaffordable

Embed the wasm in the factory instead: `const ESTATE_WASM: &[u8] = include_bytes!("../../../target/near/herit_estate/herit_estate.wasm");`
and replace `.use_global_contract(..)` with `.deploy_contract(ESTATE_WASM.to_vec())`. Each estate then **locks**
about 1 NEAR per 100 KB (refundable, not burned), so raise `MIN_ESTATE_DEPOSIT` to match. Global Contracts are the
better story for judges. Use this path only if the budget fails.

### 6.5 Remaining functions (checklist; port from the Solidity, rule for rule)

- [ ] `add_token(token)` (payable): whitelist ≤ `MAX_TOKENS`, then `ext_ft::ext(token).with_attached_deposit(0.00125 NEAR).storage_deposit(Some(current), Some(true))`. The compiler warns about `MAX_TOKENS` and `ext_ft`'s trait until this exists.
- [ ] `set_share(label, asset, bps)`: grantor, not unlocked, and `total_bps(asset) <= 10_000` (= `HeritRegistry__ShareOverflow`). Log `share_set`.
- [ ] `log_event("deposited", ...)` in `ft_on_transfer`, to match `deposit`
- [ ] `withdraw(asset, amount, attestation, signature)`: grantor, not unlocked, **`ACTION_WITHDRAW` attestation with the bound commitment** (new NEAR feature, §2.3 item 5). Debit before the promise, re-credit in a `#[private]` callback on failure, and log `withdrawn`.
- [ ] `remove_heir(label)` before unlock, plus `NftBurn` event. The heir's account survives because the estate holds no key to it, so that label can't be added again (`add_heir` rolls back).
- [ ] Views: `heirs_of`, `share_of`, `escrow_of`, `snapshot_of`, `grantor_of`, `config_of`, `nft_tokens`, `nft_total_supply`, `nft_supply_for_owner`
- [ ] Unit tests (`near_sdk::test_utils::VMContextBuilder`) for: bound-commitment mismatch, nonce replay, expiry > 30 min, check-in after an unpoked lapse is refused, grantor commitment cannot claim, a commitment cannot claim two labels, rounding-down payout, double claim refused, an estate with no check-in never lapses, `configure` refused outside Active, `configure` that would leave Active refused, a claim reduces `escrow` and a failed payout restores it

---

## 7. Attestor: World ID → ed25519, running on OutLayer

On Sepolia, the attestor key sits in the backend's environment, and `ARCHITECTURE.md` §8 names it the biggest
centralization point in the build. On NEAR, verification and signing move into an **OutLayer** worker. OutLayer runs
it inside an Intel TDX enclave and derives the signing key there, so nobody holds the private key, including you.

**The contract doesn't change.** It still calls `env::ed25519_verify` against one `attestor_pk`. OutLayer only changes
where the signature is produced, which is also why falling back (§7.5) costs so little.

| Step | Runs in | Secret it holds |
| --- | --- | --- |
| 1. Sign the World ID request (`/api/worldid/sign`) | Next.js, unchanged | `RP_SIGNING_KEY`: proves who is asking |
| 2. Verify the proof, derive the commitment, sign the attestation | **OutLayer worker** | Attestor key, in the TEE only |
| 3. Forward the proof, run preflight (`/api/worldid/verify`) | Next.js, now a thin proxy | Payment Key: can spend compute, cannot forge an attestation |
| 4. Send `check_in` / `claim` | The user's wallet | Their own key |

**What this does and doesn't prove.** The attestation shows that a specific wasm hash produced the output in genuine
TDX hardware. Readers still have to audit that code, and you still trust Intel and World's Cloud Verify.

### 7.1 Worker

```
near/attestor-worker/      # NOT a member of the contracts workspace (it targets wasm32-wasip2)
├── Cargo.toml
├── manifest.json          # embedded in the wasm, so it is covered by the wasm hash
├── wit/signing-keys.wit
└── src/main.rs
```

Copy the WIT file from OutLayer's repo:

```bash
mkdir -p near/attestor-worker/wit && curl -fsSL \
  https://raw.githubusercontent.com/out-layer/outlayer/main/worker/wit/deps/signing-keys.wit \
  -o near/attestor-worker/wit/signing-keys.wit
```

`manifest.json`:

```json
{
  "display": { "name": "Herit attestor" },
  "operations": ["public_key", "attest"],
  "capabilities": { "network": ["developer.world.org"] },
  "signing_keys": [
    { "path": "attestor", "type": "ed25519", "caller": "none" },
    { "path": "commitment-salt", "type": "ed25519", "caller": "none" }
  ]
}
```

`"caller": "none"` is essential. The default binds a key to whoever calls, which would give every user a different
key, while the contract expects exactly one. Don't rename a key's `path`, and don't delete and recreate the project:
either one changes the key.

`Cargo.toml`:

```toml
[package]
name = "herit-attestor"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "herit-attestor"
path = "src/main.rs"

[dependencies]
wit-bindgen = "0.36"
wasi-http-client = "0.2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
borsh = { version = "1", features = ["derive"] }
sha2 = "0.10"
bs58 = "0.5"
base64 = "0.22"

[profile.release]
opt-level = "z"
lto = true
strip = false # the outlayer.manifest custom section must survive
```

`src/main.rs` (set `RP_ID` to your `WLD_RP_ID`):

```rust
// near/attestor-worker/src/main.rs
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use borsh::BorshSerialize;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

mod host {
    wit_bindgen::generate!({ world: "signing-keys-host", path: "wit" });
}
use host::outlayer::signing_keys::api as keys;

// The manifest travels inside the wasm, so it is covered by the wasm's hash.
#[cfg(target_arch = "wasm32")]
#[used]
#[link_section = "outlayer.manifest"]
static OUTLAYER_MANIFEST: [u8; include_bytes!("../manifest.json").len()] = *include_bytes!("../manifest.json");

const DOMAIN: &str = "herit-near:v1";
const ESTATE_SUFFIX: &str = ".herit-near.testnet"; // this worker only attests for estates under the factory
const RP_ID: &str = "rp_REPLACE_ME"; // WLD_RP_ID. Public, so it lives in the code rather than the environment
const VERIFY_BASE: &str = "https://developer.world.org/api/v4/verify";
const TTL_NS: u64 = 15 * 60 * 1_000_000_000; // under the contract's 30-minute cap

/// Must match the contract's `SignedPayload` field order and types exactly: Borsh is positional.
#[derive(BorshSerialize)]
struct SignedPayload {
    domain: String,
    estate_id: String,
    subject: String,
    action: u8,
    heir_label: Option<String>,
    commitment: [u8; 32],
    nonce: u64,
    expiry_ns: u64,
}

#[derive(Deserialize)]
struct AttestInput {
    estate_id: String,
    subject: String,
    action: u8, // 0 check-in, 1 claim, 2 withdraw
    heir_label: Option<String>,
    result: Value, // the IDKit result, forwarded to World untouched
}

fn main() {
    let out = run().unwrap_or_else(|error| json!({ "error": error }));
    print!("{out}");
    io::stdout().flush().expect("flush");
}

fn run() -> Result<Value, String> {
    let mut raw = String::new();
    io::stdin().read_to_string(&mut raw).map_err(|e| e.to_string())?;
    let input: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    match input["operation"].as_str() {
        Some("public_key") => {
            let pk = keys::public_key("attestor", None)?;
            Ok(json!({ "attestor_pk": format!("ed25519:{}", bs58::encode(pk).into_string()) }))
        }
        Some("attest") => attest(serde_json::from_value(input).map_err(|e| e.to_string())?),
        _ => Err("unknown operation".into()),
    }
}

fn attest(input: AttestInput) -> Result<Value, String> {
    if input.action > 2 {
        return Err("bad action".into());
    }
    if (input.action == 1) != input.heir_label.is_some() {
        return Err("heir_label is required for a claim and only for a claim".into());
    }
    let label = input.estate_id.strip_suffix(ESTATE_SUFFIX).filter(|l| !l.is_empty() && !l.contains('.'))
        .ok_or("estate_id is not an estate of this factory")?;

    // Derived here, never taken from the caller, or a proof for another estate would pass.
    let world_action = format!("herit:{label}");
    let nullifier = verify_with_world(&world_action, &input.result)?;

    // The salt is a signature by a TEE-held key: deterministic, stable across upgrades, and nobody
    // can read or replace it. NOT an environment variable: on a `caller: "none"` key the caller
    // can supply the environment, and a swapped salt would make one human look like two.
    let salt = keys::sign("commitment-salt", None, b"herit-near:commitment-salt:v1")?;
    let commitment: [u8; 32] = Sha256::new()
        .chain_update(b"herit-near:commitment:v1")
        .chain_update(&salt)
        .chain_update(nullifier.trim().to_lowercase().as_bytes())
        .finalize()
        .into();

    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos() as u64;
    let expiry_ns = now + TTL_NS;
    // Random enough for a replay guard: unique per proof and per moment.
    let digest = Sha256::new().chain_update(input.result.to_string()).chain_update(now.to_le_bytes()).finalize();
    let nonce = u64::from_le_bytes(digest[..8].try_into().expect("8 bytes"));

    let message = borsh::to_vec(&SignedPayload {
        domain: DOMAIN.into(),
        estate_id: input.estate_id,
        subject: input.subject,
        action: input.action,
        heir_label: input.heir_label.clone(),
        commitment,
        nonce,
        expiry_ns,
    })
    .map_err(|e| e.to_string())?;
    let signature = keys::sign("attestor", None, &message)?;

    Ok(json!({
        "attestation": {
            "action": input.action,
            "heir_label": input.heir_label,
            "commitment": B64.encode(commitment),
            "nonce": nonce.to_string(),
            "expiry_ns": expiry_ns.to_string(),
        },
        "signature": B64.encode(signature),
    }))
}

/// Port of `verifyWithWorld` in the Sepolia route. Fails closed.
fn verify_with_world(action: &str, result: &Value) -> Result<String, String> {
    let body = serde_json::to_vec(result).map_err(|e| e.to_string())?;
    let url = format!("{VERIFY_BASE}/{RP_ID}");
    let response = wasi_http_client::Client::new()
        .post(&url)
        .header("Content-Type", "application/json")
        .body(&body)
        .send()
        .map_err(|e| format!("world verify: {e}"))?;
    let status = response.status();
    let reply: Value = serde_json::from_slice(&response.body().map_err(|e| e.to_string())?).unwrap_or(Value::Null);

    let verified = reply["code"].as_str().is_none()
        && (reply["success"] == json!(true) || reply["responses"].as_array().is_some_and(|r| !r.is_empty()));
    if !(200..300).contains(&status) || !verified {
        return Err(format!("proof rejected (status {status})"));
    }
    for side in [&reply["action"], &result["action"]] {
        if side.as_str().is_some_and(|a| a != action) {
            return Err("proof was issued for a different action".into());
        }
    }
    // Read the Selfie Check credential specifically. `face` is its legacy alias.
    // Checkpoint 5 tightens this: the proof must hold exactly one response and it must be the
    // Selfie Check, because World's v4 reply doesn't list nullifiers per credential.
    result["responses"].as_array()
        .and_then(|items| items.iter().find(|i| matches!(i["identifier"].as_str(), Some("selfie" | "face"))))
        .and_then(|i| i["nullifier"].as_str())
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "no selfie check nullifier in the proof".to_string())
}
```

**Don't read the environment in this worker.** With a `caller: "none"` key, the caller can supply environment
variables. A swapped `NULLIFIER_SALT` would make one human look like two and defeat the one-human-one-heir rule, which
is why the salt comes from a second TEE key.

### 7.2 Build, publish, read the key

```bash
rustup target add wasm32-wasip2
cd near/attestor-worker && cargo build --target wasm32-wasip2 --release
```

Create the project in the OutLayer dashboard (`/projects` → New Project, name `herit-attestor`) with the wasm's URL
and SHA-256, using a testnet wallet account. For a build others can verify, use OutLayer's
`scripts/build_github_wasm.sh --dir near/attestor-worker --target wasm32-wasip2 --twice`.

Then read the public key. This is the `attestor_pk` for real estates:

```bash
export OUTLAYER_API=https://testnet-api.outlayer.ai
export OUTLAYER_PROJECT=<your-account>.testnet/herit-attestor
curl -s -X POST "$OUTLAYER_API/call/$OUTLAYER_PROJECT" \
  -H "X-Payment-Key: $OUTLAYER_PAYMENT_KEY" -H 'Content-Type: application/json' \
  -d '{"input":{"operation":"public_key"}}'
```

### 7.3 Next.js route

`app/api/worldid/verify/route.ts` no longer signs anything. Keep `OUTLAYER_PAYMENT_KEY` server-only.

```ts
const res = await fetch(`${process.env.OUTLAYER_API}/call/${process.env.OUTLAYER_PROJECT}`, {
  method: "POST",
  headers: { "Content-Type": "application/json", "X-Payment-Key": process.env.OUTLAYER_PAYMENT_KEY! },
  body: JSON.stringify({
    input: { operation: "attest", estate_id: estateId, subject, action, heir_label: heirLabel, result },
  }),
});
// Never retry a 500 blindly: the call may already be queued and charged.
const { status, output, error } = await res.json();
if (status !== "completed") throw new VerificationError(error ?? "attestor failed");
const signed = typeof output === "string" ? JSON.parse(output) : output;
if (signed.error) throw new VerificationError(signed.error);
```

Port `lib/server/preflight.ts` to the views `status_of`, `commitment_of`, `claim_commitment_used` and `heirs_of`. The
worker computes the commitment, so preflight now runs **after** it and the route discards the attestation if preflight
fails. `subject` must be the NEAR account that will **send** the transaction: the grantor for check-in, the heir's
sub-account for claim. The World action string stays `herit:{label}`, and the worker derives it from `estate_id`.

### 7.4 Spike this first (Tue 10 Nov, 3 hours maximum)

Confirmed on 2026-11-09: `outlayer.testnet` is live, `testnet-api.outlayer.ai` answers and asks for a Payment Key, and
`estimate_execution_cost` quotes about **0.004 NEAR** per run at 10B instructions and 30 s. The worker type-checks
against OutLayer's WIT.

Not confirmed, because it needs a funded account. The spike passes when all of these work:

- [ ] A Payment Key can be funded for `testnet-api`. OutLayer's docs only show mainnet USDT. If it can't, have a
      funded relay account call `request_execution` on `outlayer.testnet` from the route. `caller: "none"` means the
      key stays the same, so nothing else changes.
- [ ] The worker builds for `wasm32-wasip2` and the project accepts it.
- [ ] `public_key` returns a stable key across two calls.
- [ ] One real Selfie Check returns an attestation, proving the worker can reach `developer.world.org`, and the
      contract accepts it.

### 7.5 Dev signer and fallback

The CLI smoke test (§8.4) can't produce a real Selfie Check, so it uses a local dev key. The same key is the fallback
if the spike fails: sign in the Next.js route instead, and the contracts stay as they are.

**Generate the dev key** (once; it never needs NEAR or gas):

```bash
cd frontend && npm i near-api-js@7.4.0 borsh@2.0.0 @hot-labs/near-connect@0.12.0
node -e 'const {KeyPair}=require("near-api-js");const k=KeyPair.fromRandom("ed25519");console.log("ATTESTOR_SECRET_KEY="+k.toString());console.log("ATTESTOR_PUBLIC_KEY="+k.getPublicKey().toString())'
```

Keep it out of git. If it's in `frontend/.env.local`, it must never be `NEXT_PUBLIC_`.

`frontend/scripts/sign-attestation.mjs` (it lives in `frontend/` so it can resolve `near-api-js`):

```js
// frontend/scripts/sign-attestation.mjs — DEV ONLY. Mints test attestations with a local key.
// node frontend/scripts/sign-attestation.mjs --estate <id> --subject <id> --action 0|1|2 --human <name> [--label son]
import { createHash, randomBytes } from "node:crypto";
import { parseArgs } from "node:util";
import { serialize } from "borsh";
import { KeyPair } from "near-api-js";

const { values: a } = parseArgs({
  options: {
    estate: { type: "string" }, subject: { type: "string" }, action: { type: "string" },
    human: { type: "string" }, label: { type: "string" },
  },
});

// Must match `SignedPayload` field ORDER and TYPES exactly; Borsh is positional.
export const SCHEMA = {
  struct: {
    domain: "string",
    estate_id: "string",
    subject: "string",
    action: "u8",
    heir_label: { option: "string" },
    commitment: { array: { type: "u8", len: 32 } },
    nonce: "u64",
    expiry_ns: "u64",
  },
};

// A different --human gives a different commitment, standing in for a different face.
const commitment = createHash("sha256").update(`dev-human:${a.human}`).digest();
const nonce = randomBytes(8).readBigUInt64LE();
const expiry_ns = BigInt(Date.now() + 15 * 60_000) * 1_000_000n; // under the 30-minute cap
const action = Number(a.action);
const heir_label = a.label ?? null;

const bytes = serialize(SCHEMA, {
  domain: "herit-near:v1", estate_id: a.estate, subject: a.subject, action, heir_label,
  commitment: [...commitment], nonce, expiry_ns,
});
const { signature } = KeyPair.fromString(process.env.ATTESTOR_SECRET_KEY).sign(bytes);

console.log(JSON.stringify({
  attestation: {
    action, heir_label, commitment: commitment.toString("base64"),
    nonce: nonce.toString(), expiry_ns: expiry_ns.toString(),
  },
  signature: Buffer.from(signature).toString("base64"),
}));
```

These bytes were compared against `near_sdk::borsh::to_vec` on 2026-11-09 and match, including `heir_label: None`. If
you ever change `SignedPayload`, repeat that comparison first, because most "invalid signature" errors in ports like
this come from a field-order mismatch.

**If you fall back**, derive the commitment in the route as `sha256("herit-near:commitment:v1" || NULLIFIER_SALT || nullifier)`
with a fresh salt. Never mix signers on one estate: a commitment from one won't match the other, so that grantor would
read as the wrong human.

---

## 8. Build → test → deploy (testnet)

### 8.1 Build and measure

```bash
cd near/contracts/herit-estate
cargo near build reproducible-wasm          # Docker; use non-reproducible-wasm while iterating
ls -l ../../target/near/herit_estate/*.wasm # size in KB / 10 = NEAR burned as Global Contract
```

> cargo-near writes to `target/near/<crate>/`. If your version writes elsewhere, the build output prints the path.

`near/scripts/code-hash.py` (no dependencies; base58 of the wasm's sha256, which is the Global Contract hash):

```python
import hashlib, sys
A = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
d = hashlib.sha256(open(sys.argv[1], "rb").read()).digest()
n, s = int.from_bytes(d, "big"), ""
while n: n, r = divmod(n, 58); s = A[r] + s
print("1" * (len(d) - len(d.lstrip(b"\0"))) + s)
```

### 8.2 Test locally (sandbox, free)

```bash
cd near && cargo test            # unit tests
cargo test --test lifecycle      # near-workspaces 0.24: spins up a local sandbox
```

`tests/lifecycle.rs` should cover the README story end to end with timers of 120 s / 60 s: open estate → add 2
heirs (60/40) → deposit 1 NEAR + 100 mock-USDC → check in → fast-forward (`worker.fast_forward(blocks)`) →
poke → Grace → fast-forward → Unlocked → heir 1 claims 0.6 NEAR + 60 USDC → heir 1 claims again (refused) →
the thief's commitment is refused. Generate each heir's key in the test and sign as the sub-account `add_heir`
created. Also cover the rollback: an `add_heir` whose sub-account already exists leaves no heir record and
refunds the grantor.

### 8.3 Deploy

```bash
# 1) Mock USDC (testnet only); copy of near-sdk-rs/examples/fungible-token
near contract deploy $TOKEN use-file near/target/near/mock_ft/mock_ft.wasm \
  with-init-call new_default_meta json-args "{\"owner_id\":\"$GRANTOR\",\"total_supply\":\"1000000000000\"}" \
  prepaid-gas '100 Tgas' attached-deposit '0 NEAR' network-config $NET sign-with-keychain send

# 2) Publish the estate code as an IMMUTABLE Global Contract (burns NEAR; see §4)
near contract deploy-as-global use-file near/target/near/herit_estate/herit_estate.wasm \
  as-global-hash $CODE network-config $NET sign-with-keychain send
export ESTATE_HASH=$(python3 near/scripts/code-hash.py near/target/near/herit_estate/herit_estate.wasm)

# 3) Deploy + init the factory IN ONE TRANSACTION (an uninitialised #[init] can be front-run)
#    ATTESTOR_PUBLIC_KEY is the DEV key from §7.5 for now; §8.4 ends by switching to OutLayer.
near contract deploy $FACTORY use-file near/target/near/herit_factory/herit_factory.wasm \
  with-init-call new json-args "{\"estate_code_hash\":\"$ESTATE_HASH\",\"attestor_pk\":\"$ATTESTOR_PUBLIC_KEY\"}" \
  prepaid-gas '30 Tgas' attached-deposit '0 NEAR' network-config $NET sign-with-keychain send
```

### 8.4 Smoke test with the CLI (live testnet run)

```bash
call() { near contract call-function as-transaction "$1" "$2" json-args "$3" prepaid-gas "${5:-100 Tgas}" \
         attached-deposit "${4:-0 NEAR}" sign-as "$6" network-config $NET sign-with-keychain send; }
view() { near contract call-function as-read-only "$1" "$2" json-args "${3:-{\}}" network-config $NET now; }
ESTATE=alice.$FACTORY

call $FACTORY open_estate '{"label":"alice","check_in_interval_sec":120,"grace_sec":60}' '1 NEAR' '' $GRANTOR

# Each heir generates their own key (on the heir's device in production) and shares only the public half.
newkey() { (cd frontend && node -e 'const {KeyPair}=require("near-api-js");const k=KeyPair.fromRandom("ed25519");console.log(k.toString(),k.getPublicKey().toString())'); }
read HEIR1_SK HEIR1_PK < <(newkey)
read HEIR2_SK HEIR2_PK < <(newkey)

# add_heir creates son.alice.herit-near.testnet and daughter.alice.herit-near.testnet (0.15 NEAR covers the 0.1 seed + storage)
call $ESTATE add_heir "{\"label\":\"son\",\"public_key\":\"$HEIR1_PK\",\"relationship\":\"son\",\"default_bps\":6000}" '0.15 NEAR' '' $GRANTOR
call $ESTATE add_heir "{\"label\":\"daughter\",\"public_key\":\"$HEIR2_PK\",\"relationship\":\"daughter\",\"default_bps\":4000}" '0.15 NEAR' '' $GRANTOR

# Put the heirs' keys in the keychain so they can sign. If the CLI asks which account, enter $HEIR1 / $HEIR2.
near account import-account using-private-key "$HEIR1_SK" network-config $NET
near account import-account using-private-key "$HEIR2_SK" network-config $NET

call $ESTATE deposit '{}' '1 NEAR' '' $GRANTOR
view $ESTATE nft_tokens_for_owner "{\"account_id\":\"$HEIR1\"}"     # state: "locked"

# check in: DEV attestation (§7.5). This first check-in is what starts the clock.
export ATTESTOR_SECRET_KEY=...   # the dev key from §7.5
ATT=$(node frontend/scripts/sign-attestation.mjs --estate $ESTATE --subject $GRANTOR --action 0 --human grantor)
call $ESTATE check_in "$ATT" '0 NEAR' '' $GRANTOR

sleep 190; call $ESTATE poke_expiry '{}' '0 NEAR' '' $HEIR1          # anyone can poke
view $ESTATE status_of                                              # "Unlocked"
ATT=$(node frontend/scripts/sign-attestation.mjs --estate $ESTATE --subject $HEIR1 --action 1 --human son --label son)
call $ESTATE claim "$(echo "$ATT" | jq '. + {label:"son"}')" '0 NEAR' '200 Tgas' $HEIR1

# Switch future estates to the OutLayer key from §7.2 (export OUTLAYER_ATTESTOR_PK first). alice.herit-near.testnet keeps the dev attestor forever,
# so treat it as a test estate and open real demo estates after this line.
call $FACTORY set_attestor "{\"attestor_pk\":\"$OUTLAYER_ATTESTOR_PK\"}" '0 NEAR' '' $FACTORY
```

**For judges,** point the README's reproduction steps at `cargo test --test lifecycle`. It runs free in a local
sandbox with its own generated key, so reproducing the flow doesn't need your dev key or a real Selfie Check.

Inspect everything on <https://testnet.nearblocks.io/address/alice.herit-near.testnet>. Paste these exact
transaction links into the README's "Proof it ran" section, as the Sepolia README does today.

---

## 9. Frontend changes (Next.js 16.3.4)

Read `frontend/node_modules/next/dist/docs/` first, per `frontend/AGENTS.md`.

| Replace | With |
| --- | --- |
| `wagmi` / `viem` providers | `NearConnector` from `@hot-labs/near-connect` (`connector.connect()`, then `wallet.signAndSendTransaction({ receiverId, actions:[{ type:"FunctionCall", params:{ methodName, args, gas, deposit }}]})`) |
| `useReadContract` | `new JsonRpcProvider({ url: "https://test.rpc.fastnear.com" }).callFunction({ contractId, method, args })` from `near-api-js` |
| `@ensdomains/ensjs` name resolution | Native NEAR account ids (validate with `/^(([a-z\d]+[-_])*[a-z\d]+\.)*([a-z\d]+[-_])*[a-z\d]+$/`, 2–64 chars) |
| Heir address input on the add-heir form | A label plus the heir's **public key**. The heir generates the key on their own device, then imports the private key into a wallet that supports private-key import once `add_heir` lands. |
| Heir → estate lookup | Strip the first segment of the heir's account id: `son.alice.herit-near.testnet` → `alice.herit-near.testnet` |
| `lib/estate/use-all-estates.ts` (subgraph) | Factory `estates_count` + `estates(from_index, limit)`, paged back from the end for newest first |
| `lib/estate/use-activity.ts` (subgraph) | `GET /api/activity?estate=<id>`, below |
| `ATTESTOR_PRIVATE_KEY`, `NULLIFIER_SALT` | `OUTLAYER_API`, `OUTLAYER_PROJECT`, `OUTLAYER_PAYMENT_KEY` (§7) |
| Sepolia addresses in `lib/` | `NEXT_PUBLIC_NEAR_NETWORK`, `NEXT_PUBLIC_HERIT_FACTORY` |

Gas for `claim` should be `"200000000000000"` (200 Tgas). Amounts are yoctoNEAR **strings**.

**Activity feed.** No hackathon partner offers indexing, so the feed uses FastNEAR's Transactions API, behind a route
so that the key stays on the server. Both calls and the response shape were checked on testnet on 2026-11-09:

```ts
// app/api/activity/route.ts
const BASE = "https://tx.test.fastnear.com"; // https://tx.main.fastnear.com on mainnet

async function post(path: string, body: unknown) {
  const res = await fetch(`${BASE}${path}`, {
    method: "POST",
    // FastNEAR's docs require a key. Take the header name from its OpenAPI spec and add it here.
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`fastnear ${res.status}`);
  return res.json();
}

export async function GET(request: Request) {
  const estate = new URL(request.url).searchParams.get("estate")!;
  const { account_txs } = await post("/v0/account", { account_id: estate, limit: 50, desc: true });
  const times = new Map(account_txs.map((t: any) => [t.transaction_hash, t.tx_block_timestamp]));
  const { transactions } = await post("/v0/transactions", {
    tx_hashes: account_txs.map((t: any) => t.transaction_hash),
  });

  const events = transactions.flatMap((tx: any) =>
    tx.receipts
      .filter((r: any) => r.receipt.receiver_id === estate) // this estate's logs only
      .flatMap((r: any) => r.execution_outcome.outcome.logs as string[])
      .filter((log) => log.startsWith("EVENT_JSON:"))
      .map((log) => JSON.parse(log.slice("EVENT_JSON:".length)))
      .filter((e) => e.standard === "herit")
      .map((e) => ({ ...e, txHash: tx.transaction.hash, timestamp: times.get(tx.transaction.hash) })),
  );
  return Response.json({ events });
}
```

The maximum number of `tx_hashes` per request isn't documented, so lower `limit` if a call is refused. Timers, balances
and status stay live contract reads, as they were on Sepolia.

| Subgraph `ActivityKind` | NEAR event |
| --- | --- |
| `EstateOpened`, `EstateConfigured` | `estate_opened`, `configured` |
| `HeirRegistered` | `heir_added` (plus the NEP-171 mint) |
| `CheckedIn`, `CheckInAttested` | `checked_in` (now one step) |
| `EnteredGrace`, `Unlocked` | `entered_grace`, `unlocked` |
| `HeirUnlocked` | None: unlock is estate-wide, and `unlocked` carries the heir count |
| `Deposited`, `Withdrawn` | `deposited`, `withdrawn` |
| `Claimed`, `ClaimSettled`, `ClaimAttested` | `paid`, one per asset. `payout_failed` is new. |

---

## 10. NEAR gotchas that bite Solidity developers

1. **Async receipts, no global revert.** A failed `ft_transfer` or `transfer` does not undo your state. Every outbound payment needs a `#[private]` callback that rolls back (`on_payout` above). Write state *before* scheduling the promise.
2. **Time is nanoseconds.** `env::block_timestamp()` is `u64` ns. Convert seconds × 1e9 at the boundary only.
3. **u64/u128 in JSON are strings.** Use `U64` and `U128` in every public signature; JS loses precision above 2^53.
4. **Storage staking.** Every byte stored locks 1e19 yocto (1 NEAR/100 KB) from the contract's own balance. Make callers pay (`#[payable]` + refund the surplus). **Never** use `env::account_balance()` as escrow; it includes the locked storage stake. Keep explicit `escrow` accounting.
5. **Methods aren't payable by default.** Attaching NEAR to a non-`#[payable]` method panics.
6. **Deploy + init in one transaction.** An `#[init]` left uncalled can be front-run by anyone.
7. **300 Tgas per transaction, total.** `claim` fans out one transfer plus one callback per asset (~25 Tgas each). `MAX_TOKENS = 5` keeps it safely under 300 Tgas.
8. **NEP-141 storage registration.** The estate must be `storage_deposit`-registered on each token before it can receive it, and the heir must be registered before `ft_transfer`. Unregistered heirs make the payout fail (and the callback rolls it back), so the frontend should register the heir first (`storage_deposit` from the heir, ~0.00125 NEAR, paid out of the account's `HEIR_ACCOUNT_SEED`).
9. **`ft_transfer` needs exactly 1 yoctoNEAR attached** (the security pattern that forces a full-access-key signature).
10. **Keyless means final.** Estates have no access keys, so code bugs are permanent per estate. This is the same trade-off as the Sepolia `immutable`s. Keep the **factory** keyed during the hackathon so you can redeploy it, and publish a fresh global hash for each estate-code fix. Old estates keep the old code.
11. **Global by hash vs by account.** By hash = immutable (chosen here, matching Herit's trust model). By account = the publisher can upgrade every estate at once, which reintroduces a privileged key. Say this explicitly in the README; judges care.
12. **Account id rules.** Lowercase `[a-z0-9_-]`, segments separated by `.`, 2–64 chars total. The full `son.alice.herit-near.testnet` must fit: `.herit-near.testnet` takes 14 chars, leaving 50 for `<heir>.<estate>`.
13. **Only the direct parent can create a sub-account.** `herit-near.testnet` cannot create `son.alice.herit-near.testnet`, and neither can the grantor's wallet. Only the contract at `alice.herit-near.testnet` can, which is why `add_heir` does it.
14. **A lost heir key has no recovery.** The estate holds no key to the heir's account and can't add one later. Before unlock, the grantor can `remove_heir` and add a replacement under a new label. After unlock, nothing can rescue that heir's share, so the frontend should make heirs back up their key before the grantor submits `add_heir`.
15. **The attestor is fixed per estate.** Each estate copies `attestor_pk` when it opens and is keyless, so `set_attestor` only affects new estates. Deleting and recreating the OutLayer project changes its key, which would strand every estate tied to the old one.
16. **NEAR Lake is retired, and NearBlocks v1 is gone on testnet.** Older tutorials still point to both. NearBlocks v3 also renames paths (`/account/` → `/accounts/`).

---

## 11. Makefile additions

Append a NEAR section to the existing `Makefile`:

```make
##@ NEAR — near/ workspace (testnet by default)

NET ?= testnet

.PHONY: near-build
near-build: ## Build all NEAR contracts (reproducible, needs Docker)
	cd near/contracts/herit-estate  && cargo near build reproducible-wasm
	cd near/contracts/herit-factory && cargo near build reproducible-wasm
	cd near/contracts/mock-ft       && cargo near build non-reproducible-wasm

.PHONY: near-test
near-test: ## Unit + sandbox integration tests
	cd near && cargo test

.PHONY: near-publish-estate
near-publish-estate: check-CODE ## Publish estate code as immutable Global Contract (burns NEAR)
	near contract deploy-as-global use-file near/target/near/herit_estate/herit_estate.wasm \
	  as-global-hash $(CODE) network-config $(NET) sign-with-keychain send
	@python3 near/scripts/code-hash.py near/target/near/herit_estate/herit_estate.wasm

.PHONY: near-deploy-factory
near-deploy-factory: check-FACTORY check-ESTATE_HASH check-ATTESTOR_PUBLIC_KEY ## Deploy + init factory in one tx
	near contract deploy $(FACTORY) use-file near/target/near/herit_factory/herit_factory.wasm \
	  with-init-call new json-args '{"estate_code_hash":"$(ESTATE_HASH)","attestor_pk":"$(ATTESTOR_PUBLIC_KEY)"}' \
	  prepaid-gas '30 Tgas' attached-deposit '0 NEAR' network-config $(NET) sign-with-keychain send

.PHONY: near-attestor-build
near-attestor-build: ## Build the OutLayer attestor worker
	cd near/attestor-worker && cargo build --target wasm32-wasip2 --release
```

Add a second CI job to `.github/workflows/test.yml`: `rustup target add wasm32-unknown-unknown wasm32-wasip2`, install cargo-near, run `cargo near build non-reproducible-wasm` for each contract, build the worker, then `cargo test`. Leave the Foundry job as it is.

---

## 12. Plan to the deadline (from Mon 9 Nov)

| Day | Goal | Done when |
| --- | --- | --- |
| **Mon 9** | Tag `pre-hackathon`, add LICENSE, upgrade tools (§3), claim `herit-near.testnet`, `near/` scaffold, estate core with unit tests | The §6.5 unit tests pass |
| **Tue 10** | **OutLayer spike (§7.4, 3 h maximum)**, then NFT credential, NEP-141 path, liveness-gated withdraw, sandbox lifecycle test | Spike passes or the fallback is chosen; `cargo test --test lifecycle` is green |
| **Wed 11** | Factory, Global Contract publish, testnet deploy, CLI smoke run (§8.4), `set_attestor` | Testnet NearBlocks links for every step |
| **Thu 12** | Verify route, frontend on near-connect, estate list and activity feed, deploy to Vercel | Grantor and heir flows work in the browser with a real Selfie Check |
| **Fri 13** | README "Built at NEAR HACKATHON", 2-min video, tag `hackathon-submission` | A fresh clone reproduces the demo from the README alone |
| **Sat 14** | X post, form submission **by 12:00 UTC** | Confirmation received. Leave 12 h of buffer before 23:59 UTC. |

The BTC, ETH and SOL checkpoint (§2.3 item 6) fits only after Thursday's "done when" is met.

Cut order if you fall behind: BTC, ETH and SOL through Intents → activity feed → NEP-141 (NEAR-only demo) → liveness-gated
withdraw. If OutLayer slips, fall back (§7.5) rather than cutting anything. Never cut the NFT credential: it **is** the
track.

---

## 13. Submission checklist

- [ ] `git tag pre-hackathon 55115bf` and `git tag hackathon-submission <final>`, both pushed. Include a README link to `compare/pre-hackathon...hackathon-submission`.
- [ ] `LICENSE` (MIT) at the repo root
- [ ] README top section "**Built at NEAR HACKATHON**":
  - what's new
  - disclosure: "Herit first won at ETHOnline 2026 on Sepolia. Only the NEAR integration is submitted for judging."
  - partner tech, libraries and AI tools used (OutLayer, HOT near-connect, FastNEAR, World ID, Claude Code, near-sdk, near-contract-standards, near-api-js, borsh)
  - testnet account links, plus the OutLayer project, its wasm hash and one attestation link
  - a "do X → see Y" reproduction script (`cargo test --test lifecycle`, plus §8.4 for the live testnet run)
- [ ] Repo is **public**
- [ ] Hosted demo live until ≥ 25 Nov, testnet accounts funded
- [ ] Public X post from your own account: the user problem first, one complete moment (heir credential flipping `locked → claimable`, then a claim), NEAR's concrete role. Tag **@NEARProtocol** and **#NEARHackathon** (optionally @NearLegion and @NearBuilders), with the video attached.
- [ ] Form at <https://nearhackathon.com/#apply>: tracks **`nfts`, `realworld`, `open`**, mainnet payout account, repo link, X post link, 20–2,000-char description. **Submit once.** Make later edits via `/request-changes`.

---

## Sources

- NEAR HACKATHON: <https://nearhackathon.com>, <https://nearhackathon.com/HACKATHON.md>
- NEAR docs (via Context7 `/near/docs`): quickstart / toolchain, deploy with init call, testnet faucet accounts, Global Contracts (CLI + cost), storage staking, NEAR Connect wallet login
- near-sdk-rs (via Context7 `/near/near-sdk-rs` + source at tag `near-sdk-v5.30.1`): `Promise::{create_account, transfer, use_global_contract, function_call, then, and}`, `env::{ed25519_verify, block_timestamp, storage_byte_cost}`, `is_promise_success`, `#[private]`, `#[payable]`, `#[init]`, `PanicOnDefault`, `examples/factory-contract-global`
- OutLayer: <https://app.outlayer.ai/docs/getting-started> (NEAR integration, HTTPS API, signing keys, secrets, TEE attestation, payment keys), <https://github.com/out-layer/outlayer> (`wasi-examples/signing-key-probe`, `wasi-examples/CONNECTOR_MANIFEST.md`, `worker/wit/deps/signing-keys.wit`)
- Indexing: <https://docs.near.org/data-infrastructure/indexers>, <https://docs.fastnear.com/tx>, <https://api-testnet.nearblocks.io/api-docs/migration>
- Versions: crates.io API (near-sdk 5.30.1, near-contract-standards 5.30.1, cargo-near 0.23.0, near-workspaces 0.24.0), GitHub releases (near-cli-rs v0.31.1), npm registry (near-api-js 7.4.0, @hot-labs/near-connect 0.12.0, borsh 2.0.0)
