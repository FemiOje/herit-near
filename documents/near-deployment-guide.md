# Herit on NEAR — track choice and deployment guide

> **Status:** plan + starter code. The Rust snippets below follow the near-sdk **5.30.1** API, checked against
> the tagged source on 2026-11-08, but they have **not been compiled** yet (this machine has no Rust
> toolchain). Treat them as a typed starting point, not finished contracts.
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
| **No git tags** | Every commit is ≤ 2026-09-18, which is before the announcement. Tag the current baseline: `git tag pre-hackathon 55115bf`. |
| The README describes ETHOnline/Sepolia only | Add a "Built at NEAR HACKATHON" section: what's new, the diff link, disclosure of the ETHOnline origin, tools used. |

---

## 2. Architecture: Herit (Sepolia) → Herit (NEAR)

### 2.1 Why NEAR fits Herit better than the original chain did

| Herit on Sepolia | Herit on NEAR | What gets better |
| --- | --- | --- |
| ENS hierarchy `herit.eth → alice.herit.eth → son.alice.herit.eth` in a separate registry | **Native account hierarchy**: `herit-xyz.testnet → alice.herit-xyz.testnet`. Heirs are **named accounts** (`son.testnet`). | Human-readable identity is built into the protocol, with no frozen-registry dependency. The Sepolia problem "names invisible in the ENS app" disappears. |
| `ROLE_HEIR_CLAIM` bit granted on an ENSv2 subname | **Soulbound dynamic NEP-171 heir credential** minted by the estate. Its metadata is computed live: `locked → claimable → claimed`. | Every NEAR wallet and explorer renders the credential, which is the core story for the NFT track. |
| Five contracts holding each other as `immutable`s, so rotating the attestor key means redeploying all five | **One estate contract per estate account**, deployed **keyless** (no access keys), from immutable Global Contract code | Immutability comes from NEAR's account model. There is no cross-contract trust ring, and estates are isolated from each other and sharded. |
| `HeritVault` with ETH + ERC20 | The estate account escrows **NEAR + NEP-141** tokens (e.g. USDC) | Stablecoins support the Real-World Money track. |
| EIP-712 + `ecrecover` attestation from the World ID backend | **ed25519 attestation** over a Borsh payload, checked with `env::ed25519_verify` | Same narrow-signer model and replay rules. Signing is simpler (`near-api-js` `KeyPair`). |
| `pokeExpiry()` is permissionless | `poke_expiry()` is permissionless | Unchanged. |
| The Graph subgraph | Contract views plus NEP-297 `EVENT_JSON` logs (indexable via NearBlocks/FastNEAR) | Fewer moving parts. |

**Aurora was considered and rejected.** It would let `forge script` run nearly as-is, but the ENS layer would not
exist there, World ID's router is not there, and the judges would see an EVM redeploy, which is the one thing
the rules say is not judged.

### 2.2 Contracts

```
herit-xyz.testnet                 ← herit-factory  (regular deploy; upgradable during the hackathon)
│   open_estate(label, ...)  ── creates ──►  alice.herit-xyz.testnet
│                                            └─ uses Global Contract <estate code hash>
│                                               herit-estate: state machine + vault + attestor + NEP-171
herit-code-xyz.testnet            ← publisher of the Global Contract (by hash ⇒ immutable code)
mock-usdc-xyz.testnet             ← NEP-141 test token for the demo (testnet)
```

**`herit-factory`**
- `new(estate_code_hash, attestor_pk)` sets up the factory.
- `open_estate(label, check_in_interval_sec, grace_sec)` is `#[payable]`. It creates the sub-account, funds its storage, attaches the global code, initialises it with `grantor = predecessor`, and indexes it. A failure callback refunds the deposit.
- Views: `estates_of(grantor)`, `estate_for_heir(heir)` (for the dashboard), `config()`.

**`herit-estate`** (one per estate; replaces `HeritRegistry` + `HeritVault` + `ClaimManager` + `LivenessAttestor` + `AccessControlGate`)
- Grantor-only, before unlock:
  - `add_heir(label, account_id, relationship, default_bps)` (payable; mints the credential)
  - `set_share(label, asset, bps)`, `add_token(token)` (payable; registers estate storage on the FT)
  - `deposit()` (payable NEAR), `ft_on_transfer` (NEP-141 deposits), `withdraw(asset, amount)`
