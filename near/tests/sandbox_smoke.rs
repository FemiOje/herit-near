//! Loads the built wasm into a local NEAR node. Rebuild the contracts first, or this tests old code.

use anyhow::Context;
use serde_json::json;

fn wasm(name: &str) -> anyhow::Result<Vec<u8>> {
    let path = format!("../target/near/{name}/{name}.wasm");
    std::fs::read(&path).with_context(|| format!("{path} is missing; run cargo near build first"))
}

#[tokio::test]
async fn placeholders_deploy_and_answer() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;

    let factory = worker.dev_deploy(&wasm("herit_factory")?).await?;
    let version: String = factory.view("version").await?.json()?;
    assert_eq!(version, "0.1.0", "herit_factory");

    // The estate panics on every call until `new` has run.
    let estate = worker.dev_deploy(&wasm("herit_estate")?).await?;
    estate
        .call("new")
        .args_json(json!({
            "grantor": estate.id(),
            "attestor_pk": "ed25519:11111111111111111111111111111111",
            "check_in_interval_sec": 120,
            "grace_sec": 60,
        }))
        .transact()
        .await?
        .into_result()?;
    let version: String = estate.view("version").await?.json()?;
    assert_eq!(version, "0.1.0", "herit_estate");

    worker.fast_forward(10).await?;
    Ok(())
}

#[tokio::test]
async fn mock_ft_mints_the_supply_with_usdc_metadata() -> anyhow::Result<()> {
    let worker = near_workspaces::sandbox().await?;
    let token = worker.dev_deploy(&wasm("mock_ft")?).await?;
    let owner = token.id();

    token
        .call("new_default_meta")
        .args_json(json!({ "owner_id": owner, "total_supply": "1000000000000" }))
        .transact()
        .await?
        .into_result()?;

    let balance: String = token
        .view("ft_balance_of")
        .args_json(json!({ "account_id": owner }))
        .await?
        .json()?;
    assert_eq!(balance, "1000000000000");

    let metadata: serde_json::Value = token.view("ft_metadata").await?.json()?;
    assert_eq!(metadata["symbol"], "mUSDC");
    assert_eq!(metadata["decimals"], 6);
    Ok(())
}
