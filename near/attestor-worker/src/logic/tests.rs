use std::cell::Cell;

use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};

use super::*;

const NOW: u64 = 1_700_000_000 * 1_000_000_000;
const ESTATE: &str = "alice.herit-near.testnet";
const GRANTOR: &str = "alice-kaykay.testnet";

/// Same payloads and hex as `herit-estate/src/golden.rs`, which JS `borsh@2.0.0` and a Python
/// encoder also produced.
const CHECK_IN_HEX: &str = "0d00000068657269742d6e6561723a763118000000616c6963652e68657269742d6e6561722e746573746e657414000000616c6963652d6b61796b61792e746573746e6574000001010101010101010101010101010101010101010101010101010101010101012a00000000000000002858c2cf9d9717";
const CLAIM_HEX: &str = "0d00000068657269742d6e6561723a763118000000616c6963652e68657269742d6e6561722e746573746e65741c000000736f6e2e616c6963652e68657269742d6e6561722e746573746e6574010103000000736f6e0202020202020202020202020202020202020202020202020202020202020202ffffffffffffffff002858c2cf9d9717";

/// Stands in for World and the TEE.
struct Fake {
    status: u16,
    reply: Value,
    attestor: SigningKey,
    salt_key: SigningKey,
    world_calls: Cell<u32>,
}

impl Fake {
    fn accepting() -> Self {
        Self {
            status: 200,
            reply: json!({ "success": true }),
            attestor: SigningKey::from_bytes(&[7; 32]),
            salt_key: SigningKey::from_bytes(&[9; 32]),
            world_calls: Cell::new(0),
        }
    }
    fn replying(status: u16, reply: Value) -> Self {
        Self {
            status,
            reply,
            ..Self::accepting()
        }
    }
}

impl Host for Fake {
    fn verify_with_world(&self, _body: &[u8]) -> Result<(u16, Value), String> {
        self.world_calls.set(self.world_calls.get() + 1);
        Ok((self.status, self.reply.clone()))
    }
    fn sign(&self, path: &str, message: &[u8]) -> Result<Vec<u8>, String> {
        let key = match path {
            "attestor" => &self.attestor,
            "commitment-salt" => &self.salt_key,
            _ => return Err(format!("undeclared key {path}")),
        };
        Ok(key.sign(message).to_bytes().to_vec())
    }
    fn now_ns(&self) -> Result<u64, String> {
        Ok(NOW)
    }
}

fn selfie(nullifier: &str) -> Value {
    json!({ "identifier": "selfie", "nullifier": nullifier })
}

fn result_with(responses: Value) -> Value {
    json!({ "protocol_version": "3.0", "action": "herit:alice", "responses": responses })
}

fn input(action: u8, heir_label: Option<&str>, result: Value) -> AttestInput {
    AttestInput {
        estate_id: ESTATE.into(),
        subject: GRANTOR.into(),
        action,
        heir_label: heir_label.map(Into::into),
        result,
    }
}

