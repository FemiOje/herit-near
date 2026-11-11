//! The factory on a real local node, opening estates that run the real estate code published as
//! a Global Contract. Rebuild herit-estate, herit-factory and test-global-publisher first.

mod common;

use common::*;
use near_workspaces::types::{KeyType, NearToken, SecretKey};
use near_workspaces::{Account, AccountId, Contract, Worker, network::Sandbox};
use serde_json::{Value, json};

struct Factory {
    worker: Worker<Sandbox>,
    factory: Contract,
    code_hash: String,
}

/// Publishes the estate code (as testnet will with the CLI), then deploys and initialises the
/// factory in one transaction.
async fn factory() -> anyhow::Result<Factory> {
    let worker = near_workspaces::sandbox().await?;
    let estate_wasm = wasm("herit_estate")?;
    let publisher = worker.dev_deploy(&wasm("test_global_publisher")?).await?;
    publisher
        .call("publish")
        .args(estate_wasm.clone())
        .max_gas()
        .transact()
        .await?
        .into_result()?;
    let code_hash = bs58::encode(sha256(&estate_wasm)).into_string();

    let pk = ed25519_dalek::SigningKey::from_bytes(&ATTESTOR_SECRET)
        .verifying_key()
        .to_bytes();
    let factory_account = worker.dev_create_account().await?;
    let factory = factory_account
        .batch(factory_account.id())
        .deploy(&wasm("herit_factory")?)
        .call(
            near_workspaces::operations::Function::new("new").args_json(json!({
                "estate_code_hash": code_hash,
                "attestor_pk": format!("ed25519:{}", bs58::encode(pk).into_string()),
            })),
        )
        .transact()
        .await?;
    factory.into_result()?;
    let factory = Contract::from_secret_key(
        factory_account.id().clone(),
        factory_account.secret_key().clone(),
        &worker,
    );
    Ok(Factory {
        worker,
        factory,
        code_hash,
    })
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).into()
}

fn estate_id(f: &Factory, label: &str) -> AccountId {
    format!("{label}.{}", f.factory.id()).parse().unwrap()
}

async fn open(
    f: &Factory,
    grantor: &Account,
    label: &str,
    interval: u32,
    deposit: NearToken,
) -> anyhow::Result<near_workspaces::result::ExecutionFinalResult> {
    Ok(grantor
        .call(f.factory.id(), "open_estate")
        .args_json(json!({ "label": label, "check_in_interval_sec": interval, "grace_sec": 60 }))
        .deposit(deposit)
        .max_gas()
        .transact()
        .await?)
}

