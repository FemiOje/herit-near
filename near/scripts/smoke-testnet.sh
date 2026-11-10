#!/usr/bin/env bash
# Checkpoint 10 smoke run: one estate through its whole life on testnet, every step a real
# transaction. See herit-docs/checkpoints/10-factory-testnet.md, Part 4.
#
#   KEYS_DIR=/somewhere/outside/git near/scripts/smoke-testnet.sh [label]
#
# Attestations come from the dev signer (frontend/scripts/sign-attestation.mjs) with
# ATTESTOR_SECRET_KEY from frontend/.env.local, which is only ever passed to the signer, never
# printed. Each heir gets a fresh key in $KEYS_DIR; only its public half is shown.
set -euo pipefail

NET=testnet
FACTORY=herit-near.testnet
GRANTOR=alice-kaykay.testnet
TOKEN=mock-usdc-kaykay.testnet
LABEL=${1:-alice}
ESTATE=$LABEL.$FACTORY
KEYS_DIR=${KEYS_DIR:?set KEYS_DIR to a folder outside git for the heirs keys}
ROOT=$(cd "$(dirname "$0")/../.." && pwd)

step() { printf '\n== %s\n' "$*"; }

# Runs a near CLI command that sends a transaction; prints its id and Herit events, fails loudly.
send() {
  local out
  if ! out=$("$@" 2>&1); then
    echo "$out" | grep -vE '^\s*$' | tail -15
    exit 1
  fi
  echo "$out" | grep -o 'Transaction ID: [A-Za-z0-9]*' | tail -1 | sed 's/Transaction ID: /  tx /'
  echo "$out" | grep -o '"event":"[a-z_]*"' | sed 's/"event":/  event /' | tr -d '"' || true
}

