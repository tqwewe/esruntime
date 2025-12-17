use serde_core::Deserialize;
use tracing::warn;
use umadb_dcb::{
    DCBAppendCondition, DCBEvent, DCBEventStoreAsync, DCBEventStoreSync, DCBQuery, DCBQueryItem,
    DCBSequencedEvent,
};
use uuid::Uuid;

use crate::{
    domain_id::{DomainIdBindings, DomainIdValue},
    emit::Emit,
    error::{CommandError, ExecuteError},
    event::EventSet,
};

/// Trait for command input structs that declare domain ID bindings.
///
/// Fields annotated with `#[domain_id("field_name")]` specify which
/// domain ID fields in events should match which input values.
///
/// # Example
///
/// ```rust,ignore
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
pub trait CommandInput {
    /// Returns the domain ID bindings for this input.
    ///
    /// Maps domain ID field names to the values to query for.
    fn domain_id_bindings(&self) -> DomainIdBindings;
}

/// The main trait for implementing command handlers.
///
/// A command handler:
/// 1. Declares its query via `Query` (which events) and `Input` (which domain IDs)
/// 2. Rebuilds state by processing historical events via `apply`
/// 3. Makes decisions and emits new events via `handle`
///
/// The handler must implement `Default` as a fresh instance is created
/// for each command execution.
///
/// # Example
///
/// ```
/// #[derive(EventSet)]
/// enum Query {
///     OpenedAccount(OpenedAccount),
///     SentFunds(SentFunds),
/// }
///
/// #[derive(CommandInput, Deserialize)]
/// struct Input {
///     #[domain_id]
///     account_id: String,
///     amount: f64,
/// }
///
/// #[derive(Default)]
/// struct Withdraw {
///     balance: f64,
/// }
///
/// impl Command for Withdraw {
///     type Query = Query;
///     type Input = Input;
///
///     fn apply(&mut self, event: Query) {
///         match event {
///             Query::OpenedAccount(ev) => self.balance = ev.initial_balance,
///             Query::SentFunds(ev) => self.balance -= ev.amount,
///         }
///     }
///
///     fn handle(self, input: Input) -> Result<Emit, CommandError> {
///         if self.balance < input.amount {
///             return Err(CommandError::rejected("Insufficient funds"));
///         }
///         
///         Ok(Emit::new().event(SentFunds {
///             account_id: input.account_id,
///             amount: input.amount,
///             recipient_id: None,
///         }))
///     }
/// }
/// ```
pub trait Command: Default + Send {
    /// The set of event types this handler reads.
    /// Defines the event type filter for the query.
    type Query: EventSet;

    /// The input type for this command.
    /// Defines the domain ID bindings for the query.
    type Input: CommandInput + for<'de> Deserialize<'de> + Send;

    /// Domain IDs query.
    ///
    /// Defaults to filtering domain ids in the input.
    fn query(&self, input: &Self::Input) -> DomainIdBindings {
        input.domain_id_bindings()
    }

    /// Apply a historical event to rebuild state.
    ///
    /// Called once for each event matching the query, in order.
    /// The handler should update its internal state based on the event.
    fn apply(&mut self, event: Self::Query);

    /// Handle the command and produce new events.
    ///
    /// Called after all historical events have been applied.
    /// Should validate the command against current state and either:
    /// - Return new events to persist
    /// - Return an error rejecting the command
    ///
    /// Takes `self` by value since the handler is consumed after execution.
    fn handle(self, input: Self::Input) -> Result<Emit, CommandError>;

