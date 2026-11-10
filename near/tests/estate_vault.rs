//! The vault on a real local node. Rebuild herit-estate first, or this tests old code.

use anyhow::Context;
use near_workspaces::result::ExecutionFinalResult;
use near_workspaces::types::NearToken;
use near_workspaces::{Account, Contract, Worker, network::Sandbox};
use serde_json::json;

const ATTESTOR_SECRET: [u8; 32] = [7; 32];
const FACE: [u8; 32] = [1; 32];
const LIFETIME_NS: u64 = 15 * 60 * 1_000_000_000;

fn attestor_pk() -> String {
    let pk = ed25519_dalek::SigningKey::from_bytes(&ATTESTOR_SECRET)
        .verifying_key()
        .to_bytes();
    format!("ed25519:{}", bs58::encode(pk).into_string())
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

/// `{attestation, signature}` for the grantor's face, valid from the sandbox's block time.
async fn sign(
    s: &Setup,
    action: u8,
    nonce: u64,
    key: [u8; 32],
) -> anyhow::Result<serde_json::Value> {
    use base64::Engine;
    use ed25519_dalek::Signer;
    let b64 = base64::engine::general_purpose::STANDARD;
    let expiry_ns = s.worker.view_block().await?.timestamp() + LIFETIME_NS;
    let msg = borsh::to_vec(&SignedPayload {
        domain: "herit-near:v1".into(),
        estate_id: s.estate.id().to_string(),
        subject: s.grantor.id().to_string(),
        action,
        heir_label: None,
        commitment: FACE,
        nonce,
        expiry_ns,
    })?;
    let sig = ed25519_dalek::SigningKey::from_bytes(&key)
        .sign(&msg)
        .to_bytes();
    Ok(json!({
        "attestation": {
            "action": action,
            "heir_label": null,
            "commitment": b64.encode(FACE),
            "nonce": nonce.to_string(),
            "expiry_ns": expiry_ns.to_string(),
        },
        "signature": b64.encode(sig),
    }))
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
    factory
        .call(estate.id(), "new")
        .args_json(json!({
            "grantor": grantor.id(),
            "attestor_pk": attestor_pk(),
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

async fn deposit(
    s: &Setup,
    from: &Account,
    amount: NearToken,
) -> anyhow::Result<ExecutionFinalResult> {
    Ok(from
        .call(s.estate.id(), "deposit")
        .deposit(amount)
        .transact()
        .await?)
}

async fn balance_of_near(s: &Setup) -> anyhow::Result<u128> {
    let v: String = s
        .estate
        .view("balance_of")
        .args_json(json!({ "asset": null }))
        .await?
        .json()?;
    Ok(v.parse()?)
}

fn failure_contains(result: ExecutionFinalResult, text: &str) -> bool {
    result.is_failure() && format!("{:?}", result.into_result().unwrap_err()).contains(text)
}

// ---------- Part 1: NEAR deposits ----------

#[tokio::test]
async fn deposited_near_is_held_and_counted() -> anyhow::Result<()> {
    let s = setup().await?;
    let before = s.worker.view_account(s.estate.id()).await?;

    // The first deposit lists NEAR and pays for that storage out of the amount credited.
    deposit(&s, &s.grantor, NearToken::from_near(2))
        .await?
        .into_result()?;
    let first = balance_of_near(&s).await?;
    let listing = NearToken::from_near(2).as_yoctonear() - first;
    assert!(
        listing > 0 && listing < NearToken::from_millinear(10).as_yoctonear(),
        "{listing}"
    );

    // Later deposits are credited in full.
    deposit(&s, &s.grantor, NearToken::from_millinear(500))
        .await?
        .into_result()?;
    assert_eq!(
        balance_of_near(&s).await?,
        first + NearToken::from_millinear(500).as_yoctonear()
    );
    let tokens: Vec<Option<String>> = s.estate.view("tokens_of").await?.json()?;
    assert_eq!(tokens, vec![None]);

    // What was kept back is exactly what the estate's extra storage costs to stake.
    let after = s.worker.view_account(s.estate.id()).await?;
    let byte_cost = 10u128.pow(19);
    assert_eq!(
        (after.storage_usage - before.storage_usage) as u128 * byte_cost,
        listing
    );
    Ok(())
}

#[tokio::test]
async fn a_strangers_deposit_is_refused_and_returned() -> anyhow::Result<()> {
    let s = setup().await?;
    let stranger = s.worker.dev_create_account().await?;
    let before = s
        .worker
        .view_account(stranger.id())
        .await?
        .balance
        .as_yoctonear();

    let result = deposit(&s, &stranger, NearToken::from_near(1)).await?;
    assert!(failure_contains(result, "not the grantor"));

    assert_eq!(balance_of_near(&s).await?, 0);
    let lost = before
        - s.worker
            .view_account(stranger.id())
            .await?
            .balance
            .as_yoctonear();
    assert!(
        lost < NearToken::from_millinear(10).as_yoctonear(),
        "only gas is lost: {lost}"
    );
    Ok(())
}

// ---------- Part 2: token deposits ----------

const SUPPLY: u128 = 1_000_000_000_000; // 1M mUSDC at 6 decimals

/// Mock USDC with the whole supply minted to the grantor.
async fn mock_usdc(s: &Setup) -> anyhow::Result<Contract> {
    let path = "../target/near/mock_ft/mock_ft.wasm";
    let wasm = std::fs::read(path).with_context(|| format!("{path} is missing; build it first"))?;
    let token = s.worker.dev_deploy(&wasm).await?;
    token
        .call("new_default_meta")
        .args_json(json!({ "owner_id": s.grantor.id(), "total_supply": SUPPLY.to_string() }))
        .transact()
        .await?
        .into_result()?;
    Ok(token)
}

async fn add_token(
    s: &Setup,
    token: &near_workspaces::AccountId,
) -> anyhow::Result<ExecutionFinalResult> {
    Ok(s.grantor
        .call(s.estate.id(), "add_token")
        .args_json(json!({ "token": token }))
        .deposit(NearToken::from_millinear(50))
        .max_gas()
        .transact()
        .await?)
}

async fn ft_transfer_call(
    from: &Account,
    token: &Contract,
    to: &Contract,
    amount: u128,
) -> anyhow::Result<()> {
    from.call(token.id(), "ft_transfer_call")
        .args_json(json!({ "receiver_id": to.id(), "amount": amount.to_string(), "msg": "" }))
        .deposit(NearToken::from_yoctonear(1))
        .max_gas()
        .transact()
        .await?
        .into_result()?;
    Ok(())
}

async fn ft_balance(token: &Contract, of: &near_workspaces::AccountId) -> anyhow::Result<u128> {
    let v: String = token
        .view("ft_balance_of")
        .args_json(json!({ "account_id": of }))
        .await?
        .json()?;
    Ok(v.parse()?)
}

async fn balance_of_token(s: &Setup, token: &Contract) -> anyhow::Result<u128> {
    let v: String = s
        .estate
        .view("balance_of")
        .args_json(json!({ "asset": token.id() }))
        .await?
        .json()?;
    Ok(v.parse()?)
}

#[tokio::test]
async fn an_added_token_is_deposited_and_counted() -> anyhow::Result<()> {
    let s = setup().await?;
    let usdc = mock_usdc(&s).await?;

    let added: bool = add_token(&s, usdc.id()).await?.into_result()?.json()?;
    assert!(added);
    let registered: serde_json::Value = usdc
        .view("storage_balance_of")
        .args_json(json!({ "account_id": s.estate.id() }))
        .await?
        .json()?;
    assert!(
        !registered.is_null(),
        "the estate is registered with the token"
    );
    let tokens: Vec<Option<String>> = s.estate.view("tokens_of").await?.json()?;
    assert_eq!(tokens, vec![Some(usdc.id().to_string())]);

    ft_transfer_call(&s.grantor, &usdc, &s.estate, 5_000_000).await?;
    ft_transfer_call(&s.grantor, &usdc, &s.estate, 1).await?;

    assert_eq!(balance_of_token(&s, &usdc).await?, 5_000_001);
    assert_eq!(ft_balance(&usdc, s.estate.id()).await?, 5_000_001);
    assert_eq!(ft_balance(&usdc, s.grantor.id()).await?, SUPPLY - 5_000_001);
    Ok(())
}

#[tokio::test]
async fn a_strangers_token_deposit_is_returned() -> anyhow::Result<()> {
    let s = setup().await?;
    let usdc = mock_usdc(&s).await?;
    add_token(&s, usdc.id()).await?.into_result()?;

    // Give a stranger some mUSDC.
    let stranger = s.worker.dev_create_account().await?;
    stranger
        .call(usdc.id(), "storage_deposit")
        .args_json(json!({ "account_id": stranger.id() }))
        .deposit(NearToken::from_millinear(10))
        .transact()
        .await?
        .into_result()?;
    s.grantor
        .call(usdc.id(), "ft_transfer")
        .args_json(json!({ "receiver_id": stranger.id(), "amount": "1000" }))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?
        .into_result()?;

    ft_transfer_call(&stranger, &usdc, &s.estate, 1000).await?;
    assert_eq!(
        ft_balance(&usdc, stranger.id()).await?,
        1000,
        "refunded in full"
    );
    assert_eq!(ft_balance(&usdc, s.estate.id()).await?, 0);
    assert_eq!(balance_of_token(&s, &usdc).await?, 0);
    Ok(())
}

#[tokio::test]
async fn a_token_that_was_not_added_is_returned() -> anyhow::Result<()> {
    let s = setup().await?;
    let usdc = mock_usdc(&s).await?;
    // Registered with the token directly, but never added to the estate.
    s.grantor
        .call(usdc.id(), "storage_deposit")
        .args_json(json!({ "account_id": s.estate.id() }))
        .deposit(NearToken::from_millinear(10))
        .transact()
        .await?
        .into_result()?;

    ft_transfer_call(&s.grantor, &usdc, &s.estate, 1000).await?;
    assert_eq!(
        ft_balance(&usdc, s.grantor.id()).await?,
        SUPPLY,
        "refunded in full"
    );
    assert_eq!(balance_of_token(&s, &usdc).await?, 0);
    Ok(())
}

#[tokio::test]
async fn a_failed_registration_unlists_and_refunds() -> anyhow::Result<()> {
    let s = setup().await?;
    // An account with no token contract: storage_deposit fails.
    let not_a_token = s.worker.dev_create_account().await?;
    let estate_before = s
        .worker
        .view_account(s.estate.id())
        .await?
        .balance
        .as_yoctonear() as i128;

    let added: bool = add_token(&s, not_a_token.id())
        .await?
        .into_result()?
        .json()?;
    assert!(!added);
    let tokens: Vec<Option<String>> = s.estate.view("tokens_of").await?.json()?;
    assert!(tokens.is_empty());

    // The estate keeps nothing of the grantor's NEAR.
    let estate_change = s
        .worker
        .view_account(s.estate.id())
        .await?
        .balance
        .as_yoctonear() as i128
        - estate_before;
    let milli = NearToken::from_millinear(1).as_yoctonear() as i128;
    assert!(
        (0..milli).contains(&estate_change),
        "estate changed by {estate_change}"
    );
    Ok(())
}

// ---------- Part 3: per-asset shares ----------

/// What the estate can actually spend: its balance less what its storage locks up.
async fn free_near(s: &Setup) -> anyhow::Result<u128> {
    let a = s.worker.view_account(s.estate.id()).await?;
    Ok(a.balance.as_yoctonear() - a.storage_usage as u128 * 10u128.pow(19))
}

#[tokio::test]
async fn shares_are_set_per_asset_and_the_heirs_near_stays_backed() -> anyhow::Result<()> {
    let s = setup().await?;
    let usdc = mock_usdc(&s).await?;
    for (label, bps) in [("son", 6_000), ("daughter", 4_000)] {
        s.grantor
            .call(s.estate.id(), "add_heir")
            .args_json(json!({
                "label": label,
                "public_key": near_workspaces::types::SecretKey::from_random(
                    near_workspaces::types::KeyType::ED25519
                ).public_key(),
                "relationship": label,
                "default_bps": bps,
            }))
            .deposit(NearToken::from_near(1))
            .max_gas()
            .transact()
            .await?
            .into_result()?;
    }
    deposit(&s, &s.grantor, NearToken::from_near(3))
        .await?
        .into_result()?;

    s.grantor
        .call(s.estate.id(), "set_share")
        .args_json(json!({ "label": "son", "asset": usdc.id(), "bps": 2_000 }))
        .deposit(NearToken::from_millinear(10))
        .transact()
        .await?
        .into_result()?;

    let share = |label: &'static str, asset: serde_json::Value| {
        let estate = s.estate.clone();
        async move {
            let v: u16 = estate
                .view("share_of")
                .args_json(json!({ "label": label, "asset": asset }))
                .await?
                .json()?;
            anyhow::Ok(v)
        }
    };
    assert_eq!(share("son", json!(usdc.id())).await?, 2_000);
    assert_eq!(share("son", json!(null)).await?, 6_000);
    assert_eq!(share("daughter", json!(usdc.id())).await?, 4_000);
    let total: u16 = s
        .estate
        .view("total_share_of")
        .args_json(json!({ "asset": usdc.id() }))
        .await?
        .json()?;
    assert_eq!(total, 6_000);

    // Pushing usdc past 100% is refused.
    let over = s
        .grantor
        .call(s.estate.id(), "set_share")
        .args_json(json!({ "label": "daughter", "asset": usdc.id(), "bps": 8_001 }))
        .deposit(NearToken::from_millinear(10))
        .transact()
        .await?;
    assert!(failure_contains(over, "shares exceed 100%"));

    // Heirs, a deposit and an override later, every yoctoNEAR the books promise the heirs is
    // still free NEAR in the account: storage was always paid by the grantor's attached NEAR.
    assert!(free_near(&s).await? >= balance_of_near(&s).await?);
    Ok(())
}

// ---------- Part 4: withdrawals with a fresh Selfie Check ----------

async fn check_in(s: &Setup) -> anyhow::Result<()> {
    let args = sign(s, 0, 1, ATTESTOR_SECRET).await?;
    s.grantor
        .call(s.estate.id(), "check_in")
        .args_json(args)
        .transact()
        .await?
        .into_result()?;
    Ok(())
}

/// The grantor withdraws with a fresh withdrawal attestation (`nonce`), signed by `key`.
async fn withdraw(
    s: &Setup,
    asset: serde_json::Value,
    amount: u128,
    nonce: u64,
    key: [u8; 32],
) -> anyhow::Result<ExecutionFinalResult> {
    let mut args = sign(s, 2, nonce, key).await?;
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

#[tokio::test]
async fn near_comes_back_to_the_grantor_with_a_selfie_check() -> anyhow::Result<()> {
    let s = setup().await?;
    check_in(&s).await?;
    deposit(&s, &s.grantor, NearToken::from_near(3))
        .await?
        .into_result()?;
    let held = balance_of_near(&s).await?;
    let balance = |id: near_workspaces::AccountId| {
        let worker = s.worker.clone();
        async move { anyhow::Ok(worker.view_account(&id).await?.balance.as_yoctonear() as i128) }
    };
    // Views read the last final block, which lags a couple of blocks: let earlier refunds settle.
    s.worker.fast_forward(5).await?;
    let grantor_before = balance(s.grantor.id().clone()).await?;
    let estate_before = balance(s.estate.id().clone()).await?;

    let one = NearToken::from_near(1).as_yoctonear();
    let result = withdraw(&s, json!(null), one, 2, ATTESTOR_SECRET).await?;
    let logs = result.logs().join("\n");
    let sent: bool = result.into_result()?.json()?;
    assert!(sent);
    assert!(logs.contains(r#""event":"withdrawn""#), "logs: {logs}");

    assert_eq!(balance_of_near(&s).await?, held - one);
    // And let the transfer settle.
    s.worker.fast_forward(5).await?;
    // The estate paid out the 1 NEAR. It kept the 1 yoctoNEAR and may earn a gas reward.
    let milli = NearToken::from_millinear(1).as_yoctonear() as i128;
    let estate_paid = estate_before - balance(s.estate.id().clone()).await?;
    assert!(
        (one as i128 - milli..=one as i128).contains(&estate_paid),
        "estate paid {estate_paid}"
    );
    // The grantor got it, less gas.
    let gained = balance(s.grantor.id().clone()).await? - grantor_before;
    assert!(
        (one as i128 - 10 * milli..=one as i128).contains(&gained),
        "gained {gained}"
    );
    assert!(free_near(&s).await? >= balance_of_near(&s).await?);
    Ok(())
}

#[tokio::test]
async fn tokens_come_back_to_the_grantor_with_a_selfie_check() -> anyhow::Result<()> {
    let s = setup().await?;
    check_in(&s).await?;
    let usdc = mock_usdc(&s).await?;
    add_token(&s, usdc.id()).await?.into_result()?;
    ft_transfer_call(&s.grantor, &usdc, &s.estate, 5_000_000).await?;

    let sent: bool = withdraw(&s, json!(usdc.id()), 2_000_000, 2, ATTESTOR_SECRET)
        .await?
        .into_result()?
        .json()?;
    assert!(sent);
    assert_eq!(balance_of_token(&s, &usdc).await?, 3_000_000);
    assert_eq!(ft_balance(&usdc, s.estate.id()).await?, 3_000_000);
    assert_eq!(ft_balance(&usdc, s.grantor.id()).await?, SUPPLY - 3_000_000);
    Ok(())
}

#[tokio::test]
async fn a_failed_token_transfer_is_credited_back() -> anyhow::Result<()> {
    let s = setup().await?;
    check_in(&s).await?;
    let usdc = mock_usdc(&s).await?;
    add_token(&s, usdc.id()).await?.into_result()?;
    // Deposit everything, then unregister the grantor from the token, so ft_transfer to them fails.
    ft_transfer_call(&s.grantor, &usdc, &s.estate, SUPPLY).await?;
    s.grantor
        .call(usdc.id(), "storage_unregister")
        .args_json(json!({ "force": false }))
        .deposit(NearToken::from_yoctonear(1))
        .transact()
        .await?
        .into_result()?;

    let result = withdraw(&s, json!(usdc.id()), 1_000, 2, ATTESTOR_SECRET).await?;
    let logs = result.logs().join("\n");
    let sent: bool = result.into_result()?.json()?;
    assert!(!sent);
    assert!(
        logs.contains(r#""event":"withdraw_failed""#),
        "logs: {logs}"
    );
    assert_eq!(balance_of_token(&s, &usdc).await?, SUPPLY, "credited back");
    assert_eq!(ft_balance(&usdc, s.estate.id()).await?, SUPPLY);
    Ok(())
}

#[tokio::test]
async fn a_stolen_key_alone_moves_nothing() -> anyhow::Result<()> {
    let s = setup().await?;
    check_in(&s).await?;
    deposit(&s, &s.grantor, NearToken::from_near(3))
        .await?
        .into_result()?;
    let held = balance_of_near(&s).await?;

    // The thief signs the transaction with the grantor's key, but can't pass the Selfie Check:
    // no attestor signature...
    let forged = withdraw(&s, json!(null), held, 2, [9; 32]).await?;
    assert!(failure_contains(forged, "invalid signature"));
    // ...nor reuse the grantor's old check-in attestation.
    let replayed = withdraw(&s, json!(null), held, 1, ATTESTOR_SECRET).await?;
    assert!(failure_contains(replayed, "nonce used"));

    assert_eq!(balance_of_near(&s).await?, held);
    Ok(())
}
