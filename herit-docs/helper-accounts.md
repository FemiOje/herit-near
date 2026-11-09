# Helper accounts

Think of Herit as a small company. `herit-near.testnet` is the head office, and these five accounts are support staff for building and demoing it. **Real users never touch any of them.**

## `herit-code-kaykay.testnet`: the master copy of the estate code

Every will (estate) runs the same program. Instead of storing a separate copy for each estate, this account publishes the code once on NEAR as a "Global Contract", and every estate points to it. It's like one master blueprint that every house is built from.

## `mock-usdc-kaykay.testnet`: our pretend dollar

This account runs a USDC-style test token that we control. In the demo, Alice leaves pretend dollars to an heir, which shows Herit can pass on stablecoins as well as NEAR. It works like a MockUSDC contract in Foundry, and we can mint as much as we need.

## `alice-kaykay.testnet`: our pretend customer

This is a normal wallet that plays the person writing a will in the demo. When it opens an estate, Herit creates `alice.herit-near.testnet`, which is a separate account from this one:

- `alice-kaykay.testnet` is **Alice's wallet**, the person.
- `alice.herit-near.testnet` is **Alice's estate**, the will and the money held in it.

## `herit-funder1-kaykay.testnet` and `herit-funder2-kaykay.testnet`: spare money

These exist only to hold extra testnet NEAR. Publishing the master copy permanently burns about 10 NEAR per 100 KB of code. If the estate code is around 200 KB, that's about 20 NEAR, but each account only got 10 from the faucet. These two send their NEAR to `herit-code-kaykay.testnet` to cover the difference, and after that they're no longer needed.

## How they fit together

```
herit-near.testnet                  head office: creates a new estate for each customer
  └─ alice.herit-near.testnet       Alice's estate (created when the estate is opened)
       └─ son.alice.herit-near.testnet   the heir (created when the heir is added)

herit-code-kaykay              master copy of the code every estate runs
mock-usdc-kaykay               pretend dollars the estate can hold
alice-kaykay                   the demo customer's wallet
herit-funder1 and 2            top up herit-code-kaykay
```
