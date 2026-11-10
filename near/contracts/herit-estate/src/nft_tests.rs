//! Checkpoint 9: the heir credential, a soulbound dynamic NEP-171 token.

use near_sdk::json_types::U128;
use near_sdk::serde_json::{self, Value};
use proptest::prelude::*;

use super::testing::*;
use super::*;

fn extra(e: &Estate, label: &str) -> Value {
    let token = e.nft_token(label.into()).expect("a credential");
    serde_json::from_str(&token.metadata.unwrap().extra.unwrap()).unwrap()
}

fn state(e: &Estate, label: &str) -> String {
    extra(e, label)["state"].as_str().unwrap().to_string()
}

// ---------- Part 1: the NEP-171 views ----------

#[test]
fn the_collection_is_named_after_the_estate() {
    let e = new_estate();
    let m = e.nft_metadata();
    assert_eq!(m.spec, "nft-1.0.0");
    assert_eq!(m.symbol, "HEIR");
    assert_eq!(m.name, format!("Herit heirs of {}", estate_id()));
}

#[test]
fn a_created_heir_holds_a_credential_owned_by_their_account() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 6_000);
    let token = e.nft_token("son".into()).unwrap();
    assert_eq!(token.token_id, "son");
    assert_eq!(token.owner_id, heir_id("son"));
    assert_eq!(token.approved_account_ids, None, "soulbound: no approvals");
    let m = token.metadata.unwrap();
    assert_eq!(m.title, Some(heir_id("son").to_string()));
    assert_eq!(m.copies, Some(1));
    assert_eq!(
        m.description,
        Some(format!(
            "Heir credential for {} · son · locked",
            estate_id()
        ))
    );
    assert_eq!(
        extra(&e, "son"),
        serde_json::json!({
            "estate": estate_id(), "relationship": "son", "share_bps": 6000, "state": "locked"
        })
    );
}

#[test]
fn a_pending_or_unknown_heir_has_no_credential() {
    let mut e = new_estate();
    add_heir_at(&mut e, "son", 5_000, 0); // account still being created
    assert!(e.nft_token("son".into()).is_none());
    assert!(e.nft_token("nobody".into()).is_none());
    assert_eq!(e.nft_total_supply(), U128(0));
}

#[test]
fn a_removed_heirs_credential_is_gone() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 5_000);
    set_context(grantor(), 0);
    e.remove_heir("son".into());
    assert!(e.nft_token("son".into()).is_none());
    assert_eq!(e.nft_supply_for_owner(heir_id("son")), U128(0));
}

#[test]
fn the_credential_follows_the_estate_locked_grace_claimable_claimed() {
    let (mut e, unlock_at) = funded_estate();
    let (grace_at, _) = e.deadlines_of();

    set_context(stranger(), CHECKED_IN_AT);
    assert_eq!(state(&e, "son"), "locked");
    set_context(stranger(), grace_at.0);
    assert_eq!(state(&e, "son"), "grace", "computed: nobody has poked");
    set_context(stranger(), unlock_at);
    assert_eq!(state(&e, "son"), "claimable");

    claim_with(
        &mut e,
        "son",
        &claim_att("son", SON_FACE, 10, unlock_at),
        unlock_at,
        CLAIM_DEPOSIT,
    );
    assert_eq!(state(&e, "son"), "claimed");
    assert_eq!(state(&e, "daughter"), "claimable", "each heir's own state");
    let description = e
        .nft_token("son".into())
        .unwrap()
        .metadata
        .unwrap()
        .description
        .unwrap();
    assert!(description.ends_with("· claimed"), "{description}");
}

#[test]
fn a_failed_payout_makes_the_credential_claimable_again() {
    let (mut e, unlock_at) = funded_estate();
    claim_with(
        &mut e,
        "son",
        &claim_att("son", SON_FACE, 10, unlock_at),
        unlock_at,
        CLAIM_DEPOSIT,
    );
    set_callback_context(false);
    e.on_paid("son".into(), Some(usdc()), U128(3_000_000));
    set_context(stranger(), unlock_at);
    assert_eq!(state(&e, "son"), "claimable");
}

