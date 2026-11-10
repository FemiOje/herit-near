//! The heir credential on a real local node. Rebuild herit-estate first, or this tests old code.

mod common;

use common::*;
use serde_json::{Value, json};

// ---------- Part 1: the NEP-171 views ----------

#[tokio::test]
async fn an_heirs_credential_reads_as_a_standard_nep171_token() -> anyhow::Result<()> {
    let s = setup().await?;
    let son = add_heir(&s, "son", 6_000).await?;

    let meta: Value = s.estate.view("nft_metadata").await?.json()?;
    assert_eq!(meta["spec"], "nft-1.0.0");
    assert_eq!(meta["symbol"], "HEIR");

    // What a wallet asks for: the tokens this account owns.
    let owned: Vec<Value> = s
        .estate
        .view("nft_tokens_for_owner")
        .args_json(json!({ "account_id": son.id() }))
        .await?
        .json()?;
    assert_eq!(owned.len(), 1);
    let token = &owned[0];
    assert_eq!(token["token_id"], "son");
    assert_eq!(token["owner_id"], json!(son.id()));
    assert_eq!(token["metadata"]["title"], json!(son.id()));
    assert_eq!(token["metadata"]["copies"], 1);
    // The image for its state, on IPFS, resolved through the collection's base_uri.
    assert_eq!(meta["base_uri"], "https://gateway.pinata.cloud/ipfs");
    assert!(
        token["metadata"]["media"]
            .as_str()
            .unwrap()
            .starts_with("bafkrei")
    );
    assert!(token["metadata"]["media_hash"].is_string());
    let extra: Value = serde_json::from_str(token["metadata"]["extra"].as_str().unwrap())?;
    assert_eq!(extra["state"], "locked");

    let supply: String = s.estate.view("nft_total_supply").await?.json()?;
    assert_eq!(supply, "1");
    Ok(())
}

// ---------- Part 2: soulbound, and the standard events ----------

#[tokio::test]
async fn the_heir_cannot_transfer_their_credential() -> anyhow::Result<()> {
    let s = setup().await?;
    let son = add_heir(&s, "son", 6_000).await?;
    let stranger = s.worker.dev_create_account().await?;
    for (method, args) in [
        (
            "nft_transfer",
            json!({ "receiver_id": stranger.id(), "token_id": "son" }),
        ),
        (
            "nft_transfer_call",
            json!({ "receiver_id": stranger.id(), "token_id": "son", "msg": "" }),
        ),
        (
            "nft_approve",
            json!({ "token_id": "son", "account_id": stranger.id() }),
        ),
    ] {
        let result = son
            .call(s.estate.id(), method)
            .args_json(args)
            .deposit(near_workspaces::types::NearToken::from_yoctonear(1))
            .gas(near_workspaces::types::Gas::from_tgas(30))
            .transact()
            .await?;
        assert!(failure_contains(result, "soulbound"), "{method}");
    }
    let token: Value = s
        .estate
        .view("nft_token")
        .args_json(json!({ "token_id": "son" }))
        .await?
        .json()?;
    assert_eq!(token["owner_id"], json!(son.id()));
    Ok(())
}

#[tokio::test]
async fn explorers_see_the_mint_and_the_burn() -> anyhow::Result<()> {
    use near_workspaces::types::{KeyType, NearToken, SecretKey};
    let s = setup().await?;
    let key = SecretKey::from_random(KeyType::ED25519);
    let added = s
        .grantor
        .call(s.estate.id(), "add_heir")
        .args_json(json!({ "label": "son", "public_key": key.public_key(), "relationship": "son", "default_bps": 5_000 }))
        .deposit(NearToken::from_near(1))
        .max_gas()
        .transact()
        .await?;
    let mint = added
        .logs()
        .into_iter()
        .find(|l| l.contains(r#""standard":"nep171""#))
        .map(String::from);
    added.into_result()?;
    let mint = mint.expect("an nft_mint event");
    assert!(
        mint.contains(r#""event":"nft_mint""#) && mint.contains(r#""token_ids":["son"]"#),
        "{mint}"
    );

    let removed = s
        .grantor
        .call(s.estate.id(), "remove_heir")
        .args_json(json!({ "label": "son" }))
        .transact()
        .await?;
    let burn = removed
        .logs()
        .into_iter()
        .find(|l| l.contains(r#""standard":"nep171""#))
        .map(String::from);
    removed.into_result()?;
    assert!(
        burn.expect("an nft_burn event")
            .contains(r#""event":"nft_burn""#)
    );
    Ok(())
}