view() {
  local args='{}'
  [ $# -ge 3 ] && args=$3
  near contract call-function as-read-only "$1" "$2" json-args "$args" network-config $NET now 2>/dev/null |
    grep -vE '^\s*$' | grep -v 'console command' | grep -v '\.bin_real' | tr -s ' \n' ' '
  echo
}

# A dev attestation; ATTESTOR_SECRET_KEY goes only into the signer's environment.
attest() {
  (cd "$ROOT/frontend" &&
    ATTESTOR_SECRET_KEY=$(grep -E '^ATTESTOR_SECRET_KEY=' .env.local | cut -d= -f2-) \
      node scripts/sign-attestation.mjs "$@")
}

# call <method> <args> <deposit> <gas> <signer> <how to sign...>: a call on the estate.
call() {
  local method=$1 args=$2 deposit=$3 gas=$4 signer=$5
  shift 5
  send near contract call-function as-transaction "$ESTATE" "$method" json-args "$args" \
    prepaid-gas "$gas" attached-deposit "$deposit" sign-as "$signer" network-config $NET "$@"
}

grantor=("$GRANTOR" sign-with-keychain send)

step "1. $GRANTOR opens $ESTATE through the factory (120 s interval, 60 s grace)"
send near contract call-function as-transaction $FACTORY open_estate \
  json-args "{\"label\":\"$LABEL\",\"check_in_interval_sec\":120,\"grace_sec\":60}" \
  prepaid-gas '100 Tgas' attached-deposit '1 NEAR' sign-as $GRANTOR network-config $NET sign-with-keychain send
view $FACTORY estates_of "{\"grantor\":\"$GRANTOR\"}"

step "2. Two heirs, each with a key only they hold"
for HEIR in son:6000 daughter:4000; do
  NAME=${HEIR%%:*} BPS=${HEIR##*:}
  PK=$(cd "$ROOT/frontend" && node --input-type=module -e "
    import { KeyPair } from 'near-api-js';
    import { writeFileSync } from 'node:fs';
    const k = KeyPair.fromRandom('ed25519');
    writeFileSync('$KEYS_DIR/$NAME.json', JSON.stringify({
      account_id: '$NAME.$ESTATE', public_key: k.getPublicKey().toString(), private_key: k.toString(),
    }), { mode: 0o600 });
    console.log(k.getPublicKey().toString());")
  echo "  $NAME.$ESTATE gets $PK"
  call add_heir "{\"label\":\"$NAME\",\"public_key\":\"$PK\",\"relationship\":\"$NAME\",\"default_bps\":$BPS}" \
    '0.2 NEAR' '100 Tgas' "${grantor[@]}"
done

step "3. Deposit 3 NEAR"
call deposit '{}' '3 NEAR' '30 Tgas' "${grantor[@]}"

step "4. Add mock USDC and deposit 10 mUSDC"
call add_token "{\"token\":\"$TOKEN\"}" '0.05 NEAR' '100 Tgas' "${grantor[@]}"
send near contract call-function as-transaction $TOKEN ft_transfer_call \
  json-args "{\"receiver_id\":\"$ESTATE\",\"amount\":\"10000000\",\"msg\":\"\"}" \
  prepaid-gas '100 Tgas' attached-deposit '1 yoctoNEAR' sign-as $GRANTOR network-config $NET sign-with-keychain send

step "5. The son gets 25% of the mUSDC instead of his 60% default"
call set_share "{\"label\":\"son\",\"asset\":\"$TOKEN\",\"bps\":2500}" '0.01 NEAR' '30 Tgas' "${grantor[@]}"

step "6. The grantor's first check-in binds their face and starts the clock"
call check_in "$(attest --estate "$ESTATE" --subject $GRANTOR --action 0 --human "$LABEL")" \
  '0 NEAR' '30 Tgas' "${grantor[@]}"

step "7. The grantor takes 0.5 NEAR back, with a fresh Selfie Check"
ARGS=$(attest --estate "$ESTATE" --subject $GRANTOR --action 2 --human "$LABEL" |
  node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{const a=JSON.parse(s);a.asset=null;a.amount="500000000000000000000000";console.log(JSON.stringify(a))})')
call withdraw "$ARGS" '1 yoctoNEAR' '100 Tgas' "${grantor[@]}"

step "8. Before the deadline"
echo -n "  status:  "; view "$ESTATE" status_of
echo -n "  NEAR:    "; view "$ESTATE" balance_of '{"asset":null}'
echo -n "  mUSDC:   "; view "$ESTATE" balance_of "{\"asset\":\"$TOKEN\"}"
echo -n "  son NFT: "; view "$ESTATE" nft_token '{"token_id":"son"}' | grep -o '\\"state\\":\\"[a-z]*' | tr -d '\\"'

step "9. The grantor stops checking in. Waiting for the clock to say Unlocked"
until view "$ESTATE" status_of | grep -q Unlocked; do sleep 15; echo -n "."; done
echo " unlocked"
echo -n "  son NFT: "; view "$ESTATE" nft_token '{"token_id":"son"}' | grep -o '\\"state\\":\\"[a-z]*' | tr -d '\\"'

step "10. Each heir claims from their own account, with their own Selfie Check"
NONCE_ARGS() { attest --estate "$ESTATE" --subject "$1.$ESTATE" --action 1 --human "$1" --label "$1" |
  node -e "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>{const a=JSON.parse(s);a.label='$1';console.log(JSON.stringify(a))})"; }
for NAME in son daughter; do
  echo "  $NAME:"
  # 50 Tgas, not 100: a signer must hold the attached gas at NEAR's pessimistic price (about
  # 0.1 NEAR per 100 Tgas) up front, and the heir's account holds its 0.1 NEAR seed.
  call claim "$(NONCE_ARGS $NAME)" '0.01 NEAR' '50 Tgas' "$NAME.$ESTATE" \
    sign-with-access-key-file "$KEYS_DIR/$NAME.json" send
done

step "11. After"
sleep 5
for NAME in son daughter; do
  echo -n "  $NAME mUSDC: "; view $TOKEN ft_balance_of "{\"account_id\":\"$NAME.$ESTATE\"}"
  echo -n "  $NAME NFT:   "; view "$ESTATE" nft_token "{\"token_id\":\"$NAME\"}" | grep -o '\\"state\\":\\"[a-z]*' | tr -d '\\"'
done
echo -n "  estate NEAR left:  "; view "$ESTATE" balance_of '{"asset":null}'
echo -n "  estate mUSDC left: "; view "$ESTATE" balance_of "{\"asset\":\"$TOKEN\"}"
echo -n "  snapshot NEAR:     "; view "$ESTATE" snapshot_of '{"asset":null}'