#[test]
fn an_asset_with_no_share_does_not_hold_the_credential_back() {
    let (mut e, unlock_at) = funded_estate();
    set_context_paying(grantor(), CHECKED_IN_AT, CLAIM_DEPOSIT);
    e.set_share("son".into(), Some(usdc()), 0);
    claim_with(
        &mut e,
        "son",
        &claim_att("son", SON_FACE, 10, unlock_at),
        unlock_at,
        CLAIM_DEPOSIT,
    );
    assert_eq!(
        state(&e, "son"),
        "claimed",
        "his only share, the NEAR, is paid"
    );
}

#[test]
fn an_estate_before_its_first_check_in_reads_locked() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 5_000);
    set_context(stranger(), 10 * 365 * 86_400 * NS_PER_SEC);
    assert_eq!(state(&e, "son"), "locked");
}

#[test]
fn enumeration_lists_created_heirs_only() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 3_000);
    add_created_heir(&mut e, "daughter", 3_000);
    add_heir_at(&mut e, "kid", 3_000, 0); // pending
    let ids: Vec<String> = e
        .nft_tokens(None, None)
        .into_iter()
        .map(|t| t.token_id)
        .collect();
    assert_eq!(ids, vec!["son", "daughter"]);
    assert_eq!(e.nft_total_supply(), U128(2));

    let page: Vec<String> = e
        .nft_tokens(Some(U128(1)), Some(1))
        .into_iter()
        .map(|t| t.token_id)
        .collect();
    assert_eq!(page, vec!["daughter"]);
    assert!(e.nft_tokens(Some(U128(2)), None).is_empty());

    assert_eq!(e.nft_supply_for_owner(heir_id("son")), U128(1));
    assert_eq!(e.nft_supply_for_owner(grantor()), U128(0));
    assert_eq!(e.nft_supply_for_owner(heir_id("kid")), U128(0));
    let owned: Vec<String> = e
        .nft_tokens_for_owner(heir_id("daughter"), None, None)
        .into_iter()
        .map(|t| t.token_id)
        .collect();
    assert_eq!(owned, vec!["daughter"]);
}

#[derive(Debug, Clone)]
enum HeirOp {
    Add(usize),
    Settle(usize, bool),
    Remove(usize),
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Random adds, creations (ok or failed) and removals: the credentials are exactly the
    /// created heirs, each owned by `<label>.<estate>`, every page joins up into the full list,
    /// and each owner holds exactly one.
    #[test]
    fn the_credentials_are_exactly_the_created_heirs(
        ops in prop::collection::vec(prop_oneof![
            (0..6usize).prop_map(HeirOp::Add),
            (0..6usize, prop::bool::weighted(0.8)).prop_map(|(l, ok)| HeirOp::Settle(l, ok)),
            (0..6usize).prop_map(HeirOp::Remove),
        ], 1..40),
        page in 1..4u64,
    ) {
        let labels = ["a", "b", "c", "d", "e", "f"];
        let mut e = new_estate();
        for op in ops {
            match op {
                HeirOp::Add(l) => {
                    call_reverting(&mut e, |e| add_heir_at(e, labels[l], 0, 0));
                }
                HeirOp::Settle(l, ok) => {
                    if e.heir_of(labels[l].into()).is_some_and(|h| !h.created) {
                        set_callback_context(ok);
                        e.on_heir_created(labels[l].into(), U128(0));
                    }
                }
                HeirOp::Remove(l) => {
                    call_reverting(&mut e, |e| {
                        set_context(grantor(), 0);
                        e.remove_heir(labels[l].into());
                    });
                }
            }
            let created: Vec<String> = e.heirs_of().into_iter().filter(|h| h.created).map(|h| h.label).collect();
            let all = e.nft_tokens(None, Some(100));
            let ids: Vec<String> = all.iter().map(|t| t.token_id.clone()).collect();
            prop_assert_eq!(&ids, &created);
            prop_assert_eq!(e.nft_total_supply(), U128(created.len() as u128));
            for t in &all {
                prop_assert_eq!(&t.owner_id, &heir_id(&t.token_id));
                prop_assert_eq!(e.nft_supply_for_owner(t.owner_id.clone()), U128(1));
            }
            let mut paged = vec![];
            let mut from = 0u128;
            loop {
                let chunk = e.nft_tokens(Some(U128(from)), Some(page));
                if chunk.is_empty() { break; }
                from += chunk.len() as u128;
                paged.extend(chunk.into_iter().map(|t| t.token_id));
            }
            prop_assert_eq!(paged, created);
        }
    }
}

