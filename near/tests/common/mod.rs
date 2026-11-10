//! Shared sandbox helpers: an estate with a known attestor key, signed attestations, deposits
//! and mock USDC. Rebuild the contracts first, or the tests run old code.
#![allow(dead_code)]

use anyhow::Context;
use base64::Engine;
use ed25519_dalek::Signer;
use near_workspaces::result::ExecutionFinalResult;
use near_workspaces::types::NearToken;
use near_workspaces::{Account, AccountId, Contract, Worker, network::Sandbox};
use serde_json::{Value, json};

pub const ATTESTOR_SECRET: [u8; 32] = [7; 32];
pub const GRANTOR_FACE: [u8; 32] = [1; 32];
pub const LIFETIME_NS: u64 = 15 * 60 * 1_000_000_000;
pub const ACTION_CHECKIN: u8 = 0;
pub const ACTION_CLAIM: u8 = 1;
pub const ACTION_WITHDRAW: u8 = 2;
/// 1M mUSDC at 6 decimals.
pub const SUPPLY: u128 = 1_000_000_000_000;
/// Gas an heir attaches to `claim`. NEAR reserves attached gas up front at a pessimistic price
/// (about 0.1 NEAR per 100 Tgas, on every network), and an heir's account holds its 0.1 NEAR seed.
pub const CLAIM_GAS_TGAS: u64 = 50;
/// What 1 byte of storage stakes: 0.00001 NEAR.
pub const BYTE_COST: u128 = 10_000_000_000_000_000_000;

pub struct Setup {
    pub worker: Worker<Sandbox>,
    pub estate: Contract,
    pub grantor: Account,
}

pub fn wasm(name: &str) -> anyhow::Result<Vec<u8>> {
    let path = format!("../target/near/{name}/{name}.wasm");
    std::fs::read(&path).with_context(|| format!("{path} is missing; run cargo near build first"))
}

/// A fresh estate with the shortest timers: 120 s interval, 60 s grace.
pub async fn setup() -> anyhow::Result<Setup> {
    let worker = near_workspaces::sandbox().await?;
    let estate = worker.dev_deploy(&wasm("herit_estate")?).await?;
    let factory = worker.dev_create_account().await?;
    let grantor = worker.dev_create_account().await?;
    let pk = ed25519_dalek::SigningKey::from_bytes(&ATTESTOR_SECRET)
        .verifying_key()
        .to_bytes();
    factory
        .call(estate.id(), "new")
        .args_json(json!({
            "grantor": grantor.id(),
            "attestor_pk": format!("ed25519:{}", bs58::encode(pk).into_string()),
            "check_in_interval_sec": 120,
            "grace_sec": 60,
        }))
        .transact()
        .await?
        .into_result()?;
    Ok(Setup {
        worker,
        estate,
        grantor,
    })
}

/// Must match the contract's `SignedPayload` field order and types exactly: Borsh is positional.
#[derive(borsh::BorshSerialize)]
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

/// What one attestation says.
pub struct Att<'a> {
    pub subject: &'a AccountId,
    pub action: u8,
    pub heir_label: Option<&'a str>,
    pub face: [u8; 32],
    pub nonce: u64,
    pub key: [u8; 32],
}

/// `{attestation, signature}`, valid from the sandbox's current block time.
pub async fn sign(s: &Setup, a: Att<'_>) -> anyhow::Result<Value> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let expiry_ns = s.worker.view_block().await?.timestamp() + LIFETIME_NS;
    let msg = borsh::to_vec(&SignedPayload {
        domain: "herit-near:v1".into(),
        estate_id: s.estate.id().to_string(),
        subject: a.subject.to_string(),
        action: a.action,
        heir_label: a.heir_label.map(Into::into),
        commitment: a.face,
        nonce: a.nonce,
        expiry_ns,
    })?;
    let sig = ed25519_dalek::SigningKey::from_bytes(&a.key)
        .sign(&msg)
        .to_bytes();
    Ok(json!({
        "attestation": {
            "action": a.action,
            "heir_label": a.heir_label,
            "commitment": b64.encode(a.face),
            "nonce": a.nonce.to_string(),
            "expiry_ns": expiry_ns.to_string(),
        },
        "signature": b64.encode(sig),
    }))
}

/// The grantor's first check-in (nonce 1): binds their face and starts the clock.
pub async fn check_in(s: &Setup) -> anyhow::Result<()> {
    let args = sign(
        s,
        Att {
            subject: s.grantor.id(),
            action: ACTION_CHECKIN,
            heir_label: None,
            face: GRANTOR_FACE,
            nonce: 1,
            key: ATTESTOR_SECRET,
        },
    )
    .await?;
    s.grantor
        .call(s.estate.id(), "check_in")
        .args_json(args)
        .transact()
        .await?
        .into_result()?;
    Ok(())
}

