//! Heirs on a real local node: `add_heir` creates `<label>.<estate>` holding the heir's own key, a
//! failed creation rolls back and refunds, and only the estate can run the callback.
//! Rebuild herit-estate first, or this tests old code.

use anyhow::Context;
use near_workspaces::result::ExecutionFinalResult;
use near_workspaces::types::{KeyType, NearToken, SecretKey};
use near_workspaces::{Account, AccountId, Contract, Worker, network::Sandbox};
use serde_json::{Value, json};

const SEED: NearToken = NearToken::from_millinear(100);

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
            // Any valid key: these tests never check in.
            "attestor_pk": "ed25519:6E8sCci9badyRkXb3JoRpBj5p8C6Tw41ELDZoiihKEtp",
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

fn heir_id(s: &Setup, label: &str) -> AccountId {
    format!("{label}.{}", s.estate.id()).parse().unwrap()
}

async fn add_heir(
    s: &Setup,
    label: &str,
    key: &SecretKey,
    bps: u16,
) -> anyhow::Result<ExecutionFinalResult> {
    Ok(s.grantor
        .call(s.estate.id(), "add_heir")
        .args_json(json!({
            "label": label,
            "public_key": key.public_key(),
            "relationship": "son",
            "default_bps": bps,
        }))
        .deposit(NearToken::from_near(1))
        .max_gas()
        .transact()
        .await?)
}

async fn heir_of(s: &Setup, label: &str) -> anyhow::Result<Value> {
    Ok(s.estate
        .view("heir_of")
        .args_json(json!({ "label": label }))
        .await?
        .json()?)
}

fn failure_contains(result: ExecutionFinalResult, text: &str) -> bool {
    result.is_failure() && format!("{:?}", result.into_result().unwrap_err()).contains(text)
}

async fn balance(s: &Setup, id: &AccountId) -> anyhow::Result<u128> {
    Ok(s.worker.view_account(id).await?.balance.as_yoctonear())
}

#[tokio::test]
async fn add_heir_creates_an_account_only_the_heir_controls() -> anyhow::Result<()> {
    let s = setup().await?;
    let key = SecretKey::from_random(KeyType::ED25519);
    let before = balance(&s, s.grantor.id()).await?;

    let result = add_heir(&s, "son", &key, 5_000).await?;
    let logs = result.logs().join("\n");
    let created: bool = result.into_result()?.json()?;
    assert!(created);
    assert!(logs.contains(r#""event":"heir_added""#), "logs: {logs}");

    let heir = heir_of(&s, "son").await?;
    assert_eq!(heir["account_id"], json!(heir_id(&s, "son")));
    assert_eq!(heir["created"], json!(true));
    assert_eq!(heir["default_bps"], json!(5_000));

    // The account exists, holds the seed, and has exactly the heir's key.
    let seed = balance(&s, &heir_id(&s, "son")).await?;
    assert!(
        seed > SEED.as_yoctonear() * 9 / 10 && seed <= SEED.as_yoctonear(),
        "seed {seed}"
    );
    let keys = s.worker.view_access_keys(&heir_id(&s, "son")).await?;
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].public_key, key.public_key());

    // The heir can sign for it.
    let heir = Account::from_secret_key(heir_id(&s, "son"), key, &s.worker);
    heir.call(s.estate.id(), "poke_expiry")
        .transact()
        .await?
        .into_result()?;

    // The grantor paid the seed plus a little storage and gas; the rest of the 1 NEAR came back.
    let spent = before - balance(&s, s.grantor.id()).await?;
    let ceiling = SEED.as_yoctonear() + NearToken::from_millinear(20).as_yoctonear();
    assert!(
        spent > SEED.as_yoctonear() && spent < ceiling,
        "spent {spent}"
    );
    Ok(())
}

#[tokio::test]
async fn a_label_cannot_be_added_twice() -> anyhow::Result<()> {
    let s = setup().await?;
    let key = SecretKey::from_random(KeyType::ED25519);
    add_heir(&s, "son", &key, 5_000).await?.into_result()?;
    assert!(failure_contains(
        add_heir(&s, "son", &key, 1_000).await?,
        "label taken"
    ));
    Ok(())
}

#[tokio::test]
async fn a_failed_creation_rolls_back_and_refunds() -> anyhow::Result<()> {
    let s = setup().await?;
    let key = SecretKey::from_random(KeyType::ED25519);
    add_heir(&s, "son", &key, 5_000).await?.into_result()?;
    s.grantor
        .call(s.estate.id(), "remove_heir")
        .args_json(json!({ "label": "son" }))
        .transact()
        .await?
        .into_result()?;

    // son.<estate> still exists, held by the heir, so creating it again fails.
    let before = balance(&s, s.grantor.id()).await? as i128;
    let estate_before = balance(&s, s.estate.id()).await? as i128;
    let result = add_heir(&s, "son", &SecretKey::from_random(KeyType::ED25519), 5_000).await?;
    let logs = result.logs().join("\n");
    let created: bool = result.into_result()?.json()?;
    assert!(!created);
    assert!(logs.contains(r#""event":"heir_failed""#), "logs: {logs}");
    assert_eq!(heir_of(&s, "son").await?, Value::Null);

    // The estate keeps nothing: the seed came back to it and went on to the grantor with the
    // storage deposit. (Earning a gas reward is fine; holding the grantor's money isn't.)
    let estate_change = balance(&s, s.estate.id()).await? as i128 - estate_before;
    let milli = NearToken::from_millinear(1).as_yoctonear() as i128;
    assert!(
        (0..milli).contains(&estate_change),
        "estate changed by {estate_change}"
    );
    // The grantor loses at most gas. It can even gain: an earlier transaction's gas refund may land
    // in between.
    let spent = before - balance(&s, s.grantor.id()).await? as i128;
    assert!(spent < 10 * milli, "spent {spent}");
    // The heir's account and key are untouched.
    let keys = s.worker.view_access_keys(&heir_id(&s, "son")).await?;
    assert_eq!(keys[0].public_key, key.public_key());
    Ok(())
}

#[tokio::test]
async fn only_the_estate_can_run_the_callback() -> anyhow::Result<()> {
    let s = setup().await?;
    let result = s
        .grantor
        .call(s.estate.id(), "on_heir_created")
        .args_json(json!({ "label": "son", "refund": "1000" }))
        .transact()
        .await?;
    assert!(failure_contains(result, "is private"));
    Ok(())
}

#[tokio::test]
async fn shares_cannot_exceed_100_percent() -> anyhow::Result<()> {
    let s = setup().await?;
    add_heir(&s, "son", &SecretKey::from_random(KeyType::ED25519), 6_000)
        .await?
        .into_result()?;
    let over = add_heir(
        &s,
        "daughter",
        &SecretKey::from_random(KeyType::ED25519),
        4_001,
    )
    .await?;
    assert!(failure_contains(over, "shares exceed 100%"));
    // Nothing was created for the refused heir.
    assert!(
        s.worker
            .view_account(&heir_id(&s, "daughter"))
            .await
            .is_err()
    );
    Ok(())
}
