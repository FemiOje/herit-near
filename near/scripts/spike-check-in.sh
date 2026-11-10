#!/usr/bin/env bash
# Checkpoint 5 spike: send a Selfie Check result to the OutLayer worker, then submit the attestation
# it signs as `check_in` on the test estate. See herit-docs/checkpoints/05-outlayer-selfie-check.md.
#
#   export OUTLAYER_PAYMENT_KEY=...                      # never commit it
#   export OUTLAYER_PROJECT=<owner>.testnet/herit-attestor
#   near/scripts/spike-check-in.sh near/scripts/spike-input.json
#
# spike-input.json is what the dev page at /dev/selfie shows after a Selfie Check.
set -euo pipefail

INPUT=${1:?usage: spike-check-in.sh <worker input json>}
API=${OUTLAYER_API:-https://testnet-api.outlayer.ai}
: "${OUTLAYER_PROJECT:?set OUTLAYER_PROJECT}"
: "${OUTLAYER_PAYMENT_KEY:?set OUTLAYER_PAYMENT_KEY}"

ESTATE=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["input"]["estate_id"])' "$INPUT")
SUBJECT=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["input"]["subject"])' "$INPUT")

echo "== attest via OutLayer ($OUTLAYER_PROJECT)"
# Never retry blindly: a failed-looking call may already be queued and charged.
REPLY=$(curl -sS -X POST "$API/call/$OUTLAYER_PROJECT" \
  -H "X-Payment-Key: $OUTLAYER_PAYMENT_KEY" -H 'Content-Type: application/json' \
  --data-binary @"$INPUT")
ARGS=$(python3 - "$REPLY" <<'PY'
import json, sys
reply = json.loads(sys.argv[1])
if reply.get("status") != "completed":
    sys.exit(f"worker did not complete: {json.dumps(reply)[:500]}")
out = reply["output"]
out = json.loads(out) if isinstance(out, str) else out
if "error" in out:
    sys.exit(f"worker refused: {out['error']}")
print(json.dumps({"attestation": out["attestation"], "signature": out["signature"]}))
PY
)
echo "$ARGS"

echo "== check_in on $ESTATE as $SUBJECT"
near contract call-function as-transaction "$ESTATE" check_in json-args "$ARGS" \
  prepaid-gas '100 Tgas' attached-deposit '0 NEAR' \
  sign-as "$SUBJECT" network-config testnet sign-with-keychain send

echo "== estate after"
near contract call-function as-read-only "$ESTATE" status_of json-args '{}' network-config testnet now
near contract call-function as-read-only "$ESTATE" commitment_of json-args '{}' network-config testnet now
