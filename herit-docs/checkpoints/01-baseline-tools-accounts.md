# Checkpoint 1: Baseline, tools, accounts

Part of the [build plan](../near-build-plan.md). Toolchain pins and account details come from [the deployment guide](../near-deployment-guide.md), §3 and §4.

**Status:** in progress (`herit.testnet` claimed in Meteor on 2026-11-09)
**Who:** you; Claude helps with anything you get stuck on

## Goal

Protect the old code, get the tools to the pinned versions, and own `herit.testnet` before anyone else does.

## Understand first

- **Two tags separate old work from new.** The rules compare `pre-hackathon` with `hackathon-submission`, and only the difference between them is judged.
- **A NEAR account is a name plus access keys.** Unlike an Ethereum address, the name is the identity, and its keys can be added or removed.
- **`save-to-keychain` stores the key in macOS Keychain,** the equivalent of a Foundry keystore.
- **If the CLI warns that it fell back to the legacy keychain, the key is in a plaintext file.** Stop and sort that out before creating more accounts.
- **Testnet names are first-come.** If someone takes `herit.testnet`, every name below it changes, so claim it first.

## Steps

### Baseline

```bash
git tag pre-hackathon 55115bf
git push origin pre-hackathon
```

Then add an MIT `LICENSE` at the repo root.

### Tools

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/near/near-cli-rs/releases/latest/download/near-cli-rs-installer.sh | sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/near/cargo-near/releases/latest/download/cargo-near-installer.sh | sh
rustup target add wasm32-wasip2
npx skills add near/agent-skills
```

The pins are NEAR CLI **0.31.1** and `cargo-near` **0.23.0**. The installers fetch the latest release, so check the versions afterwards.

### `herit.testnet`: done in Meteor, two follow-ups

The account exists with one full-access key (`ed25519:3fK6Wori2bZemzCHdXGnWoHVRcjzk58yLU2MCpaRDjmx`). On 2026-11-09 its balance was **0 NEAR**.

1. **Get the key into the CLI.** Deploys are signed from the CLI, so run the import with no further arguments and answer the prompts:

   ```bash
   near account import-account
   ```

   Choose `using-seed-phrase` or `using-private-key`, depending on what Meteor exports, then save to Keychain. Enter the secret only at the hidden prompt. Don't pass it as a command argument, because that writes it to shell history. Never paste it into chat.

2. **Fund it.** Send testnet NEAR from [near-faucet.io](https://near-faucet.io) or from a helper account. About 5 NEAR is a rough starting amount; the real need depends on the factory's wasm size.

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

Don't create the heir accounts (`son.alice.herit.testnet`, `daughter.alice.herit.testnet`). The estate contract creates them through `add_heir`.

### Create the mainnet payout account

Create it in a wallet such as HOT, Meteor or Intear. It's needed for the submission form, not the build. Write it down and never reuse it as a deployer. Claude never creates or invents this account.

## Tests

None. This checkpoint writes no contract code, so test layers start with the first contract checkpoint.

## Check

```bash
git ls-remote --tags origin
near --version && cargo near --version
rustup target list --installed
near account view-account-summary herit.testnet network-config testnet now
test ! -e ~/.near-credentials && echo "no plaintext credentials"
```

## Verified at the stop

- [ ] `pre-hackathon` is on the remote and points at `55115bf`
- [ ] `LICENSE` exists
- [ ] NEAR CLI is 0.31.1, `cargo-near` is 0.23.0, and `wasm32-wasip2` is installed
- [x] `herit.testnet` exists
- [ ] `herit.testnet` has a balance and its key is in Keychain
- [ ] The helper accounts exist
- [ ] The mainnet payout account exists
- [ ] `~/.near-credentials` does not exist