- `check_in(attestation, signature)`: only the grantor, only a matching face commitment.
- `poke_expiry()`: permissionless. On the `Unlocked` transition it snapshots the escrow.
- `claim(label, attestation, signature)`: the heir account plus the heir's own Selfie Check. It pays each asset once, with **rollback callbacks**.
- NEP-171 views: `nft_metadata`, `nft_token`, `nft_tokens`, `nft_tokens_for_owner`, `nft_supply_for_owner`, `nft_total_supply`. `nft_transfer*` panics because the credential is soulbound.
- Views: `status_of`, `deadlines_of`, `heirs_of`, `share_of`, `claimable_of`, `escrow_of`, `snapshot_of`.

### 2.3 Work for the hackathon window (what the judges see as new)

1. NEAR-native estate factory with keyless, immutable estate accounts built from Global Contract code.
2. Dynamic, soulbound NEP-171 heir credentials whose state tracks the estate's liveness.
3. NEP-141 stablecoin inheritance with async-safe payouts.
4. Port of the World ID Selfie Check attestor to ed25519.
5. *(Stretch, strongly recommended: a small change with a large story)* **Liveness-gated withdrawals.** On Sepolia, a thief holding the grantor's key could still call `withdraw`. On NEAR, `withdraw` also requires a fresh check-in attestation, so a stolen key alone can move nothing. The original README's threat model ("the thief who stole her key") then holds end to end.
6. *(Stretch, only if days 1–4 land early)* Chain Signatures, so an estate can inherit a BTC/ETH address. Leave it out of scope unless time allows.

---

## 3. Toolchain (pinned versions, checked 2026-11-08)

| Tool | Version | Install |
| --- | --- | --- |
| Rust | ≥ 1.95 (the near-sdk examples use edition 2024) | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| wasm target | — | `rustup target add wasm32-unknown-unknown` |
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

---

## 4. Accounts

Pick a team suffix once, e.g. `xyz`. Testnet named accounts are first-come.

```bash
export NET=testnet
export FACTORY=herit-xyz.testnet          # factory contract; estates become <label>.herit-xyz.testnet
export CODE=herit-code-xyz.testnet        # publishes the global estate code
export TOKEN=mock-usdc-xyz.testnet        # NEP-141 test token
export GRANTOR=alice-xyz.testnet          # demo grantor
export HEIR1=son-xyz.testnet              # demo heirs
export HEIR2=daughter-xyz.testnet

for A in $FACTORY $CODE $TOKEN $GRANTOR $HEIR1 $HEIR2; do
  near account create-account sponsor-by-faucet-service "$A" \
    autogenerate-new-keypair save-to-keychain network-config $NET create
done

near account view-account-summary $CODE network-config $NET now
```

**Budget the Global Contract.** Publishing a Global Contract **burns 10 NEAR per 100 KB** of wasm. That is 10× the normal storage
rate, and the NEAR is not recoverable. An estate contract of about 200 KB therefore burns about **20 NEAR**, which is more than one faucet
drop. Build first (§6.1), read the wasm size, then top up `$CODE` from the other faucet accounts:

```bash
near tokens $HEIR1 send-near $CODE '8 NEAR' network-config $NET sign-with-keychain send
```

If the budget doesn't work, use the fallback in §6.4: a regular per-estate deploy, which locks 1 NEAR/100 KB per estate and is refundable.

**Mainnet payout account** (needed for the form, not for the build): create one in a wallet such as HOT, Meteor or Intear. Write it down and never
reuse it as a deployer.

---

## 5. Workspace layout

Keep the Foundry project untouched (it is the baseline). Add a sibling Cargo workspace:

```
near/
├── Cargo.toml                 # [workspace]
├── contracts/
│   ├── herit-estate/          # Cargo.toml, src/lib.rs
│   ├── herit-factory/
│   └── mock-ft/               # copy of near-sdk-rs/examples/fungible-token, renamed
├── tests/                     # near-workspaces integration tests
│   └── lifecycle.rs
└── scripts/
    ├── code-hash.py           # base58(sha256(wasm)) for use_global_contract
    └── sign-attestation.mjs   # DEV ONLY: mints test attestations with the attestor key
```

