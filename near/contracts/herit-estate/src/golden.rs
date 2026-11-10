//! Golden test: pins the exact Borsh bytes of `SignedPayload`.
//!
//! The signers (the OutLayer worker and the JS dev signer) must produce these same bytes, or every
//! signature fails with "invalid signature". The hex below was produced independently by
//! `borsh@2.0.0` in JS, using the schema from guide §7.5, and by a hand-written Python encoder;
//! both agreed on 2026-11-10. The JS dev signer's own test asserts the same hex.

use super::*;

/// action 0 (check-in), no heir label, commitment [1; 32], nonce 42.
const CHECK_IN_HEX: &str = "0d00000068657269742d6e6561723a763118000000616c6963652e68657269742d6e6561722e746573746e657414000000616c6963652d6b61796b61792e746573746e6574000001010101010101010101010101010101010101010101010101010101010101012a00000000000000002858c2cf9d9717";

/// action 1 (claim), heir label "son", commitment [2; 32], nonce u64::MAX.
const CLAIM_HEX: &str = "0d00000068657269742d6e6561723a763118000000616c6963652e68657269742d6e6561722e746573746e65741c000000736f6e2e616c6963652e68657269742d6e6561722e746573746e6574010103000000736f6e0202020202020202020202020202020202020202020202020202020202020202ffffffffffffffff002858c2cf9d9717";

const EXPIRY_NS: u64 = 1_700_000_900_000_000_000;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn encode(subject: &str, action: u8, heir_label: Option<&str>, face: u8, nonce: u64) -> String {
    hex(&near_sdk::borsh::to_vec(&SignedPayload {
        domain: DOMAIN.into(),
        estate_id: "alice.herit-near.testnet".parse().unwrap(),
        subject: subject.parse().unwrap(),
        action,
        heir_label: heir_label.map(Into::into),
        commitment: [face; 32],
        nonce,
        expiry_ns: EXPIRY_NS,
    })
    .unwrap())
}

#[test]
fn check_in_payload_matches_the_js_encoder() {
    assert_eq!(
        encode("alice-kaykay.testnet", ACTION_CHECKIN, None, 1, 42),
        CHECK_IN_HEX
    );
}

#[test]
fn claim_payload_with_a_label_matches_the_js_encoder() {
    assert_eq!(
        encode("son.alice.herit-near.testnet", 1, Some("son"), 2, u64::MAX),
        CLAIM_HEX
    );
}
