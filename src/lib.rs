//! # ESRuntime SDK
//!
//! SDK for building event-sourced command handlers as WASM modules.
//!
//! ## Overview
//!
//! This crate provides the traits and types needed to write command handlers
//! that run in the ESRuntime. Command handlers:
//!
//! 1. Declare which events they need to read (via `EventSet`)
//! 2. Declare which domain IDs to query (via `CommandInput`)
//! 3. Rebuild state from historical events (via `apply`)
//! 4. Make decisions and emit new events (via `execute`)
//!
//! ## Example
//!
//! ```rust
//! use esruntime_sdk::prelude::*;
//! use my_schema::{OpenedAccount, SentFunds};
//!
//! #[derive(EventSet)]
//! enum Query {
//!     OpenedAccount(OpenedAccount),
//!     SentFunds(SentFunds),
//! }
//!
//! #[derive(CommandInput, Deserialize)]
//! struct Input {
//!     #[domain_id("account_id")]
//!     account_id: String,
//!     amount: f64,
//! }
//!
//! #[derive(Default)]
//! struct Withdraw {
//!     balance: f64,
//! }
//!
//! impl CommandHandler for Withdraw {
//!     type Query = Query;
//!     type Input = Input;
//!
//!     fn apply(&mut self, event: Query) {
//!         match event {
//!             Query::OpenedAccount(e) => self.balance = e.initial_balance,
//!             Query::SentFunds(e) => self.balance -= e.amount,
//!         }
//!     }
//!
//!     fn execute(self, input: Input) -> Result<Emit, CommandError> {
//!         if self.balance < input.amount {
//!             return Err(CommandError::rejected("Insufficient funds"));
//!         }
//!         
//!         Ok(Emit::new().event(SentFunds {
//!             account_id: input.account_id,
//!             amount: input.amount,
//!             recipient_id: None,
//!         }))
//!     }
//! }
//!
//! export_handler!(Withdraw);
//! ```

pub mod command;
pub mod domain_id;
pub mod emit;
pub mod error;
pub mod event;
#[macro_use]
mod macros;

pub mod prelude {
    pub use crate::command::*;
    pub use crate::domain_id::*;
    pub use crate::emit::*;
    pub use crate::error::*;
    pub use crate::event::*;
    #[allow(unused)]
    pub use crate::macros::*;

    // Re-export derive macros (these would come from esruntime-sdk-macros)
    // pub use esruntime_sdk_macros::{CommandInput, Event, EventSet};
}
