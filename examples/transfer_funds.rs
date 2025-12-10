//! TransferFunds command handler
//!
//! Atomically transfers funds between two accounts using DCB.
//! The consistency boundary spans both accounts.
//! Generated from ESDL schema:
//!
//! ```esdl
//! version = "0.1.0"
//!
//! event OpenedAccount {
//!   @account_id: String
//!   initial_balance: Float
//! }
//!
//! event SentFunds {
//!   @account_id: String
//!   amount: Float
//!   @recipient_id: String?
//! }
//!
//! event ReceivedFunds {
//!   @account_id: String
//!   amount: Float
//!   @sender_id: String?
//! }
//! ```

use esruntime_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// OpenedAccount
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenedAccount {
    pub account_id: String,
    pub initial_balance: f64,
}

impl Event for OpenedAccount {
    fn event_type(&self) -> &'static str {
        "OpenedAccount"
    }

    fn to_bytes(&self) -> Result<Vec<u8>, SerializationError> {
        serde_json::to_vec(self).map_err(Into::into)
    }

    fn from_bytes(data: &[u8]) -> Result<Self, SerializationError> {
        serde_json::from_slice(data).map_err(Into::into)
    }

    fn domain_ids(&self) -> DomainIdValues {
        let mut ids = HashMap::new();
        ids.insert("account_id", DomainIdValue::from(self.account_id.clone()));
        ids
    }
}

// =============================================================================
// SentFunds
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentFunds {
    pub account_id: String,
    pub amount: f64,
    pub recipient_id: Option<String>,
}

impl Event for SentFunds {
    fn event_type(&self) -> &'static str {
        "SentFunds"
    }

    fn to_bytes(&self) -> Result<Vec<u8>, SerializationError> {
        serde_json::to_vec(self).map_err(Into::into)
    }

    fn from_bytes(data: &[u8]) -> Result<Self, SerializationError> {
        serde_json::from_slice(data).map_err(Into::into)
    }

    fn domain_ids(&self) -> DomainIdValues {
        let mut ids = HashMap::new();
        ids.insert("account_id", DomainIdValue::from(self.account_id.clone()));
        ids.insert(
            "recipient_id",
            DomainIdValue::from(self.recipient_id.clone()),
        );
        ids
    }
}

// =============================================================================
// ReceivedFunds
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceivedFunds {
    pub account_id: String,
    pub amount: f64,
    pub sender_id: Option<String>,
}

impl Event for ReceivedFunds {
    fn event_type(&self) -> &'static str {
        "ReceivedFunds"
    }

    fn to_bytes(&self) -> Result<Vec<u8>, SerializationError> {
        serde_json::to_vec(self).map_err(Into::into)
    }

    fn from_bytes(data: &[u8]) -> Result<Self, SerializationError> {
        serde_json::from_slice(data).map_err(Into::into)
    }

    fn domain_ids(&self) -> DomainIdValues {
        let mut ids = HashMap::new();
        ids.insert("account_id", DomainIdValue::from(self.account_id.clone()));
        ids.insert("sender_id", DomainIdValue::from(self.sender_id.clone()));
        ids
    }
}

// =============================================================================
// Query - Events this command reads
// =============================================================================

/// The derive macro generates the EventSet implementation:
///
/// impl EventSet for Query {
///     fn event_types() -> &'static [&'static str] {
///         &["OpenedAccount", "SentFunds", "ReceivedFunds"]
///     }
///
///     fn from_event(event_type: &str, data: &[u8]) -> Option<Result<Self, SerializationError>> {
///         match event_type {
///             "OpenedAccount" => Some(OpenedAccount::from_bytes(data).map(Query::OpenedAccount)),
///             "SentFunds" => Some(SentFunds::from_bytes(data).map(Query::SentFunds)),
///             "ReceivedFunds" => Some(ReceivedFunds::from_bytes(data).map(Query::ReceivedFunds)),
///             _ => None,
///         }
///     }
/// }
// #[derive(EventSet)]
enum Query {
    OpenedAccount(OpenedAccount),
    SentFunds(SentFunds),
    ReceivedFunds(ReceivedFunds),
}