fn check_in_input() -> AttestInput {
    input(ACTION_CHECKIN, None, result_with(json!([selfie("0xABC")])))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------- golden ----------

#[test]
fn payload_matches_the_contract_golden_bytes() {
    let encode = |subject: &str, action, label: Option<&str>, face, nonce| {
        hex(&borsh::to_vec(&SignedPayload {
            domain: DOMAIN.into(),
            estate_id: ESTATE.into(),
            subject: subject.into(),
            action,
            heir_label: label.map(Into::into),
            commitment: [face; 32],
            nonce,
            expiry_ns: 1_700_000_900_000_000_000,
        })
        .unwrap())
    };
    assert_eq!(encode(GRANTOR, 0, None, 1, 42), CHECK_IN_HEX);
    assert_eq!(
        encode("son.alice.herit-near.testnet", 1, Some("son"), 2, u64::MAX),
        CLAIM_HEX
    );
}

// ---------- Selfie Check only ----------

#[test]
fn a_single_selfie_check_is_accepted() {
    assert_eq!(
        selfie_nullifier(&result_with(json!([selfie("0xAbC ")]))).unwrap(),
        "0xabc"
    );
}

#[test]
fn the_legacy_face_identifier_is_accepted() {
    let r = result_with(json!([{ "identifier": "face", "nullifier": "0x1" }]));
    assert_eq!(selfie_nullifier(&r).unwrap(), "0x1");
}

#[test]
fn a_proof_with_no_responses_is_refused() {
    assert!(selfie_nullifier(&json!({ "action": "herit:alice" })).is_err());
    let err = selfie_nullifier(&result_with(json!([]))).unwrap_err();
    assert!(err.contains("exactly one"), "{err}");
}

#[test]
fn another_credential_is_refused() {
    let err = selfie_nullifier(&result_with(
        json!([{ "identifier": "orb", "nullifier": "0x1" }]),
    ))
    .unwrap_err();
    assert!(err.contains("orb"), "{err}");
}

#[test]
fn a_selfie_check_bundled_with_another_credential_is_refused() {
    let r = result_with(json!([selfie("0x1"), { "identifier": "orb", "nullifier": "0x2" }]));
    let err = selfie_nullifier(&r).unwrap_err();
    assert!(
        err.contains("exactly one") && err.contains("selfie, orb"),
        "{err}"
    );
}

#[test]
fn a_selfie_check_without_a_nullifier_is_refused() {
    let r = result_with(json!([{ "identifier": "selfie", "nullifier": "  " }]));
    assert!(selfie_nullifier(&r).is_err());
}

#[test]
fn world_is_not_called_for_a_proof_that_is_not_a_selfie_check() {
    let host = Fake::accepting();
    let bad = input(
        ACTION_CHECKIN,
        None,
        result_with(json!([{ "identifier": "orb", "nullifier": "0x1" }])),
    );
    assert!(attest(bad, &host).is_err());
    assert_eq!(host.world_calls.get(), 0);
}

// ---------- input rules ----------

#[test]
fn the_world_action_comes_from_the_estate_id() {
    assert_eq!(world_action(&check_in_input()).unwrap(), "herit:alice");
}

#[test]
fn bad_inputs_are_refused() {
    let r = || result_with(json!([selfie("0x1")]));
    let cases = [
        input(3, None, r()),                      // no such action
        input(ACTION_CHECKIN, Some("son"), r()),  // label on a check-in
        input(ACTION_WITHDRAW, Some("son"), r()), // label on a withdraw
        input(ACTION_CLAIM, None, r()),           // claim without a label
        input(ACTION_CLAIM, Some("so.n"), r()),   // label with a dot
        input(ACTION_CLAIM, Some("Son"), r()),    // uppercase label
    ];
    for case in cases {
        assert!(world_action(&case).is_err(), "{case:?}");
    }
}

#[test]
fn estates_outside_the_factory_are_refused() {
    for estate_id in [
        "bob.testnet",
        "x.alice.herit-near.testnet",
        ".herit-near.testnet",
        "herit-near.testnet",
    ] {
        let mut i = check_in_input();
        i.estate_id = estate_id.into();
        assert!(world_action(&i).is_err(), "{estate_id}");
    }
}

// ---------- World's verdict ----------

#[test]
fn world_rejections_are_refused() {
    let proof = result_with(json!([selfie("0x1")]));
    let refused = [
        (
            400,
            json!({ "code": "invalid_proof", "detail": "bad proof" }),
        ),
        (200, json!({ "code": "verification_failed" })),
        (200, json!({ "success": false })),
        (200, Value::Null),
        (500, json!({ "success": true })),
    ];
    for (status, reply) in refused {
        assert!(
            check_verified(status, &reply, &proof, "herit:alice").is_err(),
            "{status} {reply}"
        );
    }
    assert!(check_verified(200, &json!({ "success": true }), &proof, "herit:alice").is_ok());
}

#[test]
fn a_proof_for_another_action_is_refused() {
    let ok = json!({ "success": true });
    let other = json!({ "action": "herit:bob", "responses": [selfie("0x1")] });
    assert!(check_verified(200, &ok, &other, "herit:alice").is_err());
    let missing = json!({ "responses": [selfie("0x1")] });
    assert!(check_verified(200, &ok, &missing, "herit:alice").is_err());
    let reply_other = json!({ "success": true, "action": "herit:bob" });
    let proof = result_with(json!([selfie("0x1")]));
    assert!(check_verified(200, &reply_other, &proof, "herit:alice").is_err());
}

// ---------- commitment ----------

#[test]
fn one_human_gets_one_commitment_and_two_humans_get_two() {
    let host = Fake::accepting();
    let salt = host.sign("commitment-salt", SALT_MESSAGE).unwrap();
    assert_eq!(commitment(&salt, "0xabc"), commitment(&salt, "0xabc"));
    assert_ne!(commitment(&salt, "0xabc"), commitment(&salt, "0xabd"));
    // A different salt key (another deployment) gives unrelated commitments.
    let other_salt = SigningKey::from_bytes(&[10; 32])
        .sign(SALT_MESSAGE)
        .to_bytes();
    assert_ne!(commitment(&salt, "0xabc"), commitment(&other_salt, "0xabc"));
}

// ---------- end to end: the estate contract accepts what the worker signs ----------

#[test]
fn the_estate_contract_accepts_the_workers_attestation() {
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;

    let host = Fake::accepting();
    let out = attest(check_in_input(), &host).expect("attest");

    let context = |caller: &str, now: u64| {
        VMContextBuilder::new()
            .current_account_id(ESTATE.parse().unwrap())
            .predecessor_account_id(caller.parse().unwrap())
            .block_timestamp(now)
            .build()
    };
    testing_env!(context("herit-near.testnet", 0));
    let pk = near_public_key(&host.attestor.verifying_key().to_bytes());
    let mut estate =
        herit_estate::Estate::new(GRANTOR.parse().unwrap(), pk, 30 * 86_400, 7 * 86_400);

    testing_env!(context(GRANTOR, NOW + 60 * 1_000_000_000)); // a minute later
    let attestation = serde_json::from_value(out["attestation"].clone()).unwrap();
    let signature = serde_json::from_value(out["signature"].clone()).unwrap();
    estate.check_in(attestation, signature);

    assert_eq!(estate.status_of(), herit_estate::Status::Active);
    assert_eq!(
        estate.commitment_of().map(|c| B64.encode(c.0)),
        out["attestation"]["commitment"].as_str().map(String::from)
    );
}

#[test]
fn a_claim_attestation_carries_the_heir_label() {
    let host = Fake::accepting();
    let mut i = input(
        ACTION_CLAIM,
        Some("son"),
        result_with(json!([selfie("0x2")])),
    );
    i.subject = "son.alice.herit-near.testnet".into();
    let out = attest(i, &host).unwrap();
    assert_eq!(out["attestation"]["action"], json!(1));
    assert_eq!(out["attestation"]["heir_label"], json!("son"));
}

#[test]
fn nothing_is_signed_when_world_rejects_the_proof() {
    let host = Fake::replying(400, json!({ "code": "invalid_proof" }));
    let err = attest(check_in_input(), &host).unwrap_err();
    assert!(err.contains("invalid_proof"), "{err}");
    assert_eq!(host.world_calls.get(), 1);
}

#[test]
fn only_world_id_3_proofs_are_accepted() {
    let mut v4 = result_with(json!([selfie("0x1")]));
    v4["protocol_version"] = json!("4.0");
    let err = selfie_nullifier(&v4).unwrap_err();
    assert!(err.contains("4.0"), "{err}");
    let mut missing = result_with(json!([selfie("0x1")]));
    missing.as_object_mut().unwrap().remove("protocol_version");
    assert!(selfie_nullifier(&missing).is_err());
}
