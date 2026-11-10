//! The estate's liveness clock on a real local node: init, views, and who may call what.
//! Rebuild herit-estate first, or this tests old code.

use anyhow::Context;
use near_workspaces::{Account, Contract, Worker, network::Sandbox};
use serde_json::{Value, json};

const ATTESTOR_PK: &str = "ed25519:11111111111111111111111111111111";

struct Setup {
    worker: Worker<Sandbox>,
    estate: Contract,
    factory: Account,
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
        .args_json(new_args(&grantor))
        .transact()
        .await?
        .into_result()?;
    Ok(Setup {
        worker,
        estate,
        factory,
        grantor,
    })
}

fn new_args(grantor: &Account) -> Value {
    json!({
        "grantor": grantor.id(),
        "attestor_pk": ATTESTOR_PK,
        "check_in_interval_sec": 120,
        "grace_sec": 60,
    })
}

#[tokio::test]
async fn opens_active_with_no_deadlines() -> anyhow::Result<()> {
    let s = setup().await?;
    let status: String = s.estate.view("status_of").await?.json()?;
    assert_eq!(status, "Active");
    let deadlines: (String, String) = s.estate.view("deadlines_of").await?.json()?;
    assert_eq!(deadlines, ("0".to_string(), "0".to_string()));
    Ok(())
}

#[tokio::test]
async fn new_cannot_run_twice() -> anyhow::Result<()> {
    let s = setup().await?;
    let again = s
        .factory
        .call(s.estate.id(), "new")
        .args_json(new_args(&s.grantor))
        .transact()
        .await?;
    assert!(again.is_failure(), "a second new must be refused");
    Ok(())
}

#[tokio::test]
async fn only_the_grantor_can_configure() -> anyhow::Result<()> {
    let s = setup().await?;
    let args = json!({ "check_in_interval_sec": 600, "grace_sec": 120 });

    let stranger = s.worker.dev_create_account().await?;
    let refused = stranger
        .call(s.estate.id(), "configure")
        .args_json(&args)
        .transact()
        .await?;
    assert!(refused.is_failure());
    assert!(format!("{:?}", refused.into_result().unwrap_err()).contains("not the grantor"));

    let done = s
        .grantor
        .call(s.estate.id(), "configure")
        .args_json(&args)
        .transact()
        .await?;
    let logs = done.logs().join("\n");
    done.into_result()?;
    assert!(logs.contains(r#""event":"configured""#), "logs: {logs}");
    Ok(())
}

#[tokio::test]
async fn no_check_in_never_lapses_on_chain() -> anyhow::Result<()> {
    let s = setup().await?;
    s.worker.fast_forward(500).await?; // well past 120 s + 60 s of blocks
    let status: String = s
        .grantor
        .call(s.estate.id(), "poke_expiry")
        .transact()
        .await?
        .into_result()?
        .json()?;
    assert_eq!(status, "Active");
    Ok(())
}
