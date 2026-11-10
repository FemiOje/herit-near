//! Copies one live testnet contract into the sandbox. Needs the network, so it only runs with
//! `cargo test -p herit-tests -- --ignored`.

use near_workspaces::AccountId;

#[tokio::test]
#[ignore = "needs the network"]
async fn imports_a_live_contract() -> anyhow::Result<()> {
    let testnet = near_workspaces::testnet().await?;
    let sandbox = near_workspaces::sandbox().await?;
    let id: AccountId = "wrap.testnet".parse()?;
    let imported = sandbox.import_contract(&id, &testnet).transact().await?;
    assert_eq!(imported.id().as_str(), "wrap.testnet");
    Ok(())
}
