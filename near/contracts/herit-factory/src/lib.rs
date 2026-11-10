//! Herit factory contract: placeholder, so the build and every test layer have something to run.

use near_sdk::near;

#[near(contract_state)]
#[derive(Default)]
pub struct Factory {}

#[near]
impl Factory {
    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }
}

#[cfg(test)]
mod fuzz;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_matches_the_package() {
        assert_eq!(Factory::default().version(), "0.1.0");
    }
}
