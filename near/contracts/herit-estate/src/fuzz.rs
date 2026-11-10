use near_sdk::test_utils::VMContextBuilder;
use near_sdk::{AccountId, testing_env};
use proptest::prelude::*;

use super::Estate;

proptest! {
    #[test]
    fn version_does_not_depend_on_the_caller(name in "[a-z0-9]{2,32}") {
        let caller: AccountId = name.parse().unwrap();
        testing_env!(VMContextBuilder::new().predecessor_account_id(caller).build());
        prop_assert_eq!(Estate::default().version(), "0.1.0");
    }
}
