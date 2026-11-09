# Checkpoint 1: Baseline, tools, accounts

Part of the [build plan](../near-build-plan.md). Toolchain pins and account details come from [the deployment guide](../near-deployment-guide.md), §3 and §4.

**Status:** done (2026-11-09)
**Who:** you; Claude helps with anything you get stuck on

## Goal

Protect the old code, get the tools to the pinned versions, and own `herit-near.testnet` before anyone else does.

## Understand first

- **Two tags separate old work from new.** The rules compare `pre-hackathon` with `hackathon-submission`, and only the difference between them is judged.
- **A NEAR account is a name plus access keys.** Unlike an Ethereum address, the name is the identity, and its keys can be added or removed.
- **`save-to-keychain` stores the key in macOS Keychain,** the equivalent of a Foundry keystore.
- **If the CLI warns that it fell back to the legacy keychain, the key is in a plaintext file.** Stop and sort that out before creating more accounts.
- **Testnet names are first-come.** If someone takes `herit-near.testnet`, every name below it changes, so claim it first.

## Steps

### Baseline

```bash
git tag pre-hackathon 55115bf
git push origin pre-hackathon
```

Then add an MIT `LICENSE` at the repo root.

### Tools

```bash
npm install -g near-cli-rs@0.31.1   # the current 0.30.1 is an npm global, so upgrade it the same way
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/near/cargo-near/releases/latest/download/cargo-near-installer.sh | sh
rustup target add wasm32-wasip2
npx skills add near/agent-skills
```

The pins are NEAR CLI **0.31.1** and `cargo-near` **0.23.0**. The `cargo-near` installer fetches the latest release, so check the versions afterwards.

### The factory account: `herit-near.testnet`

The factory must be deployed by the CLI, so its account has to be created by the CLI. A sub-account can only be created by its parent, so a factory hosted elsewhere couldn't create `alice.<name>` under a wallet-owned name.

The account was created and funded from a helper, because the faucet returned `429 Too Many Requests` after several accounts in one day:

```bash
near account create-account fund-myself herit-near.testnet '5 NEAR' autogenerate-new-keypair \
  save-to-keychain sign-as herit-funder1-kaykay.testnet network-config testnet sign-with-keychain send
```

**Why not the first name, `herit` + `.testnet`, in Meteor?** Meteor blocks websites from adding a full-access key and from deploying a contract (both tested on 2026-11-09, Approve disabled), and we chose not to export a Meteor secret. That account stays in Meteor, unused, with 25 NEAR that can be sent to any helper with a normal transfer.

### Create the helper accounts

Pick one team suffix and use it everywhere in place of `xyz`.

```bash
export NET=testnet
export CODE=herit-code-xyz.testnet        # publishes the global estate code
export TOKEN=mock-usdc-xyz.testnet        # NEP-141 test token
export GRANTOR=alice-xyz.testnet          # demo grantor's wallet
export FUNDER1=herit-funder1-xyz.testnet  # spare faucet account for topping up $CODE
export FUNDER2=herit-funder2-xyz.testnet  # spare faucet account for topping up $CODE

for A in $CODE $TOKEN $GRANTOR $FUNDER1 $FUNDER2; do
  near account create-account sponsor-by-faucet-service "$A" \
    autogenerate-new-keypair save-to-keychain network-config $NET create
done
```

Don't create the heir accounts (`son.alice.herit-near.testnet`, `daughter.alice.herit-near.testnet`). The estate contract creates them through `add_heir`.

### Create the mainnet payout account

Create it in a wallet such as HOT, Meteor or Intear. It's needed for the submission form, not the build. Write it down and never reuse it as a deployer. Claude never creates or invents this account.

## Tests

None. This checkpoint writes no contract code, so test layers start with the first contract checkpoint.

## Check

```bash
git ls-remote --tags origin
near --version && cargo near --version
rustup target list --installed
near account view-account-summary herit-near.testnet network-config testnet now
grep -rqs private_key ~/.near-credentials || echo "no plaintext keys"   # the CLI keeps non-secret bookkeeping files in this folder
```

## Verified at the stop

- [x] `pre-hackathon` is on the remote and points at `55115bf`
- [x] `LICENSE` exists
- [x] NEAR CLI is 0.31.1, `cargo-near` is 0.23.0, and `wasm32-wasip2` is installed
- [x] `herit-near.testnet` exists
- [x] `herit-near.testnet` has a balance and its key is in Keychain
- [x] The helper accounts exist (suffix `kaykay`, keys in Keychain)
- [x] The mainnet payout account exists
- [x] No private keys in `~/.near-credentials` (the CLI's `accounts.json` and `ft_contracts.json` are expected there)
