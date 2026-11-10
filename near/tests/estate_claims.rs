//! Claims on a real local node. Rebuild herit-estate and mock-ft first, or this tests old code.

mod common;

use common::*;
use near_workspaces::types::NearToken;
use serde_json::json;

// ---------- Part 1: the snapshot at unlock ----------

#[tokio::test]
async fn unlocking_freezes_the_balances_without_spending_the_heirs_near() -> anyhow::Result<()> {
    let s = setup().await?;
    check_in(&s).await?;
    deposit(&s, NearToken::from_near(3)).await?;
    let usdc = add_mock_usdc(&s).await?;
    deposit_token(&s, &usdc, 5_000_000).await?;
    assert!(!s.estate.view("snapshot_taken").await?.json::<bool>()?);

    wait_for_unlock(&s).await?;
    let storage_before = s.worker.view_account(s.estate.id()).await?.storage_usage;
    // Anyone may poke; here a stranger does.
    let stranger = s.worker.dev_create_account().await?;
    let result = stranger
        .call(s.estate.id(), "poke_expiry")
        .transact()
        .await?;
    let logs = result.logs().join("\n");
    result.into_result()?;
    assert!(
        logs.contains(r#""event":"snapshotted","data":[{"assets":2}]"#),
        "logs: {logs}"
    );

    assert!(s.estate.view("snapshot_taken").await?.json::<bool>()?);
    let near = amount_of(&s, "balance_of", json!(null)).await?;
    assert_eq!(amount_of(&s, "snapshot_of", json!(null)).await?, near);
    assert_eq!(
        amount_of(&s, "snapshot_of", json!(usdc.id())).await?,
        5_000_000
    );

    // No new storage: the entries were paid for at listing, so the heirs' NEAR is untouched.
    assert_eq!(
        s.worker.view_account(s.estate.id()).await?.storage_usage,
        storage_before
    );
    assert!(free_near(&s).await? >= near);
    Ok(())
}

// ---------- Part 2: who may claim ----------

const SON_FACE: [u8; 32] = [3; 32];

#[tokio::test]
async fn an_heir_claims_from_their_own_account_with_their_own_face() -> anyhow::Result<()> {
    let s = setup().await?;
    let son = add_heir(&s, "son", 6_000).await?;
    let daughter = add_heir(&s, "daughter", 4_000).await?;
    check_in(&s).await?;
    deposit(&s, NearToken::from_near(2)).await?;

    // Too early.
    let early = claim(&s, &son, "son", SON_FACE, 10).await?;
    assert!(failure_contains(early, "estate not unlocked"));

    wait_for_unlock(&s).await?;
    // The daughter's account can't claim the son's slot, and nobody claims with the grantor's face.
    assert!(failure_contains(
        claim(&s, &daughter, "son", SON_FACE, 11).await?,
        "not entitled"
    ));
    assert!(failure_contains(
        claim(&s, &son, "son", GRANTOR_FACE, 12).await?,
        "wrong human"
    ));

    // The son's own claim unlocks the estate, freezes the balances and records his face.
    let result = claim(&s, &son, "son", SON_FACE, 13).await?;
    let logs = result.logs().join("\n");
    result.into_result()?;
    assert!(logs.contains(r#""event":"snapshotted""#), "logs: {logs}");
    assert!(logs.contains(r#""event":"claim_attested""#), "logs: {logs}");
    let face = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, SON_FACE);
    let used: Option<String> = s
        .estate
        .view("claim_commitment_used")
        .args_json(json!({ "commitment": face }))
        .await?
        .json()?;
    assert_eq!(used.as_deref(), Some("son"));

    // The same human can't take the daughter's slot too.
    let second = claim(&s, &daughter, "daughter", SON_FACE, 14).await?;
    assert!(failure_contains(second, "commitment used"));
    Ok(())
}

// ---------- Part 3: payouts ----------

const DAUGHTER_FACE: [u8; 32] = [4; 32];

async fn ft_balance(
    token: &near_workspaces::Contract,
    of: &near_workspaces::AccountId,
) -> anyhow::Result<u128> {
    let v: String = token
        .view("ft_balance_of")
        .args_json(json!({ "account_id": of }))
        .await?
        .json()?;
    Ok(v.parse()?)
}

#[tokio::test]
async fn both_heirs_are_paid_their_share_of_near_and_tokens() -> anyhow::Result<()> {
    let s = setup().await?;
    let son = add_heir(&s, "son", 6_000).await?;
    let daughter = add_heir(&s, "daughter", 4_000).await?;
    check_in(&s).await?;
    deposit(&s, NearToken::from_near(3)).await?;
    let usdc = add_mock_usdc(&s).await?;
    deposit_token(&s, &usdc, 5_000_001).await?; // odd, so the split leaves 1 unit of dust
    s.grantor
        .call(s.estate.id(), "set_share")
        .args_json(json!({ "label": "son", "asset": usdc.id(), "bps": 2_000 }))
        .deposit(NearToken::from_millinear(10))
        .transact()
        .await?
        .into_result()?;
    wait_for_unlock(&s).await?;
    let near_held = amount_of(&s, "balance_of", json!(null)).await?;

    let son_before = s
        .worker
        .view_account(son.id())
        .await?
        .balance
        .as_yoctonear();
    let result = claim(&s, &son, "son", SON_FACE, 10).await?;
    let gas = result.total_gas_burnt;
    let logs = result.logs().join("\n");
    let transfers: u32 = result.into_result()?.json()?;
    assert_eq!(transfers, 2);
    assert_eq!(
        logs.matches(r#""event":"claimed""#).count(),
        2,
        "logs: {logs}"
    );
    println!("a two-asset claim burnt {} Tgas", gas.as_tgas());

    claim(&s, &daughter, "daughter", DAUGHTER_FACE, 11)
        .await?
        .into_result()?;

    // Tokens: the son's 20% override and the daughter's 40%, rounded down, of the snapshot.
    assert_eq!(ft_balance(&usdc, son.id()).await?, 1_000_000);
    assert_eq!(ft_balance(&usdc, daughter.id()).await?, 2_000_000);
    assert_eq!(
        amount_of(&s, "balance_of", json!(usdc.id())).await?,
        5_000_001 - 3_000_000
    );
    // NEAR: 60% and 40% of the snapshot; nothing but rounding dust is left.
    let son_near = near_held * 6_000 / 10_000;
    let daughter_near = near_held * 4_000 / 10_000;
    assert_eq!(
        amount_of(&s, "balance_of", json!(null)).await?,
        near_held - son_near - daughter_near
    );
    s.worker.fast_forward(5).await?;
    let son_gain = s
        .worker
        .view_account(son.id())
        .await?
        .balance
        .as_yoctonear() as i128
        - son_before as i128;
    let milli = NearToken::from_millinear(1).as_yoctonear() as i128;
    // His share, less the claim's gas, records and token registration (a few hundredths in the sandbox).
    assert!(
        (son_near as i128 - 80 * milli..son_near as i128).contains(&son_gain),
        "son gained {son_gain}"
    );

    // Paid once: a second claim finds nothing, and the estate still covers its books.
    assert!(failure_contains(
        claim(&s, &son, "son", SON_FACE, 12).await?,
        "nothing to claim"
    ));
    assert!(free_near(&s).await? >= amount_of(&s, "balance_of", json!(null)).await?);
    Ok(())
}
