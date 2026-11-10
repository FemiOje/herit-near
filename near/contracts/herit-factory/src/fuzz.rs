//! Fuzz tests: any label, any deposit.

use near_sdk::{AccountId, NearToken};
use proptest::prelude::*;

use super::tests::*;

proptest! {
    /// Any string: `open_estate` goes ahead exactly when it is one lowercase segment making a valid
    /// account under the factory, and enough NEAR is attached.
    #[test]
    fn open_estate_accepts_exactly_valid_labels_and_deposits(
        label in "[a-zA-Z0-9._ -]{0,70}",
        milli in 0..300u128,
    ) {
        let mut f = new_factory();
        set_context(grantor(), NearToken::from_millinear(milli));
        let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = f.open_estate(label.clone(), 600, 300);
        }))
        .is_ok();
        let valid_label = !label.is_empty()
            && label.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
            && format!("{label}.herit-near.testnet").parse::<AccountId>().is_ok();
        prop_assert_eq!(ok, valid_label && milli >= 110, "{:?} with {} mNEAR", label, milli);
    }
}