// ---------- Part 2: soulbound, and the standard events ----------

use near_sdk::test_utils::get_logs;

/// The standard (non-Herit) events in the logs, parsed.
fn nep171_events() -> Vec<Value> {
    get_logs()
        .iter()
        .filter_map(|l| l.strip_prefix("EVENT_JSON:"))
        .filter_map(|j| serde_json::from_str::<Value>(j).ok())
        .filter(|v| v["standard"] == "nep171")
        .collect()
}

#[test]
fn the_credential_is_minted_when_the_heirs_account_is_created() {
    let mut e = new_estate();
    add_heir_at(&mut e, "son", 5_000, 0);
    assert!(
        nep171_events().is_empty(),
        "nothing is minted before the account exists"
    );
    set_callback_context(true);
    e.on_heir_created("son".into(), U128(0));
    let events = nep171_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"], "nft_mint");
    assert_eq!(
        events[0]["data"],
        serde_json::json!([{ "owner_id": heir_id("son"), "token_ids": ["son"], "memo": "herit heir credential" }])
    );
}

#[test]
fn nothing_is_minted_when_the_account_creation_fails() {
    let mut e = new_estate();
    add_heir_at(&mut e, "son", 5_000, 0);
    set_callback_context(false);
    e.on_heir_created("son".into(), U128(0));
    assert!(nep171_events().is_empty());
}

#[test]
fn removing_an_heir_burns_their_credential() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 5_000);
    set_context(grantor(), 0);
    e.remove_heir("son".into());
    let events = nep171_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event"], "nft_burn");
    assert_eq!(
        events[0]["data"],
        serde_json::json!([{ "owner_id": heir_id("son"), "token_ids": ["son"], "memo": "heir removed" }])
    );
}

/// One call on an estate.
type Call = dyn Fn(&mut Estate);

#[test]
fn every_transfer_and_approval_call_refuses() {
    let calls: Vec<(&str, Box<Call>)> = vec![
        (
            "nft_transfer",
            Box::new(|e| e.nft_transfer(stranger(), "son".into(), None, None)),
        ),
        (
            "nft_transfer_call",
            Box::new(|e| {
                let _ = e.nft_transfer_call(stranger(), "son".into(), None, None, String::new());
            }),
        ),
        (
            "nft_approve",
            Box::new(|e| e.nft_approve("son".into(), stranger(), None)),
        ),
        (
            "nft_revoke",
            Box::new(|e| e.nft_revoke("son".into(), stranger())),
        ),
        (
            "nft_revoke_all",
            Box::new(|e| e.nft_revoke_all("son".into())),
        ),
    ];
    for (name, call) in calls {
        let mut e = new_estate();
        add_created_heir(&mut e, "son", 5_000);
        // From the owner itself, with the 1 yoctoNEAR wallets attach.
        set_context_paying(heir_id("son"), 0, ONE_YOCTO);
        let message = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(&mut e)))
            .expect_err(name)
            .downcast_ref::<String>()
            .cloned()
            .unwrap_or_default();
        assert!(message.contains("soulbound"), "{name}: {message}");
        assert_eq!(
            e.nft_token("son".into()).unwrap().owner_id,
            heir_id("son"),
            "{name}"
        );
    }
}

