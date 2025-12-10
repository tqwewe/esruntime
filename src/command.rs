use serde_core::Deserialize;

use crate::{domain_id::DomainIdBindings, emit::Emit, error::CommandError, event::EventSet};

/// Trait for command input structs that declare domain ID bindings.
///
/// Fields annotated with `#[domain_id("field_name")]` specify which
/// domain ID fields in events should match which input values.
///
/// # Example
///
/// ```rust
/// #[derive(CommandInput, Deserialize)]
/// struct TransferInput {
///     #[domain_id("account_id")]
///     source_account: String,
///     #[domain_id("account_id")]
///     dest_account: String,
///     amount: f64,
/// }
/// ```
///
/// This generates a query for events where `account_id` is either
/// `source_account` or `dest_account`.
pub trait CommandInput: for<'de> Deserialize<'de> {
    /// Returns the domain ID bindings for this input.
    ///
    /// Maps domain ID field names to the values to query for.
    fn domain_id_bindings(&self) -> DomainIdBindings;
}

// =============================================================================
// CommandHandler Trait
// =============================================================================

/// The main trait for implementing command handlers.
///
/// A command handler:
/// 1. Declares its query via `Query` (which events) and `Input` (which domain IDs)
/// 2. Rebuilds state by processing historical events via `apply`
/// 3. Makes decisions and emits new events via `execute`
///
/// The handler must implement `Default` as a fresh instance is created
/// for each command execution.
pub trait CommandHandler: Default {
    /// The set of event types this handler reads.
    /// Defines the event type filter for the query.
    type Query: EventSet;

    /// The input type for this command.
    /// Defines the domain ID bindings for the query.
    type Input: CommandInput;

    /// Domain IDs query.
    /// Defaults to filtering domain ids in the input.
    fn query(&self, input: &Self::Input) -> DomainIdBindings {
        input.domain_id_bindings()
    }

    /// Apply a historical event to rebuild state.
    ///
    /// Called once for each event matching the query, in order.
    /// The handler should update its internal state based on the event.
    fn apply(&mut self, event: Self::Query);

    /// Execute the command and produce new events.
    ///
    /// Called after all historical events have been applied.
    /// Should validate the command against current state and either:
    /// - Return new events to persist
    /// - Return an error rejecting the command
    ///
    /// Takes `self` by value since the handler is consumed after execution.
    fn execute(self, input: Self::Input) -> Result<Emit, CommandError>;
}
