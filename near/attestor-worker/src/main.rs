//! Herit attestor: an OutLayer worker. It verifies a World ID Selfie Check with World, derives the
//! human's commitment, and signs the attestation the estate contract checks. Both keys live in
//! OutLayer's TEE, so nobody holds the private halves.
//!
//! Input on stdin, one JSON object:
//!   {"operation":"public_key"}
//!   {"operation":"attest","estate_id":..,"subject":..,"action":0|1|2,"heir_label":..,"result":<IDKit result>}
//! Output on stdout: the result, or {"error": "..."}.

mod logic;

use std::io::{self, Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

mod host {
    wit_bindgen::generate!({ world: "signing-keys-host", path: "wit" });
}
use host::outlayer::signing_keys::api as keys;

// The manifest travels inside the wasm, so it is covered by the wasm's hash.
#[cfg(target_arch = "wasm32")]
#[used]
#[link_section = "outlayer.manifest"]
static OUTLAYER_MANIFEST: [u8; include_bytes!("../manifest.json").len()] =
    *include_bytes!("../manifest.json");

/// `WORLD_RP_ID`. Public, so it lives in the code: the worker never reads its environment,
/// because with a `caller: "none"` key the caller can supply it.
const RP_ID: &str = "rp_d58f1c7f1d94ac9b";
const VERIFY_BASE: &str = "https://developer.world.org/api/v4/verify";

fn main() {
    let out = run().unwrap_or_else(|error| json!({ "error": error }));
    print!("{out}");
    io::stdout().flush().expect("flush");
}

fn run() -> Result<Value, String> {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .map_err(|e| e.to_string())?;
    let input: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    match input["operation"].as_str() {
        Some("public_key") => {
            let pk = keys::public_key("attestor", None)?;
            Ok(json!({ "attestor_pk": logic::near_public_key(&pk) }))
        }
        Some("attest") => logic::attest(
            serde_json::from_value(input).map_err(|e| e.to_string())?,
            &OutLayer,
        ),
        _ => Err("unknown operation".into()),
    }
}

/// The real host: World's verify API over WASI HTTP, and OutLayer's TEE keys.
struct OutLayer;

impl logic::Host for OutLayer {
    fn verify_with_world(&self, body: &[u8]) -> Result<(u16, Value), String> {
        let response = wasi_http_client::Client::new()
            .post(&format!("{VERIFY_BASE}/{RP_ID}"))
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .map_err(|e| format!("world verify: {e}"))?;
        let status = response.status();
        let reply = serde_json::from_slice(&response.body().map_err(|e| e.to_string())?)
            .unwrap_or(Value::Null);
        Ok((status, reply))
    }

    fn sign(&self, path: &str, message: &[u8]) -> Result<Vec<u8>, String> {
        keys::sign(path, None, message)
    }

    fn now_ns(&self) -> Result<u64, String> {
        Ok(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos() as u64)
    }
}
