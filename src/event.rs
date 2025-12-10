use crate::{domain_id::DomainIdValues, error::SerializationError};

/// Trait for individual event structs.
///
/// This is implemented by generated event structs from ESDL schemas.
/// Each event knows its type name and which fields are domain identifiers.
///
/// # Example (generated code)
///
/// ```rust
/// #[derive(Event, Clone, Serialize, Deserialize)]
/// #[event_type("SentFunds")]
/// pub struct SentFunds {
///     #[domain_id]
///     pub account_id: String,
///     pub amount: f64,
///     #[domain_id]
///     pub recipient_id: Option<String>,
/// }
/// ```
pub trait Event: Sized {
    /// The event type name as it appears in the event store.
    /// This should match the ESDL event name.
    fn event_type(&self) -> &'static str;

    /// Serialize this event to bytes (JSON).
    fn to_bytes(&self) -> Result<Vec<u8>, SerializationError>;

    /// Deserialize an event from bytes (JSON).
    fn from_bytes(data: &[u8]) -> Result<Self, SerializationError>;

    /// Returns the domain ID field names and their values for this event instance.
    /// Used by the runtime for indexing and querying.
    fn domain_ids(&self) -> DomainIdValues;
}

/// Trait for a set of events that a command handler reads.
///
/// This is derived on a user-defined enum that wraps the event types
/// the command cares about. The runtime uses this to:
///
/// 1. Know which event types to fetch from the store
/// 2. Deserialize events into the correct variant
///
/// # Example
///
/// ```rust
/// #[derive(EventSet)]
/// enum Query {
///     OpenedAccount(OpenedAccount),
///     SentFunds(SentFunds),
/// }
/// ```
pub trait EventSet: Sized {
    /// Returns the event type names this set can contain.
    /// Used to build the query to the event store.
    fn event_types() -> &'static [&'static str];

    /// Attempt to deserialize an event into this set.
    ///
    /// Returns `None` if the event type is not part of this set,
    /// or `Some(Err(...))` if deserialization fails.
    fn from_event(event_type: &str, data: &[u8]) -> Option<Result<Self, SerializationError>>;
}