#[test]
fn nobody_is_ever_approved() {
    let mut e = new_estate();
    add_created_heir(&mut e, "son", 5_000);
    assert!(!e.nft_is_approved("son".into(), stranger(), None));
    assert!(!e.nft_is_approved("son".into(), heir_id("son"), Some(0)));
}

// ---------- Part 3: the images, on IPFS ----------

use super::nft::CREDENTIAL_IMAGES;

/// The four files, read at compile time (tests only: none of this is in the wasm).
const FILES: [(&str, &[u8]); 4] = [
    (
        "locked",
        include_bytes!("../../../assets/credential/locked.svg"),
    ),
    (
        "grace",
        include_bytes!("../../../assets/credential/grace.svg"),
    ),
    (
        "claimable",
        include_bytes!("../../../assets/credential/claimable.svg"),
    ),
    (
        "claimed",
        include_bytes!("../../../assets/credential/claimed.svg"),
    ),
];

/// CIDv1, raw codec, sha2-256, base32 lower: `b` + base32(0x01 0x55 0x12 0x20 ‖ sha256).
fn raw_cid(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut data = vec![0x01, 0x55, 0x12, 0x20];
    data.extend(env::sha256(bytes));
    let (mut out, mut buffer, mut bits) = (String::from("b"), 0u32, 0);
    for byte in data {
        buffer = (buffer << 8) | byte as u32;
        bits += 8;
        while bits >= 5 {
            out.push(ALPHABET[((buffer >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}

#[test]
fn the_images_match_their_files() {
    new_estate(); // a mocked blockchain for env::sha256
    for ((state, cid, hash), (file_state, bytes)) in CREDENTIAL_IMAGES.iter().zip(FILES) {
        assert_eq!(*state, file_state);
        assert_eq!(hash.to_vec(), env::sha256(bytes), "{state}: hash");
        assert_eq!(*cid, raw_cid(bytes), "{state}: CID");
    }
}

#[test]
fn the_cid_encoder_matches_a_known_cid() {
    new_estate();
    // The raw CIDv1 of the empty file, as `ipfs add --raw-leaves --cid-version 1` gives it.
    assert_eq!(
        raw_cid(b""),
        "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku"
    );
}

#[test]
fn wallets_resolve_the_image_through_the_collections_base_uri() {
    let e = new_estate();
    assert_eq!(
        e.nft_metadata().base_uri.as_deref(),
        Some("https://gateway.pinata.cloud/ipfs")
    );
}

#[test]
fn each_state_shows_its_own_image() {
    let (mut e, unlock_at) = funded_estate();
    let (grace_at, _) = e.deadlines_of();
    let image = |e: &Estate| {
        let m = e.nft_token("son".into()).unwrap().metadata.unwrap();
        (m.media.unwrap(), m.media_hash.unwrap().0)
    };
    let expect = |state: &str| {
        let (_, cid, hash) = CREDENTIAL_IMAGES
            .iter()
            .find(|(s, _, _)| *s == state)
            .unwrap();
        (cid.to_string(), hash.to_vec())
    };

    set_context(stranger(), CHECKED_IN_AT);
    assert_eq!(image(&e), expect("locked"));
    set_context(stranger(), grace_at.0);
    assert_eq!(image(&e), expect("grace"));
    set_context(stranger(), unlock_at);
    assert_eq!(image(&e), expect("claimable"));
    claim_with(
        &mut e,
        "son",
        &claim_att("son", SON_FACE, 10, unlock_at),
        unlock_at,
        CLAIM_DEPOSIT,
    );
    assert_eq!(image(&e), expect("claimed"));

    let cids: std::collections::HashSet<&str> =
        CREDENTIAL_IMAGES.iter().map(|(_, c, _)| *c).collect();
    assert_eq!(cids.len(), 4, "four different images");
}
