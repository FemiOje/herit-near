//! Check-in on a real local node: signed attestations, replay, and a lapse after fast-forward.
//! Rebuild herit-estate first, or this tests old code.

use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use borsh::BorshSerialize;
use ed25519_dalek::{Signer, SigningKey};
use near_workspaces::result::ExecutionFinalResult;
use near_workspaces::{Account, Contract, Worker, network::Sandbox};
use serde_json::json;

const ATTESTOR_SECRET: [u8; 32] = [7; 32];
const FACE: [u8; 32] = [1; 32];
const LIFETIME_NS: u64 = 15 * 60 * 1_000_000_000;

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

struct Setup {
    worker: Worker<Sandbox>,
    estate: Contract,
    grantor: Account,
}

async fn setup() -> anyhow::Result<Setup> {
    let path = "../target/near/herit_estate/herit_estate.wasm";
    let wasm = std::fs::read(path).with_context(|| format!("{path} is missing; build it first"))?;
    let worker = near_workspaces::sandbox().await?;
    let estate = worker.dev_deploy(&wasm).await?;
    let factory = worker.dev_create_account().await?;
    let grantor = worker.dev_create_account().await?;
    let pk = SigningKey::from_bytes(&ATTESTOR_SECRET)
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

/// Signs a check-in for `estate_id` and `subject`, valid from the sandbox's current block time.
async fn sign_check_in(
    worker: &Worker<Sandbox>,
    estate_id: &str,
    subject: &str,
    nonce: u64,
) -> anyhow::Result<serde_json::Value> {
    let now = worker.view_block().await?.timestamp();
    let expiry_ns = now + LIFETIME_NS;
    let msg = borsh::to_vec(&SignedPayload {
        domain: "herit-near:v1".into(),
        estate_id: estate_id.into(),
        subject: subject.into(),
        action: 0,
        heir_label: None,
        commitment: FACE,
        nonce,
        expiry_ns,
    })?;
    let sig = SigningKey::from_bytes(&ATTESTOR_SECRET)
        .sign(&msg)
        .to_bytes();
    Ok(json!({
        "attestation": {
            "action": 0,
            "heir_label": null,
            "commitment": B64.encode(FACE),
            "nonce": nonce.to_string(),
            "expiry_ns": expiry_ns.to_string(),
        },
        "signature": B64.encode(sig),
    }))
}

async fn check_in(s: &Setup, args: serde_json::Value) -> anyhow::Result<ExecutionFinalResult> {
    Ok(s.grantor
        .call(s.estate.id(), "check_in")
        .args_json(args)
        .max_gas()
        .transact()
        .await?)
}

fn failure_contains(result: ExecutionFinalResult, text: &str) -> bool {
    result.is_failure() && format!("{:?}", result.into_result().unwrap_err()).contains(text)
}

#[tokio::test]
async fn a_check_in_binds_the_face_and_starts_the_clock() -> anyhow::Result<()> {
    let s = setup().await?;
    let args = sign_check_in(
        &s.worker,
        s.estate.id().as_str(),
        s.grantor.id().as_str(),
        1,
    )
    .await?;
    let result = check_in(&s, args).await?;
    let logs = result.logs().join("\n");
    result.into_result()?;
    assert!(logs.contains(r#""event":"checked_in""#), "logs: {logs}");

    let deadlines: (String, String) = s.estate.view("deadlines_of").await?.json()?;
    assert_ne!(deadlines.0, "0");
    let used: bool = s
        .estate
        .view("nonce_used")
        .args_json(json!({ "nonce": "1" }))
        .await?
        .json()?;
    assert!(used);
    let face: Option<String> = s.estate.view("commitment_of").await?.json()?;
    assert_eq!(face, Some(B64.encode(FACE)));
    Ok(())
}

#[tokio::test]
async fn a_replayed_attestation_is_refused() -> anyhow::Result<()> {
    let s = setup().await?;
    let args = sign_check_in(
        &s.worker,
        s.estate.id().as_str(),
        s.grantor.id().as_str(),
        1,
    )
    .await?;
    check_in(&s, args.clone()).await?.into_result()?;
    assert!(failure_contains(check_in(&s, args).await?, "nonce used"));
    Ok(())
}

#[tokio::test]
async fn a_signature_for_another_estate_is_refused() -> anyhow::Result<()> {
    let s = setup().await?;
    let args = sign_check_in(&s.worker, "bob.test.near", s.grantor.id().as_str(), 1).await?;
    assert!(failure_contains(
        check_in(&s, args).await?,
        "invalid signature"
    ));
    // A failed transaction reverts, so the nonce is still free.
    let used: bool = s
        .estate
        .view("nonce_used")
        .args_json(json!({ "nonce": "1" }))
        .await?
        .json()?;
    assert!(!used);
    Ok(())
}

#[tokio::test]
async fn a_check_in_after_the_deadlines_is_refused() -> anyhow::Result<()> {
    let s = setup().await?;
    let args = sign_check_in(
        &s.worker,
        s.estate.id().as_str(),
        s.grantor.id().as_str(),
        1,
    )
    .await?;
    check_in(&s, args).await?.into_result()?;

    // 120 s interval + 60 s grace. Fast-forward until the clock says Unlocked, nobody poking.
    let mut status = String::new();
    for _ in 0..20 {
        s.worker.fast_forward(100).await?;
        status = s.estate.view("status_of").await?.json()?;
        if status == "Unlocked" {
            break;
        }
    }
    assert_eq!(status, "Unlocked");

    let args = sign_check_in(
        &s.worker,
        s.estate.id().as_str(),
        s.grantor.id().as_str(),
        2,
    )
    .await?;
    assert!(failure_contains(
        check_in(&s, args).await?,
        "estate unlocked"
    ));
    Ok(())
}