pub async fn deposit(s: &Setup, amount: NearToken) -> anyhow::Result<()> {
    s.grantor
        .call(s.estate.id(), "deposit")
        .deposit(amount)
        .transact()
        .await?
        .into_result()?;
    Ok(())
}

/// Mock USDC with the whole supply minted to the grantor, added to the estate.
pub async fn add_mock_usdc(s: &Setup) -> anyhow::Result<Contract> {
    let token = s.worker.dev_deploy(&wasm("mock_ft")?).await?;
    token
        .call("new_default_meta")
        .args_json(json!({ "owner_id": s.grantor.id(), "total_supply": SUPPLY.to_string() }))
        .transact()
        .await?
        .into_result()?;
    s.grantor
        .call(s.estate.id(), "add_token")
        .args_json(json!({ "token": token.id() }))
        .deposit(NearToken::from_millinear(50))
        .max_gas()
        .transact()
        .await?
        .into_result()?;
    Ok(token)
}

pub async fn deposit_token(s: &Setup, token: &Contract, amount: u128) -> anyhow::Result<()> {
    s.grantor
        .call(token.id(), "ft_transfer_call")
        .args_json(json!({ "receiver_id": s.estate.id(), "amount": amount.to_string(), "msg": "" }))
        .deposit(NearToken::from_yoctonear(1))
        .max_gas()
        .transact()
        .await?
        .into_result()?;
    Ok(())
}

/// A `U128` view of the estate, such as `balance_of` or `snapshot_of`, for `asset` (`null` = NEAR).
pub async fn amount_of(s: &Setup, view: &str, asset: Value) -> anyhow::Result<u128> {
    let v: String = s
        .estate
        .view(view)
        .args_json(json!({ "asset": asset }))
        .await?
        .json()?;
    Ok(v.parse()?)
}

/// The estate's balance less what its storage locks up: what it can actually pay out.
pub async fn free_near(s: &Setup) -> anyhow::Result<u128> {
    let a = s.worker.view_account(s.estate.id()).await?;
    Ok(a.balance.as_yoctonear() - a.storage_usage as u128 * BYTE_COST)
}

/// Fast-forwards until the estate's clock says Unlocked, without poking.
pub async fn wait_for_unlock(s: &Setup) -> anyhow::Result<()> {
    for _ in 0..30 {
        s.worker.fast_forward(100).await?;
        let status: String = s.estate.view("status_of").await?.json()?;
        if status == "Unlocked" {
            return Ok(());
        }
    }
    anyhow::bail!("the estate never unlocked")
}

pub fn failure_contains(result: ExecutionFinalResult, text: &str) -> bool {
    result.is_failure() && format!("{:?}", result.into_result().unwrap_err()).contains(text)
}

/// The grantor adds `label` with `bps`; returns the heir's account, signing with the heir's own
/// key (which the heir generated; the grantor only saw the public half).
pub async fn add_heir(s: &Setup, label: &str, bps: u16) -> anyhow::Result<Account> {
    use near_workspaces::types::{KeyType, SecretKey};
    let key = SecretKey::from_random(KeyType::ED25519);
    let created: bool = s
        .grantor
        .call(s.estate.id(), "add_heir")
        .args_json(json!({
            "label": label,
            "public_key": key.public_key(),
            "relationship": label,
            "default_bps": bps,
        }))
        .deposit(NearToken::from_near(1))
        .max_gas()
        .transact()
        .await?
        .into_result()?
        .json()?;
    anyhow::ensure!(created, "{label} was not created");
    let id: AccountId = format!("{label}.{}", s.estate.id()).parse()?;
    Ok(Account::from_secret_key(id, key, &s.worker))
}

/// `heir` claims `label` with `face`, attaching enough for the commitment record.
pub async fn claim(
    s: &Setup,
    heir: &Account,
    label: &str,
    face: [u8; 32],
    nonce: u64,
) -> anyhow::Result<ExecutionFinalResult> {
    let args = sign(
        s,
        Att {
            subject: heir.id(),
            action: ACTION_CLAIM,
            heir_label: Some(label),
            face,
            nonce,
            key: ATTESTOR_SECRET,
        },
    )
    .await?;
    let mut args = args;
    args["label"] = json!(label);
    Ok(heir
        .call(s.estate.id(), "claim")
        .args_json(args)
        .deposit(NearToken::from_millinear(10))
        // Not max_gas: an heir's account holds only its 0.1 NEAR seed, and the signer must be able
        // to prepay all the gas it attaches.
        .gas(near_workspaces::types::Gas::from_tgas(CLAIM_GAS_TGAS))
        .transact()
        .await?)
}

