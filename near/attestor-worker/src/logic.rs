//! Everything the attestor decides, kept apart from World's HTTP API and OutLayer's TEE keys so it
//! can be tested natively. `main.rs` plugs in the real ones.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use borsh::BorshSerialize;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const DOMAIN: &str = "herit-near:v1";
/// This worker only attests for estates under the factory.
pub const ESTATE_SUFFIX: &str = ".herit-near.testnet";
/// Under the contract's 30-minute cap.
pub const TTL_NS: u64 = 15 * 60 * 1_000_000_000;
pub const ACTION_CHECKIN: u8 = 0;
pub const ACTION_CLAIM: u8 = 1;
pub const ACTION_WITHDRAW: u8 = 2;
/// Selfie Check's credential identifier in an IDKit result. `face` is its legacy alias.
const SELFIE_IDENTIFIERS: [&str; 2] = ["selfie", "face"];
/// The only World ID protocol version accepted. See `selfie_nullifier`.
const PROTOCOL_VERSION: &str = "3.0";

/// Must match the contract's `SignedPayload` field order and types exactly: Borsh is positional.
/// `tests::payload_matches_the_contract_golden_bytes` pins this.
#[derive(BorshSerialize)]
pub struct SignedPayload {
    pub domain: String,
    pub estate_id: String,
    pub subject: String,
    pub action: u8,
    pub heir_label: Option<String>,
    pub commitment: [u8; 32],
    pub nonce: u64,
    pub expiry_ns: u64,
}

#[derive(Deserialize, Debug)]
pub struct AttestInput {
    pub estate_id: String,
    /// The NEAR account that will send the transaction: the grantor for check-in and withdraw, the
    /// heir's sub-account for a claim. Taken from the input because our server pays for every run,
    /// so the host can't name the user. The contract enforces who may call, so a wrong subject only
    /// produces an attestation nobody can use.
    pub subject: String,
    pub action: u8,
    pub heir_label: Option<String>,
    /// The IDKit result, forwarded to World untouched.
    pub result: Value,
}

/// What the worker needs from the outside world. `main.rs` implements it with World's API and
/// OutLayer's keys; tests implement it with fakes.
pub trait Host {
    /// POSTs `body` to World's verify endpoint. Returns the HTTP status and the parsed reply
    /// (`Value::Null` if it wasn't JSON).
    fn verify_with_world(&self, body: &[u8]) -> Result<(u16, Value), String>;
    /// Signs `message` with the TEE key declared at `path`.
    fn sign(&self, path: &str, message: &[u8]) -> Result<Vec<u8>, String>;
    fn now_ns(&self) -> Result<u64, String>;
}

/// Checks the request and returns the World action the proof must be for, `herit:<estate label>`.
/// Derived here, never taken from the caller, or a proof for another estate would pass.
pub fn world_action(input: &AttestInput) -> Result<String, String> {
    if ![ACTION_CHECKIN, ACTION_CLAIM, ACTION_WITHDRAW].contains(&input.action) {
        return Err("bad action".into());
    }
    match (&input.heir_label, input.action == ACTION_CLAIM) {
        (Some(label), true) if is_label(label) => {}
        (Some(_), true) => return Err("heir_label must be a single lowercase segment".into()),
        (None, true) | (Some(_), false) => {
            return Err("heir_label is required for a claim and only for a claim".into())
        }
        (None, false) => {}
    }
    if input.subject.is_empty() {
        return Err("subject is required".into());
    }
    let label = input
        .estate_id
        .strip_suffix(ESTATE_SUFFIX)
        .filter(|l| is_label(l))
        .ok_or("estate_id is not an estate of this factory")?;
    Ok(format!("herit:{label}"))
}

