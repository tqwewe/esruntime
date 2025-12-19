use serde_json::Value;

use crate::{domain_id::DomainIdValues, error::SerializationError, event::Event};

/// A collection of events to be emitted by a command.
///
/// Built using the builder pattern:
///
/// ```rust,ignore
/// Ok(Emit::new()
///     .event(SentFunds { ... })
///     .event(ReceivedFunds { ... }))
/// ```
#[derive(Debug, Default)]
pub struct Emit {
    events: Vec<EmittedEvent>,
}

/// A serialized event ready for persistence.
#[derive(Debug)]
pub struct EmittedEvent {
    /// The event type name
    pub event_type: String,
    /// The serialized event data (JSON)
    pub data: Value,
    /// Domain ID values for indexing
    pub domain_ids: DomainIdValues,
}

impl Emit {
    /// Create a new empty emit collection.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Add an event to be emitted.
    ///
    /// # Panics
    ///
    /// Panics if the event cannot be serialized. In practice this
    /// shouldn't happen with well-formed event structs.
    pub fn event<E: Event>(mut self, event: E) -> Self {
        let domain_ids = event.domain_ids();
        let emitted = EmittedEvent {
            event_type: E::EVENT_TYPE.to_string(),
            data: serde_json::to_value(event).expect("event serialization failed"),
            domain_ids,
        };
        self.events.push(emitted);
        self
    }

    /// Add an event, returning an error if serialization fails.
    pub fn try_event<E: Event>(mut self, event: E) -> Result<Self, SerializationError> {
        let domain_ids = event.domain_ids();
        let emitted = EmittedEvent {
            event_type: E::EVENT_TYPE.to_string(),
            data: serde_json::to_value(event)?,
            domain_ids,
        };
        self.events.push(emitted);
        Ok(self)
    }

    /// Returns true if no events will be emitted.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns the number of events to be emitted.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Consume and return the collected events.
    pub fn into_events(self) -> Vec<EmittedEvent> {
        self.events
    }
}
