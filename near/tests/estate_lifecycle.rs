//! A whole estate on a real local node, from opening to both heirs paid: the deployment guide's
//! §8.4 smoke run, minus the factory. Rebuild herit-estate and mock-ft first.

mod common;

use common::*;
use near_workspaces::result::ExecutionFinalResult;
use near_workspaces::types::NearToken;
use serde_json::json;

const SON_FACE: [u8; 32] = [3; 32];
const DAUGHTER_FACE: [u8; 32] = [4; 32];

/// Runs a transaction that must succeed and returns its event names.
fn ok(result: ExecutionFinalResult) -> anyhow::Result<Vec<String>> {
    let names = events(&result);
    result.into_result()?;
    Ok(names)
}

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
async fn an_estate_from_opening_to_both_heirs_paid() -> anyhow::Result<()> {
    let s = setup().await?;
    let mut trail: Vec<String> = vec![];

    // ---- Setup: two heirs, the first check-in, NEAR and mock USDC, a USDC override. ----
    let son = add_heir(&s, "son", 6_000).await?;
    let daughter = add_heir(&s, "daughter", 4_000).await?;
    trail.extend(["heir_added".into(), "heir_added".into()]);
    trail.extend(ok(check_in_with(&s, 1).await?)?);
    deposit(&s, NearToken::from_near(5)).await?;
    trail.push("deposited".into());
    let usdc = add_mock_usdc(&s).await?;
    trail.push("token_added".into());
    deposit_token(&s, &usdc, 10_000_000).await?;
    trail.push("deposited".into());
    trail.extend(ok(s
        .grantor
        .call(s.estate.id(), "set_share")
        .args_json(json!({ "label": "son", "asset": usdc.id(), "bps": 2_500 }))
        .deposit(NearToken::from_millinear(10))
        .transact()
        .await?)?);
    assert_eq!(status(&s).await?, "Active");
    // Each heir holds a credential, locked while the grantor lives.
    assert_eq!(credential_state(&s, "son").await?, "locked");
    assert_eq!(credential_state(&s, "daughter").await?, "locked");

    // ---- While alive: the grantor takes 1 NEAR back, with a Selfie Check. ----
    let one = NearToken::from_near(1).as_yoctonear();
    let near_before = amount_of(&s, "balance_of", json!(null)).await?;
    trail.extend(ok(withdraw(&s, json!(null), one, 2).await?)?);
    assert_eq!(
        amount_of(&s, "balance_of", json!(null)).await?,
        near_before - one
    );

    // ---- A missed check-in: Grace, then recovery by checking in. ----
    wait_for_status(&s, "Grace").await?;
    assert_eq!(
        credential_state(&s, "son").await?,
        "grace",
        "computed: nobody has poked yet"
    );
    trail.extend(ok(s
        .grantor
        .call(s.estate.id(), "poke_expiry")
        .transact()
        .await?)?);
    trail.extend(ok(check_in_with(&s, 3).await?)?);
    assert_eq!(
        status(&s).await?,
        "Active",
        "a check-in in Grace recovers the estate"
    );
    assert_eq!(
        credential_state(&s, "son").await?,
        "locked",
        "and the credentials with it"
    );

    // ---- The grantor stops checking in. Unlocked, nobody poked yet. ----
    wait_for_status(&s, "Unlocked").await?;
    let near_at_unlock = amount_of(&s, "balance_of", json!(null)).await?;
    assert_eq!(near_at_unlock, near_before - one);
    // The credentials wake up on the clock alone: nobody has poked.
    assert_eq!(credential_state(&s, "son").await?, "claimable");
    assert_eq!(credential_state(&s, "daughter").await?, "claimable");
    // Now it's worth something, and it still can't leave the heir's account.
    let sale = son
        .call(s.estate.id(), "nft_transfer")
        .args_json(json!({ "receiver_id": s.grantor.id(), "token_id": "son" }))
        .deposit(NearToken::from_yoctonear(1))
        .gas(near_workspaces::types::Gas::from_tgas(30))
        .transact()
        .await?;
    assert!(failure_contains(sale, "soulbound"));

    // Every grantor action is now refused, poke or no poke.
    assert!(failure_contains(
        check_in_with(&s, 4).await?,
        "estate unlocked"
    ));
    assert!(failure_contains(
        withdraw(&s, json!(null), 1, 5).await?,
        "estate unlocked"
    ));
    let late_deposit = s
        .grantor
        .call(s.estate.id(), "deposit")
        .deposit(NearToken::from_near(1))
        .transact()
        .await?;
    assert!(failure_contains(late_deposit, "estate unlocked"));

    // ---- The son claims: his claim unlocks the estate, freezes it, and pays him. ----
    let claimed = ok(claim(&s, &son, "son", SON_FACE, 10).await?)?;
    trail.extend(claimed.clone());
    assert_eq!(
        claimed[..3],
        [
            "snapshotted".to_string(),
            "unlocked".into(),
            "claim_attested".into()
        ],
        "the claim's own poke unlocks and freezes the estate first"
    );
    assert_eq!(
        amount_of(&s, "snapshot_of", json!(null)).await?,
        near_at_unlock
    );
    assert_eq!(
        amount_of(&s, "snapshot_of", json!(usdc.id())).await?,
        10_000_000
    );

    assert_eq!(credential_state(&s, "son").await?, "claimed");
    assert_eq!(
        credential_state(&s, "daughter").await?,
        "claimable",
        "each heir's own state"
    );

    // ---- The daughter claims later; her 40% is still of the same snapshot. ----
    s.worker.fast_forward(50).await?;
    trail.extend(ok(
        claim(&s, &daughter, "daughter", DAUGHTER_FACE, 11).await?
    )?);

    assert_eq!(credential_state(&s, "daughter").await?, "claimed");

    // ---- Everyone got exactly their share. ----
    assert_eq!(
        ft_balance(&usdc, son.id()).await?,
        2_500_000,
        "his 25% USDC override"
    );
    assert_eq!(
        ft_balance(&usdc, daughter.id()).await?,
        4_000_000,
        "her 40% default"
    );
    assert_eq!(
        amount_of(&s, "balance_of", json!(usdc.id())).await?,
        3_500_000,
        "35% nobody was given"
    );
    let son_near = near_at_unlock * 6_000 / 10_000;
    let daughter_near = near_at_unlock * 4_000 / 10_000;
    let dust = near_at_unlock - son_near - daughter_near;
    assert!(dust < 2, "only rounding dust is left: {dust}");
    assert_eq!(amount_of(&s, "balance_of", json!(null)).await?, dust);
    for (label, asset) in [
        ("son", json!(null)),
        ("son", json!(usdc.id())),
        ("daughter", json!(null)),
    ] {
        let paid: bool = s
            .estate
            .view("has_claimed")
            .args_json(json!({ "label": label, "asset": asset }))
            .await?
            .json()?;
        assert!(paid, "{label} {asset}");
    }

    // Nobody gets paid twice, and the estate can still cover what its books say it holds.
    assert!(failure_contains(
        claim(&s, &son, "son", SON_FACE, 12).await?,
        "nothing to claim"
    ));
    assert!(failure_contains(
        claim(&s, &daughter, "daughter", DAUGHTER_FACE, 13).await?,
        "nothing to claim"
    ));
    assert!(free_near(&s).await? >= amount_of(&s, "balance_of", json!(null)).await?);

    // ---- The trail an activity feed would show, in order. ----
    let expected = [
        "heir_added",
        "heir_added",
        "checked_in",
        "deposited",
        "token_added",
        "deposited",
        "share_set",
        "withdrawn",
        "entered_grace",
        "checked_in",
        "snapshotted",
        "unlocked",
        "claim_attested",
        "claimed",
        "claimed",
        "claim_settled",
        "claim_attested",
        "claimed",
        "claimed",
        "claim_settled",
    ];
    // Each `claimed` comes from its own callback receipt, so it lands after `claim_settled`, in
    // whichever order the transfers finish. Compare as a multiset, then check the phases' order.
    let mut got = trail.clone();
    got.sort();
    let mut want: Vec<String> = expected.iter().map(|e| e.to_string()).collect();
    want.sort();
    assert_eq!(got, want, "trail: {trail:?}");
    let first_claim = trail.iter().position(|e| e == "claim_attested").unwrap();
    assert!(
        trail[..first_claim].iter().all(|e| e != "claimed"),
        "nothing paid before the first claim"
    );
    assert!(
        trail.iter().position(|e| e == "entered_grace")
            < trail.iter().position(|e| e == "snapshotted")
    );
    Ok(())
}