/// The grantor checks in with a fresh attestation (`nonce`). Returns the outcome.
pub async fn check_in_with(s: &Setup, nonce: u64) -> anyhow::Result<ExecutionFinalResult> {
    let args = sign(
        s,
        Att {
            subject: s.grantor.id(),
            action: ACTION_CHECKIN,
            heir_label: None,
            face: GRANTOR_FACE,
            nonce,
            key: ATTESTOR_SECRET,
        },
    )
    .await?;
    Ok(s.grantor
        .call(s.estate.id(), "check_in")
        .args_json(args)
        .transact()
        .await?)
}

/// The grantor withdraws with a fresh Selfie Check attestation (`nonce`).
pub async fn withdraw(
    s: &Setup,
    asset: Value,
    amount: u128,
    nonce: u64,
) -> anyhow::Result<ExecutionFinalResult> {
    let mut args = sign(
        s,
        Att {
            subject: s.grantor.id(),
            action: ACTION_WITHDRAW,
            heir_label: None,
            face: GRANTOR_FACE,
            nonce,
            key: ATTESTOR_SECRET,
        },
    )
    .await?;
    args["asset"] = asset;
    args["amount"] = json!(amount.to_string());
    Ok(s.grantor
        .call(s.estate.id(), "withdraw")
        .args_json(args)
        .deposit(NearToken::from_yoctonear(1))
        .max_gas()
        .transact()
        .await?)
}

/// The estate's status, as the clock computes it.
pub async fn status(s: &Setup) -> anyhow::Result<String> {
    Ok(s.estate.view("status_of").await?.json()?)
}

/// Fast-forwards in small steps until the clock says `want`.
pub async fn wait_for_status(s: &Setup, want: &str) -> anyhow::Result<()> {
    for _ in 0..200 {
        if status(s).await? == want {
            return Ok(());
        }
        s.worker.fast_forward(5).await?;
    }
    anyhow::bail!("the estate never reached {want}")
}

/// The Herit `event` names in a transaction's logs, in order. Other standards' events, such as
/// the token's own NEP-141 `ft_transfer`, are left out.
pub fn events(result: &ExecutionFinalResult) -> Vec<String> {
    result
        .logs()
        .iter()
        .filter_map(|l| l.strip_prefix("EVENT_JSON:"))
        .filter_map(|j| serde_json::from_str::<Value>(j).ok())
        .filter(|v| v["standard"] == "herit")
        .filter_map(|v| v["event"].as_str().map(String::from))
        .collect()
}

/// The IPFS CID of each credential state's image (`near/assets/credential/README.md`).
pub fn credential_cid(state: &str) -> &'static str {
    match state {
        "locked" => "bafkreibqatv5ibnvmkutd3mt3nf3fhwp5ricnatupszbum3yrwkg6pi2k4",
        "grace" => "bafkreigv6cslco4b2wziwh3ul3blrkiloew2exxtrh6uudjmhfwjtckjhy",
        "claimable" => "bafkreianqa2t6ixipd3ipyyqatzvgiqy2swd2as5kxdaym2dqoqgqqxabm",
        "claimed" => "bafkreicd2b6s67zdyvediuh57nkbvgqyrq4qvwqg7iuhaizdgd3wanzo2e",
        other => panic!("no state {other}"),
    }
}

/// Reads `label`'s credential the way a wallet does and checks its image matches its state.
/// Returns the state.
pub async fn credential_state(s: &Setup, label: &str) -> anyhow::Result<String> {
    let token: Value = s
        .estate
        .view("nft_token")
        .args_json(json!({ "token_id": label }))
        .await?
        .json()?;
    let metadata = &token["metadata"];
    let extra: Value = serde_json::from_str(metadata["extra"].as_str().unwrap_or("{}"))?;
    let state = extra["state"].as_str().unwrap_or_default().to_string();
    anyhow::ensure!(
        metadata["media"] == credential_cid(&state),
        "{label}: {state} shows {}",
        metadata["media"]
    );
    anyhow::ensure!(
        metadata["description"]
            .as_str()
            .unwrap_or_default()
            .ends_with(&format!("· {state}")),
        "{label}: description {}",
        metadata["description"]
    );
    Ok(state)
}