#[tokio::test]
async fn the_factory_opens_a_keyless_estate_running_the_published_code() -> anyhow::Result<()> {
    let f = factory().await?;
    let grantor = f.worker.dev_create_account().await?;

    let result = open(&f, &grantor, "alice", 120, NearToken::from_near(1)).await?;
    let logs = result.logs().join("\n");
    let created: bool = result.into_result()?.json()?;
    assert!(created);
    assert!(logs.contains(r#""event":"estate_opened""#), "logs: {logs}");

    // A real account, running the published code by its hash, with no keys at all.
    let estate = estate_id(&f, "alice");
    let details = f.worker.view_account(&estate).await?;
    assert_eq!(
        format!("{:?}", details.contract_state),
        format!("GlobalHash({})", f.code_hash),
        "the estate runs the Global Contract"
    );
    assert!(
        f.worker.view_access_keys(&estate).await?.is_empty(),
        "keyless"
    );
    // Funded with the deposit less the factory's fee.
    let held = details.balance.as_yoctonear();
    assert!(
        held > NearToken::from_millinear(980).as_yoctonear()
            && held <= NearToken::from_millinear(990).as_yoctonear(),
        "{held}"
    );

    // The factory's records.
    let mine: Vec<String> = f
        .factory
        .view("estates_of")
        .args_json(json!({ "grantor": grantor.id() }))
        .await?
        .json()?;
    assert_eq!(mine, vec![estate.to_string()]);
    let count: u32 = f.factory.view("estates_count").await?.json()?;
    assert_eq!(count, 1);
    let config: Value = f.factory.view("config").await?.json()?;
    assert_eq!(config["estate_code_hash"], f.code_hash);
    Ok(())
}

#[tokio::test]
async fn an_estate_opened_by_the_factory_runs_its_whole_life() -> anyhow::Result<()> {
    let f = factory().await?;
    let grantor = f.worker.dev_create_account().await?;
    open(&f, &grantor, "alice", 120, NearToken::from_near(1))
        .await?
        .into_result()?;
    // A view handle for the keyless estate: views need no key, and calls go from real accounts.
    let estate = Contract::from_secret_key(
        estate_id(&f, "alice"),
        SecretKey::from_random(KeyType::ED25519),
        &f.worker,
    );
    let s = Setup {
        worker: f.worker.clone(),
        estate,
        grantor,
    };

    // The caller became the grantor, and the factory's attestor key came with the estate.
    let son = add_heir(&s, "son", 10_000).await?;
    check_in(&s).await?;
    deposit(&s, NearToken::from_near(2)).await?;
    let held = amount_of(&s, "balance_of", json!(null)).await?;
    wait_for_unlock(&s).await?;
    claim(&s, &son, "son", [3; 32], 10).await?.into_result()?;
    assert_eq!(
        amount_of(&s, "balance_of", json!(null)).await?,
        0,
        "the son took 100% of {held}"
    );
    Ok(())
}

#[tokio::test]
async fn a_failed_opening_refunds_everything_and_records_nothing() -> anyhow::Result<()> {
    let f = factory().await?;
    let grantor = f.worker.dev_create_account().await?;
    open(&f, &grantor, "alice", 120, NearToken::from_near(1))
        .await?
        .into_result()?;
    f.worker.fast_forward(5).await?;

    for (why, label, interval) in [
        ("the name is taken", "alice", 120),
        ("the timers are out of bounds", "bob", 1),
    ] {
        let before = f
            .worker
            .view_account(grantor.id())
            .await?
            .balance
            .as_yoctonear();
        let created: bool = open(&f, &grantor, label, interval, NearToken::from_near(1))
            .await?
            .into_result()?
            .json()?;
        assert!(!created, "{why}");
        f.worker.fast_forward(5).await?;
        let lost = before
            - f.worker
                .view_account(grantor.id())
                .await?
                .balance
                .as_yoctonear();
        assert!(
            lost < NearToken::from_millinear(30).as_yoctonear(),
            "{why}: lost {lost}, more than gas"
        );
    }
    assert!(
        f.worker.view_account(&estate_id(&f, "bob")).await.is_err(),
        "no account for bob"
    );
    let count: u32 = f.factory.view("estates_count").await?.json()?;
    assert_eq!(count, 1);
    Ok(())
}

#[tokio::test]
async fn only_the_factory_runs_its_callback_and_sets_the_attestor() -> anyhow::Result<()> {
    let f = factory().await?;
    let stranger = f.worker.dev_create_account().await?;
    let callback = stranger
        .call(f.factory.id(), "on_estate_created")
        .args_json(json!({ "estate": stranger.id(), "grantor": stranger.id(), "deposit": "1" }))
        .transact()
        .await?;
    assert!(failure_contains(callback, "is private"));
    let attestor = stranger
        .call(f.factory.id(), "set_attestor")
        .args_json(json!({ "attestor_pk": "ed25519:6E8sCci9badyRkXb3JoRpBj5p8C6Tw41ELDZoiihKEtp" }))
        .transact()
        .await?;
    assert!(failure_contains(attestor, "is private"));
    let code = stranger
        .call(f.factory.id(), "set_estate_code")
        .args_json(json!({ "estate_code_hash": "11111111111111111111111111111111" }))
        .transact()
        .await?;
    assert!(failure_contains(code, "is private"));
    Ok(())
}
