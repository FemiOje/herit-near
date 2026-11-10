//! The heir credential: one soulbound, dynamic NEP-171 token per heir (Checkpoint 9).
//!
//! The token id is the heir's label and its owner is the heir's account, `<label>.<estate>`. Only
//! heirs whose account exists hold one. Nothing about a token is stored: its metadata is computed
//! from the estate's live state, so the token *is* the claim right and changes as the estate does:
//! `locked` → `grace` → `claimable` → `claimed`.
//!
//! The views are written directly rather than through `near_contract_standards::NonFungibleToken`,
//! whose approval and transfer machinery a soulbound token doesn't need. The JSON shapes are the
//! standard's own types, so wallets and explorers render the credentials.

use near_contract_standards::non_fungible_token::Token;
use near_contract_standards::non_fungible_token::events::{NftBurn, NftMint};
use near_contract_standards::non_fungible_token::metadata::{
    NFT_METADATA_SPEC, NFTContractMetadata, TokenMetadata,
};
use near_sdk::json_types::{Base64VecU8, U128};
use near_sdk::{AccountId, PromiseOrValue, env, near};

use crate::{Estate, EstateExt, Heir, Status};

/// The page size when a caller gives no limit.
const DEFAULT_LIMIT: u64 = 50;
/// Where wallets fetch `media` from (NEP-177: `base_uri` + "/" + `media`). Pinata's gateway: the
/// images are pinned there and it serves them as `image/svg+xml`. A raw CID carries no file type,
/// and `ipfs.io` rate-limits and its in-browser redirect labels SVG as text, which wallets won't
/// draw. Any gateway works for the same CIDs.
const IPFS_GATEWAY: &str = "https://gateway.pinata.cloud/ipfs";

/// One image per state: (state, IPFS CIDv1 with raw encoding, SHA-256 of the file). The files are
/// `near/assets/credential/<state>.svg`; a raw CIDv1 is just the file's SHA-256, so the address
/// can't point at anything else. `nft_tests::the_images_match_their_files` checks every entry.
pub(crate) const CREDENTIAL_IMAGES: [(&str, &str, [u8; 32]); 4] = [
    (
        "locked",
        "bafkreibqatv5ibnvmkutd3mt3nf3fhwp5ricnatupszbum3yrwkg6pi2k4",
        [
            48, 4, 235, 212, 5, 181, 98, 169, 49, 237, 147, 219, 75, 178, 158, 207, 236, 80, 38,
            130, 116, 124, 178, 26, 51, 120, 141, 148, 111, 61, 26, 87,
        ],
    ),
    (
        "grace",
        "bafkreigv6cslco4b2wziwh3ul3blrkiloew2exxtrh6uudjmhfwjtckjhy",
        [
            213, 240, 164, 177, 59, 129, 213, 178, 139, 31, 116, 94, 194, 184, 169, 11, 113, 45,
            162, 94, 243, 137, 253, 74, 13, 44, 57, 108, 153, 137, 73, 62,
        ],
    ),
    (
        "claimable",
        "bafkreianqa2t6ixipd3ipyyqatzvgiqy2swd2as5kxdaym2dqoqgqqxabm",
        [
            13, 128, 53, 63, 34, 232, 120, 246, 135, 227, 16, 4, 243, 83, 34, 24, 212, 172, 61, 2,
            93, 85, 198, 12, 51, 67, 131, 160, 104, 66, 224, 11,
        ],
    ),
    (
        "claimed",
        "bafkreicd2b6s67zdyvediuh57nkbvgqyrq4qvwqg7iuhaizdgd3wanzo2e",
        [
            67, 208, 125, 47, 127, 35, 197, 72, 52, 80, 253, 251, 84, 26, 154, 24, 140, 57, 10,
            218, 6, 250, 40, 112, 35, 35, 48, 247, 96, 55, 46, 209,
        ],
    ),
];

const SOULBOUND: &str = "heir credentials are soulbound: they can't be transferred or approved";

/// The standard `nft_mint` event (NEP-297, `nep171`), so explorers and indexers pick it up.
pub(crate) fn emit_mint(owner: &AccountId, label: &str) {
    NftMint {
        owner_id: owner,
        token_ids: &[label],
        memo: Some("herit heir credential"),
    }
    .emit();
}

/// The standard `nft_burn` event, for an heir the grantor removed.
pub(crate) fn emit_burn(owner: &AccountId, label: &str) {
    NftBurn {
        owner_id: owner,
        token_ids: &[label],
        authorized_id: None,
        memo: Some("heir removed"),
    }
    .emit();
}

#[near]
impl Estate {
    pub fn nft_metadata(&self) -> NFTContractMetadata {
        NFTContractMetadata {
            spec: NFT_METADATA_SPEC.into(),
            name: format!("Herit heirs of {}", env::current_account_id()),
            symbol: "HEIR".into(),
            icon: None,
            base_uri: Some(IPFS_GATEWAY.into()),
            reference: None,
            reference_hash: None,
        }
    }

