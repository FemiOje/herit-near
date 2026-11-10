//! TEST ONLY. Publishes the raw call input as a Global Contract identified by its code hash, so the
//! sandbox tests can open estates the way testnet will. near-workspaces has no action for this.
//! Never deployed to testnet: there, the NEAR CLI publishes the estate code (Checkpoint 10, Part 3).

use near_sdk::{Promise, env, near};

#[near(contract_state)]
#[derive(Default)]
pub struct Publisher {}

#[near]
impl Publisher {
    /// Publishes the call's raw input bytes. Burns the publisher's NEAR at the Global Contract rate.
    pub fn publish(&mut self) -> Promise {
        let code = env::input().unwrap_or_else(|| env::panic_str("send the wasm as the raw input"));
        Promise::new(env::current_account_id()).deploy_global_contract(code)
    }
}