`near/Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["contracts/herit-estate", "contracts/herit-factory", "contracts/mock-ft"]

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
near-workspaces = "0.24.0"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
serde_json = "1"

[package.metadata.near.reproducible_build]
# Copy the current image + digest from near-sdk-rs/examples/*/Cargo.toml at tag near-sdk-v5.30.1
# (or run `cargo near new` once and copy its block). Pin the digest; the tag is descriptive only.
image = "sourcescan/cargo-near:<version>-rust-<version>"
image_digest = "sha256:<digest>"
passed_env = []
container_build_command = ["cargo", "near", "build", "non-reproducible-wasm", "--locked"]
```

`herit-factory/Cargo.toml` is the same without `near-contract-standards`.

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
    NearToken, PanicOnDefault, Promise, PromiseOrValue,
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
    last_check_in_ns: u64,
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
        // Same bounds as HeritRegistry: 2 min..90 days, 1 min..14 days.
        require!((120..=90 * 86_400).contains(&check_in_interval_sec), "bad interval");
        require!((60..=14 * 86_400).contains(&grace_sec), "bad grace");
        Self {
            factory: env::predecessor_account_id(),
            grantor,
            attestor_pk: decode_ed25519(&attestor_pk),
            grantor_commitment: None,
            check_in_interval_ns: check_in_interval_sec as u64 * 1_000_000_000,
            grace_ns: grace_sec as u64 * 1_000_000_000,
            last_check_in_ns: env::block_timestamp(),
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
        if s == Status::Unlocked && self.status != Status::Unlocked {
            for (asset, bal) in self.escrow.iter() {
                self.snapshot.insert(asset.clone(), *bal); // freeze at unlock, not at first claim
            }
            log_event("unlocked", &format!(r#"{{"heirs":{}}}"#, self.heirs.len()));
        }
        self.status = s;
        s
    }

    // ---------- grantor setup (before unlock) ----------

    #[payable] // attached NEAR pays the credential's storage; the surplus is refunded
    pub fn add_heir(&mut self, label: String, account_id: AccountId, relationship: String, default_bps: u16) {
        self.only_grantor();
        self.not_unlocked();
        require!(self.heirs.len() < MAX_HEIRS, "too many heirs");
        require!(!self.heirs.contains_key(&label), "label taken");
        let before = env::storage_usage();
        self.heirs.insert(label.clone(), Heir { account_id: account_id.clone(), relationship, default_bps });
        self.heirs.flush(); // make storage_usage() see the write
        require!(self.total_bps(&None) <= BPS, "shares exceed 100%");
        refund_storage_surplus(before);
        nft::emit_mint(&account_id, &label);
    }

    #[payable]
    pub fn deposit(&mut self) {
        self.only_grantor();
        self.not_unlocked();
        let amt = env::attached_deposit().as_yoctonear();
        require!(amt > 0, "zero amount");
        *self.escrow.entry(None).or_insert(0) += amt;
    }

    // add_token / set_share / withdraw / views follow the same pattern (see §6.5 checklist)

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

    /// NEAR has no cross-receipt revert: if the transfer fails, undo the `paid` mark here.
    #[private]
    pub fn on_payout(&mut self, label: String, asset: Asset, amount: U128) -> bool {
        if is_promise_success() {
            log_event("paid", &format!(r#"{{"label":"{label}","amount":"{}"}}"#, amount.0));
            true
        } else {
            self.paid.remove(&(label, asset));
            false
        }
    }

    // ---------- views ----------

    pub fn status_of(&self) -> Status { self.pending_status() }
    pub fn deadlines_of(&self) -> (U64, U64) {
        let g = self.last_check_in_ns + self.check_in_interval_ns;
        (U64(g), U64(g + self.grace_ns))
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

fn decode_ed25519(s: &str) -> [u8; 32] {
    let b58 = s.strip_prefix("ed25519:").expect("expected ed25519:<base58>");
    near_sdk::bs58::decode(b58).into_vec().expect("bad base58")
        .try_into().expect("ed25519 key must be 32 bytes")
}

fn refund_storage_surplus(before: u64) {
    let used = env::storage_usage().saturating_sub(before) as u128;
    let cost = env::storage_byte_cost().as_yoctonear() * used;
    let paid = env::attached_deposit().as_yoctonear();
    require!(paid >= cost, "attach more NEAR for storage");
    if paid > cost {
        Promise::new(env::predecessor_account_id()).transfer(NearToken::from_yoctonear(paid - cost)).detach();
    }
}

fn log_event(event: &str, data: &str) {
    env::log_str(&format!(r#"EVENT_JSON:{{"standard":"herit","version":"1.0.0","event":"{event}","data":[{data}]}}"#));
}
```

> If `.detach()` doesn't exist on `Promise` in your resolved version, drop it: a `Promise` that is not
> returned is still scheduled when it is dropped at the end of the call.

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
                title: Some(format!("{token_id}.{}", env::current_account_id())),
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

`TokenMetadata` has a fixed field list. If 5.30.1 adds or renames a field, the compiler will name it; adjust then.

### 6.3 `herit-factory`

```rust
// near/contracts/herit-factory/src/lib.rs
use near_sdk::store::LookupMap;
use near_sdk::{env, is_promise_success, near, require, AccountId, BorshStorageKey, Gas, NearToken, PanicOnDefault, Promise};

const MIN_ESTATE_DEPOSIT: NearToken = NearToken::from_millinear(500); // tune after measuring storage
const INIT_GAS: Gas = Gas::from_tgas(30);

#[derive(BorshStorageKey)]
#[near]
enum Key { Estates }

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Factory {
    estate_code_hash: [u8; 32],
    attestor_pk: String, // "ed25519:<base58>", forwarded to each estate
    estates: LookupMap<AccountId, Vec<AccountId>>, // grantor -> estates
}

#[near]
impl Factory {
    #[init]
    pub fn new(estate_code_hash: String, attestor_pk: String) -> Self {
        let h: [u8; 32] = near_sdk::bs58::decode(&estate_code_hash).into_vec().expect("bad base58")
            .try_into().expect("hash must be 32 bytes");
        Self { estate_code_hash: h, attestor_pk, estates: LookupMap::new(Key::Estates) }
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
            .function_call("new".into(), args.to_string().into_bytes(), NearToken::from_near(0), INIT_GAS)
            .then(Self::ext(env::current_account_id()).with_static_gas(Gas::from_tgas(10))
                .on_estate_created(estate, grantor, deposit))
    }

    #[private]
    pub fn on_estate_created(&mut self, estate: AccountId, grantor: AccountId, deposit: NearToken) -> bool {
        if is_promise_success() {
            let mut list = self.estates.get(&grantor).cloned().unwrap_or_default();
            list.push(estate);
            self.estates.insert(grantor, list);
            true
        } else {
            // A failed batch refunds the attached NEAR to the factory, so forward it back to the grantor.
            Promise::new(grantor).transfer(deposit);
            false
        }
    }

    pub fn estates_of(&self, grantor: AccountId) -> Vec<AccountId> {
        self.estates.get(&grantor).cloned().unwrap_or_default()
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

- [ ] `add_token(token)` (payable): whitelist ≤ `MAX_TOKENS`, then `ext_ft::ext(token).with_attached_deposit(0.00125 NEAR).storage_deposit(Some(current), Some(true))`
- [ ] `set_share(label, asset, bps)`: grantor, not unlocked, and `total_bps(asset) <= 10_000` (= `HeritRegistry__ShareOverflow`)
- [ ] `withdraw(asset, amount, attestation, signature)`: grantor, not unlocked, **`ACTION_WITHDRAW` attestation with the bound commitment** (new NEAR feature, §2.3 item 5). Debit before the promise and re-credit in a `#[private]` callback on failure.
- [ ] `remove_heir(label)` before unlock, plus `NftBurn` event
- [ ] Views: `heirs_of`, `share_of`, `escrow_of`, `snapshot_of`, `grantor_of`, `config_of`, `nft_tokens`, `nft_total_supply`, `nft_supply_for_owner`
- [ ] Unit tests (`near_sdk::test_utils::VMContextBuilder`) for: bound-commitment mismatch, nonce replay, expiry > 30 min, check-in after an unpoked lapse is refused, grantor commitment cannot claim, a commitment cannot claim two labels, rounding-down payout, double claim refused

---

## 7. Backend: port the attestation signer

The World ID flow (`app/api/worldid/sign`, `/verify`) is unchanged up to the point where the verified proof is
turned into an attestation. Swap only the EIP-712/secp256k1 signing for ed25519 over the Borsh payload.

**Generate the attestor key** (once, offline; it never needs NEAR or gas):

```bash
cd frontend && npm i near-api-js@7.4.0 borsh@2.0.0 @hot-labs/near-connect@0.12.0
node -e 'const {KeyPair}=require("near-api-js");const k=KeyPair.fromRandom("ed25519");console.log("ATTESTOR_SECRET_KEY="+k.toString());console.log("ATTESTOR_PUBLIC_KEY="+k.getPublicKey().toString())'
```

Put `ATTESTOR_SECRET_KEY` in `frontend/.env.local` (server-only; never `NEXT_PUBLIC_`). `ATTESTOR_PUBLIC_KEY` is the
`attestor_pk` you pass to `Factory::new`.

**Sign** (in the verify route, after Cloud Verify succeeds):

```ts
import { KeyPair } from "near-api-js";
import { serialize } from "borsh";

// Must match `SignedPayload` field ORDER and TYPES exactly; Borsh is positional.
const SCHEMA = {
  struct: {
    domain: "string",
    estate_id: "string",       // AccountId borsh-serialises as a string
    subject: "string",
    action: "u8",              // 0 checkin, 1 claim, 2 withdraw
    heir_label: { option: "string" },
    commitment: { array: { type: "u8", len: 32 } },
    nonce: "u64",
    expiry_ns: "u64",
  },
} as const;

export function signAttestation(p: {
  estateId: string; subject: string; action: 0 | 1 | 2;
  heirLabel: string | null; commitment: Uint8Array; nonce: bigint;
}) {
  const expiry_ns = BigInt(Date.now() + 20 * 60_000) * 1_000_000n; // < 30 min cap
  const bytes = serialize(SCHEMA, {
    domain: "herit-near:v1", estate_id: p.estateId, subject: p.subject, action: p.action,
    heir_label: p.heirLabel, commitment: p.commitment, nonce: p.nonce, expiry_ns,
  });
  const kp = KeyPair.fromString(process.env.ATTESTOR_SECRET_KEY as `ed25519:${string}`);
  const { signature } = kp.sign(bytes);
  return {
    attestation: {
      action: p.action,
      heir_label: p.heirLabel,
      commitment: Buffer.from(p.commitment).toString("base64"),
      nonce: p.nonce.toString(),         // U64 travels as a string
      expiry_ns: expiry_ns.toString(),
    },
    signature: Buffer.from(signature).toString("base64"),
  };
}
```

`subject` must be the NEAR account that will **send** the transaction: the grantor for check-in, the heir for claim.
Keep the existing commitment derivation (salted nullifier → 32 bytes). Nonces must be unique per estate; a
random `u64` is fine.

**Write a Borsh golden test before anything else.** Have one Rust unit test print `hex(borsh::to_vec(&payload))`
for fixed inputs, and one Node script `serialize` the same inputs. Diff the two outputs. Most "invalid signature"
errors in ports like this come from a field-order mismatch here.

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
the thief's commitment is refused.

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
near contract deploy $FACTORY use-file near/target/near/herit_factory/herit_factory.wasm \
  with-init-call new json-args "{\"estate_code_hash\":\"$ESTATE_HASH\",\"attestor_pk\":\"$ATTESTOR_PUBLIC_KEY\"}" \
  prepaid-gas '30 Tgas' attached-deposit '0 NEAR' network-config $NET sign-with-keychain send
```

### 8.4 Smoke test with the CLI (this is also the judge-reproducible path)

```bash
call() { near contract call-function as-transaction "$1" "$2" json-args "$3" prepaid-gas "${5:-100 Tgas}" \
         attached-deposit "${4:-0 NEAR}" sign-as "$6" network-config $NET sign-with-keychain send; }
view() { near contract call-function as-read-only "$1" "$2" json-args "${3:-{\}}" network-config $NET now; }
ESTATE=alice.$FACTORY

call $FACTORY open_estate '{"label":"alice","check_in_interval_sec":120,"grace_sec":60}' '1 NEAR' '' $GRANTOR
call $ESTATE add_heir "{\"label\":\"son\",\"account_id\":\"$HEIR1\",\"relationship\":\"son\",\"default_bps\":6000}" '0.05 NEAR' '' $GRANTOR
call $ESTATE add_heir "{\"label\":\"daughter\",\"account_id\":\"$HEIR2\",\"relationship\":\"daughter\",\"default_bps\":4000}" '0.05 NEAR' '' $GRANTOR
call $ESTATE deposit '{}' '1 NEAR' '' $GRANTOR
view $ESTATE nft_tokens_for_owner "{\"account_id\":\"$HEIR1\"}"     # state: "locked"

# check in: DEV attestation from scripts/sign-attestation.mjs (wraps §7's signAttestation)
ATT=$(node near/scripts/sign-attestation.mjs --estate $ESTATE --subject $GRANTOR --action 0)
call $ESTATE check_in "$ATT" '0 NEAR' '' $GRANTOR

sleep 190; call $ESTATE poke_expiry '{}' '0 NEAR' '' $HEIR1          # anyone can poke
view $ESTATE status_of                                              # "Unlocked"
ATT=$(node near/scripts/sign-attestation.mjs --estate $ESTATE --subject $HEIR1 --action 1 --label son)
call $ESTATE claim "$(echo "$ATT" | jq '. + {label:"son"}')" '0 NEAR' '200 Tgas' $HEIR1
```

Inspect everything on <https://testnet.nearblocks.io/address/alice.herit-xyz.testnet>. Paste these exact
transaction links into the README's "Proof it ran" section, as the Sepolia README does today.

---

## 9. Frontend changes (Next.js 16.3.4)

Read `frontend/node_modules/next/dist/docs/` first, per `frontend/AGENTS.md`.

| Replace | With |
| --- | --- |
| `wagmi` / `viem` providers | `NearConnector` from `@hot-labs/near-connect` (`connector.connect()`, then `wallet.signAndSendTransaction({ receiverId, actions:[{ type:"FunctionCall", params:{ methodName, args, gas, deposit }}]})`) |
| `useReadContract` | `new JsonRpcProvider({ url: "https://test.rpc.fastnear.com" }).callFunction({ contractId, method, args })` from `near-api-js` |
| `@ensdomains/ensjs` name resolution | Native NEAR account ids (validate with `/^(([a-z\d]+[-_])*[a-z\d]+\.)*([a-z\d]+[-_])*[a-z\d]+$/`, 2–64 chars) |
| Subgraph activity feed | Estate views + NearBlocks API for the tx list (or drop it for the demo) |
| Sepolia addresses in `lib/` | `NEXT_PUBLIC_NEAR_NETWORK`, `NEXT_PUBLIC_HERIT_FACTORY` |

Gas for `claim` should be `"200000000000000"` (200 Tgas). Amounts are yoctoNEAR **strings**.

---

## 10. NEAR gotchas that bite Solidity developers

1. **Async receipts, no global revert.** A failed `ft_transfer` or `transfer` does not undo your state. Every outbound payment needs a `#[private]` callback that rolls back (`on_payout` above). Write state *before* scheduling the promise.
2. **Time is nanoseconds.** `env::block_timestamp()` is `u64` ns. Convert seconds × 1e9 at the boundary only.
3. **u64/u128 in JSON are strings.** Use `U64` and `U128` in every public signature; JS loses precision above 2^53.
4. **Storage staking.** Every byte stored locks 1e19 yocto (1 NEAR/100 KB) from the contract's own balance. Make callers pay (`#[payable]` + refund the surplus). **Never** use `env::account_balance()` as escrow; it includes the locked storage stake. Keep explicit `escrow` accounting.
5. **Methods aren't payable by default.** Attaching NEAR to a non-`#[payable]` method panics.
6. **Deploy + init in one transaction.** An `#[init]` left uncalled can be front-run by anyone.
7. **300 Tgas per transaction, total.** `claim` fans out one transfer plus one callback per asset (~25 Tgas each). `MAX_TOKENS = 5` keeps it safely under 300 Tgas.
8. **NEP-141 storage registration.** The estate must be `storage_deposit`-registered on each token before it can receive it, and the heir must be registered before `ft_transfer`. Unregistered heirs make the payout fail (and the callback rolls it back), so the frontend should register the heir first (`storage_deposit` from the heir, ~0.00125 NEAR).
9. **`ft_transfer` needs exactly 1 yoctoNEAR attached** (the security pattern that forces a full-access-key signature).
10. **Keyless means final.** Estates have no access keys, so code bugs are permanent per estate. This is the same trade-off as the Sepolia `immutable`s. Keep the **factory** keyed during the hackathon so you can redeploy it, and publish a fresh global hash for each estate-code fix. Old estates keep the old code.
11. **Global by hash vs by account.** By hash = immutable (chosen here, matching Herit's trust model). By account = the publisher can upgrade every estate at once, which reintroduces a privileged key. Say this explicitly in the README; judges care.
12. **Account id rules.** Lowercase `[a-z0-9_-]`, segments separated by `.`, 2–64 chars total. `label + "." + factory` must fit.

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
```

Add a second CI job to `.github/workflows/test.yml`: `rustup target add wasm32-unknown-unknown`, install cargo-near, run `cargo near build non-reproducible-wasm` for each contract, then `cargo test`. Leave the Foundry job as it is.

---

## 12. Six-day plan (today is Sun 8 Nov)

| Day | Goal | Done when |
| --- | --- | --- |
| **Sun 8** | Tag `pre-hackathon`, add LICENSE, toolchain, accounts, `near/` scaffold, Borsh golden test | `cargo near build` produces a wasm, and the Rust and JS payload bytes match |
| **Mon 9** | Estate core: state machine, `verify`, check-in, poke, NEAR escrow, unit tests | Sepolia rule parity: the 8 unit tests in §6.5 pass |
| **Tue 10** | NFT credential, NEP-141 path, claim + rollback callbacks, liveness-gated withdraw, sandbox lifecycle test | `cargo test --test lifecycle` is green |
| **Wed 11** | Factory, Global Contract publish, testnet deploy, CLI smoke run (§8.4) | Testnet NearBlocks links for every step |
| **Thu 12** | Backend signer port, frontend on near-connect, deploy to Vercel | Grantor and heir flows work in the browser with a real Selfie Check |
| **Fri 13** | README "Built at NEAR HACKATHON", 2-min video, tag `hackathon-submission` | A fresh clone reproduces the demo from the README alone |
| **Sat 14** | X post, form submission **by 12:00 UTC** | Confirmation received. Leave 12 h of buffer before 23:59 UTC. |

Cut order if you fall behind: Chain Signatures (already out) → NEP-141 (NEAR-only demo) → liveness-gated withdraw.
Never cut the NFT credential: it **is** the track.

---

## 13. Submission checklist

- [ ] `git tag pre-hackathon 55115bf` and `git tag hackathon-submission <final>`, both pushed. Include a README link to `compare/pre-hackathon...hackathon-submission`.
- [ ] `LICENSE` (MIT) at the repo root
- [ ] README top section "**Built at NEAR HACKATHON**":
  - what's new
  - disclosure: "Herit first won at ETHOnline 2026 on Sepolia. Only the NEAR integration is submitted for judging."
  - libraries and AI tools used (Claude Code, near-sdk, near-contract-standards, near-api-js, near-connect, borsh)
  - testnet account links
  - a "do X → see Y" reproduction script (§8.4)
- [ ] Repo is **public**
- [ ] Hosted demo live until ≥ 25 Nov, testnet accounts funded
- [ ] Public X post from your own account: the user problem first, one complete moment (heir credential flipping `locked → claimable`, then a claim), NEAR's concrete role. Tag **@NEARProtocol** and **#NEARHackathon** (optionally @NearLegion and @NearBuilders), with the video attached.
- [ ] Form at <https://nearhackathon.com/#apply>: tracks **`nfts`, `realworld`, `open`**, mainnet payout account, repo link, X post link, 20–2,000-char description. **Submit once.** Make later edits via `/request-changes`.

---

## Sources

- NEAR HACKATHON: <https://nearhackathon.com>, <https://nearhackathon.com/HACKATHON.md>
- NEAR docs (via Context7 `/near/docs`): quickstart / toolchain, deploy with init call, testnet faucet accounts, Global Contracts (CLI + cost), storage staking, NEAR Connect wallet login
- near-sdk-rs (via Context7 `/near/near-sdk-rs` + source at tag `near-sdk-v5.30.1`): `Promise::{create_account, transfer, use_global_contract, function_call, then, and}`, `env::{ed25519_verify, block_timestamp, storage_byte_cost}`, `is_promise_success`, `#[private]`, `#[payable]`, `#[init]`, `PanicOnDefault`, `examples/factory-contract-global`
- Versions: crates.io API (near-sdk 5.30.1, near-contract-standards 5.30.1, cargo-near 0.23.0, near-workspaces 0.24.0), GitHub releases (near-cli-rs v0.31.1), npm registry (near-api-js 7.4.0, @hot-labs/near-connect 0.12.0, borsh 2.0.0)