    /// The credential for `token_id` (an heir's label), computed from live state. `None` for an
    /// unknown label or an heir whose account is still being created.
    pub fn nft_token(&self, token_id: String) -> Option<Token> {
        let heir = self.heirs.get(&token_id).filter(|h| h.created)?;
        Some(self.credential(&token_id, heir))
    }

    /// Every credential, in the order the heirs were added.
    pub fn nft_tokens(&self, from_index: Option<U128>, limit: Option<u64>) -> Vec<Token> {
        self.credentials(|_| true, from_index, limit)
    }

    pub fn nft_total_supply(&self) -> U128 {
        U128(self.heirs.values().filter(|h| h.created).count() as u128)
    }

    /// An heir's account holds exactly one credential: its own label's.
    pub fn nft_tokens_for_owner(
        &self,
        account_id: AccountId,
        from_index: Option<U128>,
        limit: Option<u64>,
    ) -> Vec<Token> {
        self.credentials(|h| h.account_id == account_id, from_index, limit)
    }

    pub fn nft_supply_for_owner(&self, account_id: AccountId) -> U128 {
        U128(
            self.heirs
                .values()
                .filter(|h| h.created && h.account_id == account_id)
                .count() as u128,
        )
    }
}

/// The transfer and approval half of NEP-171 and NEP-178. Each refuses: the credential is the
/// heir's right to their share, and it stays with the heir's account. The methods exist so
/// wallets and marketplaces get a clear answer rather than "method not found".
#[near]
impl Estate {
    #[payable]
    pub fn nft_transfer(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        approval_id: Option<u64>,
        memo: Option<String>,
    ) {
        let _ = (receiver_id, token_id, approval_id, memo);
        env::panic_str(SOULBOUND)
    }

    #[payable]
    pub fn nft_transfer_call(
        &mut self,
        receiver_id: AccountId,
        token_id: String,
        approval_id: Option<u64>,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<bool> {
        let _ = (receiver_id, token_id, approval_id, memo, msg);
        env::panic_str(SOULBOUND)
    }

    #[payable]
    pub fn nft_approve(&mut self, token_id: String, account_id: AccountId, msg: Option<String>) {
        let _ = (token_id, account_id, msg);
        env::panic_str(SOULBOUND)
    }

    #[payable]
    pub fn nft_revoke(&mut self, token_id: String, account_id: AccountId) {
        let _ = (token_id, account_id);
        env::panic_str(SOULBOUND)
    }

    #[payable]
    pub fn nft_revoke_all(&mut self, token_id: String) {
        let _ = token_id;
        env::panic_str(SOULBOUND)
    }

    /// Nobody is ever approved.
    pub fn nft_is_approved(
        &self,
        token_id: String,
        approved_account_id: AccountId,
        approval_id: Option<u64>,
    ) -> bool {
        let _ = (token_id, approved_account_id, approval_id);
        false
    }
}

impl Estate {
    /// Where the credential stands, from the estate's live status and this heir's payouts.
    pub(crate) fn credential_state(&self, label: &str) -> &'static str {
        match self.pending_status() {
            Status::Active => "locked",
            Status::Grace => "grace",
            Status::Unlocked if self.fully_paid(label) => "claimed",
            Status::Unlocked => "claimable",
        }
    }

    /// Every asset this heir has a share of has been paid.
    fn fully_paid(&self, label: &str) -> bool {
        self.tokens.iter().all(|asset| {
            self.share(label, asset) == 0 || self.paid.contains(&(label.to_string(), asset.clone()))
        })
    }

    fn credential(&self, label: &str, heir: &Heir) -> Token {
        let state = self.credential_state(label);
        let (_, cid, hash) = CREDENTIAL_IMAGES
            .iter()
            .find(|(s, _, _)| *s == state)
            .expect("an image for every state");
        let estate = env::current_account_id();
        Token {
            token_id: label.to_string(),
            owner_id: heir.account_id.clone(),
            metadata: Some(TokenMetadata {
                title: Some(heir.account_id.to_string()),
                description: Some(format!(
                    "Heir credential for {estate} · {} · {state}",
                    heir.relationship
                )),
                // The relationship is limited to letters, digits, spaces and hyphens, so it can go
                // into JSON as is.
                extra: Some(format!(
                    r#"{{"estate":"{estate}","relationship":"{}","share_bps":{},"state":"{state}"}}"#,
                    heir.relationship, heir.default_bps
                )),
                copies: Some(1),
                media: Some(cid.to_string()),
                media_hash: Some(Base64VecU8(hash.to_vec())),
                ..Default::default()
            }),
            // Soulbound: nobody can ever be approved to move it.
            approved_account_ids: None,
        }
    }

    fn credentials(
        &self,
        include: impl Fn(&Heir) -> bool,
        from_index: Option<U128>,
        limit: Option<u64>,
    ) -> Vec<Token> {
        self.heirs
            .iter()
            .filter(|(_, h)| h.created && include(h))
            .skip(from_index.map_or(0, |i| i.0 as usize))
            .take(limit.unwrap_or(DEFAULT_LIMIT) as usize)
            .map(|(l, h)| self.credential(l, h))
            .collect()
    }
}
