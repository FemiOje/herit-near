# Heir credential images

One image per credential state. The estate contract returns the one for the heir's current state as the token's `media`, with the file's SHA-256 as `media_hash`, resolved through the collection's `base_uri`, `https://gateway.pinata.cloud/ipfs` (`near/contracts/herit-estate/src/nft.rs`, `CREDENTIAL_IMAGES`).

| State | File | IPFS CID (v1, raw) |
|---|---|---|
| locked | `locked.svg` | `bafkreibqatv5ibnvmkutd3mt3nf3fhwp5ricnatupszbum3yrwkg6pi2k4` |
| grace | `grace.svg` | `bafkreigv6cslco4b2wziwh3ul3blrkiloew2exxtrh6uudjmhfwjtckjhy` |
| claimable | `claimable.svg` | `bafkreianqa2t6ixipd3ipyyqatzvgiqy2swd2as5kxdaym2dqoqgqqxabm` |
| claimed | `claimed.svg` | `bafkreicd2b6s67zdyvediuh57nkbvgqyrq4qvwqg7iuhaizdgd3wanzo2e` |

A raw CIDv1 is the file's SHA-256, so it can only ever name these exact bytes. `nft_tests::the_images_match_their_files` recomputes every CID and hash from the files.

**Changing an image** changes its CID: update `CREDENTIAL_IMAGES` (the test says what to), re-pin, and redeploy. Estates already published as a Global Contract keep the old images.

## Pinning

Pin all four so gateways can serve them. The CID the service reports **must** match the table: a different one (for example a `Qm…` CIDv0) means the service wrapped the file differently, and wallets would fetch an address nobody serves.

- **Pinata:** upload each file with CID version 1. Or, with an IPFS node: `ipfs add --cid-version 1 --raw-leaves <file>`, then pin by CID.
- **Any service that pins by CID** (Filebase, a local `ipfs` node): add with `--cid-version 1 --raw-leaves` and pin.

Check each one loads as an image: `https://gateway.pinata.cloud/ipfs/<cid>` must answer `image/svg+xml`.

**Pinned on Pinata on 2026-11-10**: all four CIDs match, and the gateway serves the exact bytes as `image/svg+xml`.

**Why not `ipfs.io`.** A raw CID carries no file name or type, so a gateway guesses the type. `ipfs.io` redirects browsers to `inbrowser.link`, which showed the SVG as text, and rate-limits other clients (HTTP 429). Wallets draw `media` with an `<img>` tag, which refuses an SVG not labelled `image/svg+xml`. Pinata's and Filebase's gateways label it correctly.