fn is_label(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// Returns the Selfie Check nullifier, and refuses any proof that isn't exactly one Selfie Check.
///
/// World's v4 verify reply doesn't list nullifiers per credential, so the nullifier comes from the
/// proof itself. That is only safe if the proof World verifies *is* the Selfie Check: a proof with
/// a second credential could otherwise pass on that one.
///
/// Only World ID 3.0 proofs are accepted, which is all the `selfieCheckLegacy` preset returns. A
/// person's v3 and v4 nullifiers differ, so accepting both would give one human two commitments
/// and two heir slots.
pub fn selfie_nullifier(result: &Value) -> Result<String, String> {
    if result["protocol_version"].as_str() != Some(PROTOCOL_VERSION) {
        return Err(format!(
            "proof must be World ID {PROTOCOL_VERSION} (got: {})",
            result["protocol_version"].as_str().unwrap_or("none")
        ));
    }
    let responses = result["responses"]
        .as_array()
        .ok_or("proof has no responses")?;
    let identifiers: Vec<String> = responses
        .iter()
        .map(|r| r["identifier"].as_str().unwrap_or("?").to_string())
        .collect();
    let [only] = responses.as_slice() else {
        return Err(format!(
            "proof must hold exactly one Selfie Check credential (got: {})",
            if identifiers.is_empty() {
                "none".into()
            } else {
                identifiers.join(", ")
            }
        ));
    };
    if !SELFIE_IDENTIFIERS.contains(&only["identifier"].as_str().unwrap_or("")) {
        return Err(format!(
            "proof must be a Selfie Check (got: {})",
            identifiers.join(", ")
        ));
    }
    only["nullifier"]
        .as_str()
        .filter(|n| !n.trim().is_empty())
        .map(|n| n.trim().to_lowercase())
        .ok_or_else(|| "Selfie Check response has no nullifier".into())
}

/// Fails closed: anything not clearly a success is a rejection.
///
/// The exact success field is undocumented. Tighten this once a real reply has been recorded
/// (Checkpoint 5 pass criteria).
pub fn check_verified(
    status: u16,
    reply: &Value,
    result: &Value,
    action: &str,
) -> Result<(), String> {
    let verified = reply["code"].as_str().is_none()
        && (reply["success"] == json!(true)
            || reply["responses"].as_array().is_some_and(|r| !r.is_empty()));
    if !(200..300).contains(&status) || !verified {
        let detail = reply["detail"]
            .as_str()
            .or(reply["code"].as_str())
            .unwrap_or("no detail");
        return Err(format!("proof rejected (status {status}: {detail})"));
    }
    // The proof must name our action. Nullifiers are per action, so a proof for any other action
    // would give the same human a fresh commitment and let them take a second heir slot.
    if result["action"].as_str() != Some(action) {
        return Err("proof was not issued for this estate's action".into());
    }
    if reply["action"].as_str().is_some_and(|a| a != action) {
        return Err("proof was issued for a different action".into());
    }
    Ok(())
}

/// `sha256("herit-near:commitment:v1" || salt || nullifier)`. The salt is a TEE signature, so
/// nobody can read or swap it, and it stays the same across code upgrades.
pub fn commitment(salt: &[u8], nullifier: &str) -> [u8; 32] {
    Sha256::new()
        .chain_update(b"herit-near:commitment:v1")
        .chain_update(salt)
        .chain_update(nullifier.as_bytes())
        .finalize()
        .into()
}

/// The fixed message the `commitment-salt` key signs to produce the salt.
pub const SALT_MESSAGE: &[u8] = b"herit-near:commitment-salt:v1";

/// The whole `attest` operation.
pub fn attest(input: AttestInput, host: &impl Host) -> Result<Value, String> {
    let action = world_action(&input)?;
    // Checked before calling World, so a wrong credential costs nothing.
    let nullifier = selfie_nullifier(&input.result)?;

    let body = serde_json::to_vec(&input.result).map_err(|e| e.to_string())?;
    let (status, reply) = host.verify_with_world(&body)?;
    check_verified(status, &reply, &input.result, &action)?;

    let salt = host.sign("commitment-salt", SALT_MESSAGE)?;
    let commitment = commitment(&salt, &nullifier);

    let now = host.now_ns()?;
    let expiry_ns = now + TTL_NS;
    // Unique per proof and per moment, which is all a replay guard needs.
    let digest = Sha256::new()
        .chain_update(input.result.to_string())
        .chain_update(now.to_le_bytes())
        .finalize();
    let nonce = u64::from_le_bytes(digest[..8].try_into().expect("8 bytes"));

    let message = borsh::to_vec(&SignedPayload {
        domain: DOMAIN.into(),
        estate_id: input.estate_id,
        subject: input.subject,
        action: input.action,
        heir_label: input.heir_label.clone(),
        commitment,
        nonce,
        expiry_ns,
    })
    .map_err(|e| e.to_string())?;
    let signature = host.sign("attestor", &message)?;

    Ok(json!({
        "attestation": {
            "action": input.action,
            "heir_label": input.heir_label,
            "commitment": B64.encode(commitment),
            "nonce": nonce.to_string(),
            "expiry_ns": expiry_ns.to_string(),
        },
        "signature": B64.encode(signature),
    }))
}

/// `ed25519:<base58>`, the form the estate's `new` takes.
pub fn near_public_key(bytes: &[u8]) -> String {
    format!("ed25519:{}", bs58::encode(bytes).into_string())
}

#[cfg(test)]
mod tests;