    /// Execute the command, persisting the resulting events.
    fn execute(
        store: &impl DCBEventStoreAsync,
        input: Self::Input,
    ) -> impl Future<Output = Result<ExecuteResult, ExecuteError>> + Send {
        async move {
            let mut handler = Self::default();
            let bindings = handler.query(&input);
            let query_items = build_query_items(&bindings, Self::Query::EVENT_TYPES);

            let query = DCBQuery::with_items(query_items);
            let (events, head) = store
                .read(Some(query.clone()), Some(0), false, None, false)
                .await?
                .collect_with_head()
                .await?;

            for DCBSequencedEvent { position: _, event } in events {
                let Some(event) =
                    Self::Query::from_event(&event.event_type, &event.data).transpose()?
                else {
                    warn!("received event unused by query");
                    continue;
                };
                handler.apply(event);
            }

            let append_events: Vec<_> = handler
                .handle(input)?
                .into_events()
                .into_iter()
                .map(|event| DCBEvent {
                    event_type: event.event_type,
                    tags: event
                        .domain_ids
                        .into_iter()
                        .filter_map(|(category, id)| {
                            assert!(
                                !category.contains(':'),
                                "domain id categories cannot contain a colon character"
                            );
                            match id {
                                DomainIdValue::Value(id) => Some(format!("{category}:{id}")),
                                DomainIdValue::None => None,
                            }
                        })
                        .collect(),
                    data: event.data,
                    uuid: Some(Uuid::new_v4()),
                })
                .collect();

            if append_events.is_empty() {
                return Ok(ExecuteResult {
                    position: head,
                    events: Vec::new(),
                });
            }

            let new_position = store
                .append(
                    append_events.clone(),
                    Some(DCBAppendCondition {
                        fail_if_events_match: query,
                        after: head,
                    }),
                )
                .await?;

            Ok(ExecuteResult {
                position: Some(new_position),
                events: append_events,
            })
        }
    }

    /// Execute the command in a blocking context, persisting the resulting events.
    fn execute_blocking(
        store: &impl DCBEventStoreSync,
        input: Self::Input,
    ) -> Result<ExecuteResult, ExecuteError> {
        let mut handler = Self::default();
        let bindings = handler.query(&input);
        let query_items = build_query_items(&bindings, Self::Query::EVENT_TYPES);

        let query = DCBQuery::with_items(query_items);
        let (events, head) = store
            .read(Some(query.clone()), Some(0), false, None, false)?
            .collect_with_head()?;

        for DCBSequencedEvent { position: _, event } in events {
            let Some(event) =
                Self::Query::from_event(&event.event_type, &event.data).transpose()?
            else {
                warn!("received event unused by query");
                continue;
            };
            handler.apply(event);
        }

        let append_events: Vec<_> = handler
            .handle(input)?
            .into_events()
            .into_iter()
            .map(|event| DCBEvent {
                event_type: event.event_type,
                tags: event
                    .domain_ids
                    .into_iter()
                    .filter_map(|(category, id)| {
                        assert!(
                            !category.contains(':'),
                            "domain id categories cannot contain a colon character"
                        );
                        match id {
                            DomainIdValue::Value(id) => Some(format!("{category}:{id}")),
                            DomainIdValue::None => None,
                        }
                    })
                    .collect(),
                data: event.data,
                uuid: Some(Uuid::new_v4()),
            })
            .collect();

        if append_events.is_empty() {
            return Ok(ExecuteResult {
                position: head,
                events: Vec::new(),
            });
        }

        let new_position = store.append(
            append_events.clone(),
            Some(DCBAppendCondition {
                fail_if_events_match: query,
                after: head,
            }),
        )?;

        Ok(ExecuteResult {
            position: Some(new_position),
            events: append_events,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ExecuteResult {
    pub position: Option<u64>,
    pub events: Vec<DCBEvent>,
}

/// Builds DCB query items from domain ID bindings.
///
/// Takes the cartesian product across different domain ID field names,
/// giving OR semantics within each field and AND semantics across fields.
pub fn build_query_items(bindings: &DomainIdBindings, event_types: &[&str]) -> Vec<DCBQueryItem> {
    // Convert bindings to a vec of (field_name, values) for easier iteration
    let binding_groups: Vec<_> = bindings.iter().map(|(k, v)| (*k, v)).collect();

    if binding_groups.is_empty() {
        // No domain IDs - query all events of these types
        return vec![DCBQueryItem::new().types(event_types.iter().copied())];
    }

    // Build cartesian product
    let mut combinations: Vec<Vec<String>> = vec![vec![]];

    for (field_name, values) in &binding_groups {
        let mut new_combinations = Vec::new();

        for existing in &combinations {
            for value in *values {
                let mut new_combo = existing.clone();
                new_combo.push(format!("{field_name}:{value}"));
                new_combinations.push(new_combo);
            }
        }

        combinations = new_combinations;
    }

    // Convert each combination to a query item
    combinations
        .into_iter()
        .map(|tags| {
            DCBQueryItem::new()
                .tags(tags)
                .types(event_types.iter().copied())
        })
        .collect()
}