impl EventSet for Query {
    fn event_types() -> &'static [&'static str] {
        &["OpenedAccount", "SentFunds", "ReceivedFunds"]
    }

    fn from_event(event_type: &str, data: &[u8]) -> Option<Result<Self, SerializationError>> {
        match event_type {
            "OpenedAccount" => Some(OpenedAccount::from_bytes(data).map(Query::OpenedAccount)),
            "SentFunds" => Some(SentFunds::from_bytes(data).map(Query::SentFunds)),
            "ReceivedFunds" => Some(ReceivedFunds::from_bytes(data).map(Query::ReceivedFunds)),
            _ => None,
        }
    }
}

// =============================================================================
// Input - Command payload with domain ID bindings
// =============================================================================

/// The derive macro generates the CommandInput implementation:
///
/// impl CommandInput for Input {
///     fn domain_id_bindings(&self) -> DomainIdBindings {
///         let mut bindings = HashMap::new();
///         bindings
///             .entry("account_id")
///             .or_insert_with(Vec::new)
///             .push(self.source_account.clone());
///         bindings
///             .entry("account_id")
///             .or_insert_with(Vec::new)
///             .push(self.dest_account.clone());
///         bindings
///     }
/// }
// #[derive(CommandInput, Deserialize)]
#[derive(Deserialize)]
struct Input {
    // #[domain_id("account_id")]
    source_account: String,
    // #[domain_id("account_id")]
    dest_account: String,
    amount: f64,
}

impl CommandInput for Input {
    fn domain_id_bindings(&self) -> DomainIdBindings {
        let mut bindings = HashMap::new();
        bindings
            .entry("account_id")
            .or_insert_with(Vec::new)
            .push(self.source_account.clone());
        bindings
            .entry("account_id")
            .or_insert_with(Vec::new)
            .push(self.dest_account.clone());
        bindings
    }
}

// =============================================================================
// Handler State
// =============================================================================

#[derive(Default)]
struct TransferFunds {
    /// Balance per account_id
    balances: HashMap<String, f64>,
    /// Which accounts are open
    open_accounts: HashMap<String, bool>,
}

// =============================================================================
// CommandHandler Implementation
// =============================================================================

impl CommandHandler for TransferFunds {
    type Query = Query;
    type Input = Input;

    fn apply(&mut self, event: Query) {
        match event {
            Query::OpenedAccount(e) => {
                self.balances
                    .insert(e.account_id.clone(), e.initial_balance);
                self.open_accounts.insert(e.account_id, true);
            }
            Query::SentFunds(e) => {
                if let Some(balance) = self.balances.get_mut(&e.account_id) {
                    *balance -= e.amount;
                }
            }
            Query::ReceivedFunds(e) => {
                if let Some(balance) = self.balances.get_mut(&e.account_id) {
                    *balance += e.amount;
                }
            }
        }
    }

    fn execute(self, input: Input) -> Result<Emit, CommandError> {
        // Validate source account
        if !self
            .open_accounts
            .get(&input.source_account)
            .copied()
            .unwrap_or(false)
        {
            return Err(CommandError::rejected("Source account not open"));
        }

        // Validate destination account
        if !self
            .open_accounts
            .get(&input.dest_account)
            .copied()
            .unwrap_or(false)
        {
            return Err(CommandError::rejected("Destination account not open"));
        }

        // Validate amount
        if input.amount <= 0.0 {
            return Err(CommandError::invalid_input("Amount must be positive"));
        }

        // Check sufficient funds
        let source_balance = self
            .balances
            .get(&input.source_account)
            .copied()
            .unwrap_or(0.0);
        if source_balance < input.amount {
            return Err(CommandError::rejected(format!(
                "Insufficient funds: available {}, requested {}",
                source_balance, input.amount
            )));
        }

        // Emit both events atomically
        Ok(Emit::new()
            .event(SentFunds {
                account_id: input.source_account.clone(),
                amount: input.amount,
                recipient_id: Some(input.dest_account.clone()),
            })
            .event(ReceivedFunds {
                account_id: input.dest_account,
                amount: input.amount,
                sender_id: Some(input.source_account),
            }))
    }
}

// Generate WASM exports
// export_handler!(TransferFunds);
