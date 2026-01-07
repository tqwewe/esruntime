use std::{sync::Arc, time::Duration};

use esruntime_sdk::{
    error::SerializationError,
    event::{EventSet, StoredEvent, StoredEventData},
};
use futures::TryStreamExt;
use serde_json::Value;
use sqlx::{PgPool, PgTransaction};
use thiserror::Error;
use tokio::time::Instant;
use tracing::warn;
use umadb_client::AsyncUmaDBClient;
use umadb_dcb::{DCBError, DCBEventStoreAsync, DCBQuery, DCBQueryItem, DCBReadResponseAsync};

const DEFAULT_CHECKPOINT_TABLE_NAME: &str = "checkpoints";
const DEFAULT_CHECKPOINT_POSITION_COLUMN: &str = "position";
const DEFAULT_CHECKPOINT_PROJECTION_ID_COL: &str = "projection_id";

pub struct ProjectionRunner<H, C>
where
    H: EventHandler,
    C: Checkpoint,
{
    pool: PgPool,
    handler: H,
    checkpoint: C,
    flush_config: FlushConfig,
    head: Option<u64>,
    stream: Box<dyn DCBReadResponseAsync + Send + 'static>,
    transaction: Option<PgTransaction<'static>>,
    position: Option<u64>,
    last_flushed_position: Option<u64>,
    events_since_flush: u32,
    last_flushed_at: Instant,
}

impl<H, C> ProjectionRunner<H, C>
where
    H: EventHandler,
    C: Checkpoint,
    ProjectionError<H::Error>: From<C::Error>,
{
    async fn new(
        pool: PgPool,
        event_store: &AsyncUmaDBClient,
        handler: H,
        checkpoint: C,
        query: Option<DCBQuery>,
        flush_config: FlushConfig,
    ) -> Result<Self, ProjectionError<H::Error>> {
        let position = checkpoint.load().await?;

        let head = event_store.head().await?;
        let stream = event_store
            .read(query, position.map(|pos| pos + 1), false, None, true)
            .await?;

        Ok(ProjectionRunner {
            pool,
            handler,
            checkpoint,
            flush_config,
            head,
            stream,
            transaction: None,
            position,
            last_flushed_position: position,
            events_since_flush: 0,
            last_flushed_at: Instant::now(),
        })
    }

    pub fn builder(checkpoint: C) -> ProjectionRunnerBuilder<C> {
        ProjectionRunnerBuilder::new(checkpoint)
    }

    pub async fn run(&mut self) -> Result<(), ProjectionError<H::Error>> {
        while self.next().await? {}

        Ok(())
    }

    pub async fn next(&mut self) -> Result<bool, ProjectionError<H::Error>> {
        let event = if self.events_since_flush > 0 {
            let is_replaying = self.position <= self.head;
            let interval = if is_replaying {
                self.flush_config.replay_time_interval
            } else {
                self.flush_config.live_time_interval
            };
            match tokio::time::timeout_at(self.last_flushed_at + interval, self.stream.try_next())
                .await
            {
                Ok(res) => res?,
                Err(_) => {
                    self.flush_if_necessary().await?;
                    return Ok(true);
                }
            }
        } else {
            self.stream.try_next().await?
        };
        let Some(event) = event else {
            return Ok(false);
        };

        if self.transaction.is_none() {
            let tx = self.pool.begin().await?;
            self.transaction = Some(tx);
        }

        let tx = self.transaction.as_mut().unwrap();

        let event_data: StoredEventData<Value> = serde_json::from_slice(&event.event.data)?;
        let query = H::Query::from_event(&event.event.event_type, event_data.data).transpose()?;

        if let Some(data) = query {
            let stored_event = StoredEvent {
                id: event.event.uuid.unwrap(),
                position: event.position,
                event_type: event.event.event_type,
                tags: event.event.tags,
                timestamp: event_data.timestamp,
                correlation_id: event_data.correlation_id,
                causation_id: event_data.causation_id,
                triggered_by: event_data.triggered_by,
                data,
            };
            self.handler
                .handle(tx, stored_event)
                .await
                .map_err(ProjectionError::Handler)?;
        } else {
            warn!("received event which was not deserialized into the query");
        }
        self.position = Some(event.position);
        self.events_since_flush += 1;

        self.flush_if_necessary().await?;

        Ok(true)
    }

    async fn flush(&mut self, is_replaying: bool) -> Result<(), ProjectionError<H::Error>> {
        let Some(position) = self.position.take() else {
            return Ok(());
        };

        let Some(mut tx) = self.transaction.take() else {
            return Ok(());
        };

        self.checkpoint
            .save(&mut tx, self.last_flushed_position, position)
            .await?;

        self.handler
            .flush(&mut tx, is_replaying)
            .await
            .map_err(ProjectionError::Handler)?;

        tx.commit().await?;

        self.handler
            .post_commit(is_replaying)
            .await
            .map_err(ProjectionError::Handler)?;

        self.events_since_flush = 0;
        self.last_flushed_at = Instant::now();
        self.last_flushed_position = Some(position);

        Ok(())
    }

    async fn flush_if_necessary(&mut self) -> Result<(), ProjectionError<H::Error>> {
        let is_replaying = self.position <= self.head;

        if self.flush_config.should_flush(
            self.events_since_flush,
            self.last_flushed_at,
            is_replaying,
        ) {
            self.flush(is_replaying).await?;
        }

        Ok(())
    }
}

pub struct ProjectionRunnerBuilder<C> {
    checkpoint: C,
    query: Option<Option<DCBQuery>>,
    flush_config: FlushConfig,
}

impl<C> ProjectionRunnerBuilder<C> {
    pub fn new(checkpoint: C) -> Self {
        ProjectionRunnerBuilder {
            checkpoint,
            query: None,
            flush_config: FlushConfig::default(),
        }
    }

    pub async fn build<H>(
        self,
        pool: PgPool,
        event_store: &AsyncUmaDBClient,
        handler: H,
    ) -> Result<ProjectionRunner<H, C>, ProjectionError<H::Error>>
    where
        H: EventHandler,
        C: Checkpoint,
        ProjectionError<H::Error>: From<C::Error>,
    {
        ProjectionRunner::new(
            pool,
            event_store,
            handler,
            self.checkpoint,
            self.query.unwrap_or_else(|| {
                Some(DCBQuery::with_items([
                    DCBQueryItem::new().types(H::Query::EVENT_TYPES.iter().copied())
                ]))
            }),
            self.flush_config,
        )
        .await
    }

    pub fn checkpoint<T>(self, checkpoint: T) -> ProjectionRunnerBuilder<T> {
        ProjectionRunnerBuilder {
            checkpoint,
            query: self.query,
            flush_config: self.flush_config,
        }
    }

    pub fn query(mut self, query: Option<DCBQuery>) -> Self {
        self.query = Some(query);
        self
    }

    pub fn flush_live_events_interval(mut self, flush_events_interval: u32) -> Self {
        self.flush_config.live_events_interval = flush_events_interval;
        self
    }

    pub fn flush_live_time_interval(mut self, flush_time_interval: Duration) -> Self {
        self.flush_config.live_time_interval = flush_time_interval;
        self
    }

    pub fn flush_replay_events_interval(mut self, flush_events_interval: u32) -> Self {
        self.flush_config.replay_events_interval = flush_events_interval;
        self
    }

    pub fn flush_replay_time_interval(mut self, flush_time_interval: Duration) -> Self {
        self.flush_config.replay_time_interval = flush_time_interval;
        self
    }
}

pub trait EventHandler {
    type Query: EventSet;
    type Error;

    fn handle(
        &mut self,
        tx: &mut PgTransaction<'static>,
        event: StoredEvent<Self::Query>,
    ) -> impl Future<Output = Result<(), Self::Error>>;

    #[allow(unused_variables)]
    fn flush(
        &mut self,
        tx: &mut PgTransaction<'static>,
        is_replaying: bool,
    ) -> impl Future<Output = Result<(), Self::Error>> {
        async { Ok(()) }
    }

    #[allow(unused_variables)]
    fn post_commit(&mut self, is_replaying: bool) -> impl Future<Output = Result<(), Self::Error>> {
        async { Ok(()) }
    }
}

pub trait Checkpoint {
    type Error;

    fn load(&self) -> impl Future<Output = Result<Option<u64>, Self::Error>>;
    fn save(
        &self,
        tx: &mut PgTransaction<'static>,
        expected_position: Option<u64>,
        position: u64,
    ) -> impl Future<Output = Result<(), Self::Error>>;
}

pub struct CheckpointTable {
    pool: PgPool,
    projection_id: Arc<str>,
    table: Arc<str>,
    position_col: Arc<str>,
    projection_id_col: Arc<str>,
}

impl CheckpointTable {
    pub fn new(pool: PgPool, projection_id: impl Into<Arc<str>>) -> Self {
        CheckpointTable {
            pool,
            projection_id: projection_id.into(),
            table: DEFAULT_CHECKPOINT_TABLE_NAME.into(),
            position_col: DEFAULT_CHECKPOINT_POSITION_COLUMN.into(),
            projection_id_col: DEFAULT_CHECKPOINT_PROJECTION_ID_COL.into(),
        }
    }

    pub fn table(mut self, table: impl Into<Arc<str>>) -> Self {
        self.table = table.into();
        self
    }

    pub fn position_col(mut self, position_col: impl Into<Arc<str>>) -> Self {
        self.position_col = position_col.into();
        self
    }

    pub fn projection_id_col(mut self, projection_id_col: impl Into<Arc<str>>) -> Self {
        self.projection_id_col = projection_id_col.into();
        self
    }
}

impl Checkpoint for CheckpointTable {
    type Error = sqlx::Error;

    async fn load(&self) -> Result<Option<u64>, Self::Error> {
        let position = sqlx::query_scalar::<_, i64>(&format!(
            "SELECT {} FROM {} WHERE {} = $1",
            self.position_col, self.table, self.projection_id_col,
        ))
        .bind(self.projection_id.as_ref())
        .fetch_optional(&self.pool)
        .await?;

        Ok(position.map(|pos| pos as u64))
    }

    async fn save(
        &self,
        tx: &mut PgTransaction<'static>,
        expected_position: Option<u64>,
        position: u64,
    ) -> Result<(), Self::Error> {
        match expected_position {
            None => {
                // Expect no row exists - insert will fail if it does
                sqlx::query(&format!(
                    "INSERT INTO {} ({}, {}) VALUES ($1, $2)",
                    self.table, self.projection_id_col, self.position_col,
                ))
                .bind(self.projection_id.as_ref())
                .bind(position as i64)
                .execute(&mut **tx)
                .await?;
            }
            Some(expected) => {
                // Only update if current position matches expected (compare-and-swap)
                let result = sqlx::query(&format!(
                    "UPDATE {} SET {} = $1 WHERE {} = $2 AND {} = $3",
                    self.table, self.position_col, self.projection_id_col, self.position_col,
                ))
                .bind(position as i64)
                .bind(self.projection_id.as_ref())
                .bind(expected as i64)
                .execute(&mut **tx)
                .await?;

                if result.rows_affected() == 0 {
                    return Err(sqlx::Error::RowNotFound);
                }
            }
        }

        Ok(())
    }
}

struct FlushConfig {
    live_events_interval: u32,
    live_time_interval: Duration,
    replay_events_interval: u32,
    replay_time_interval: Duration,
}

impl FlushConfig {
    fn should_flush(
        &self,
        events_since_flush: u32,
        last_flushed_at: Instant,
        is_replaying: bool,
    ) -> bool {
        if is_replaying {
            events_since_flush >= self.replay_events_interval
                || last_flushed_at.elapsed() >= self.replay_time_interval
        } else {
            events_since_flush >= self.live_events_interval
                || last_flushed_at.elapsed() >= self.live_time_interval
        }
    }
}

impl Default for FlushConfig {
    fn default() -> Self {
        Self {
            live_events_interval: 1,
            live_time_interval: Duration::from_secs(1),
            replay_events_interval: 500,
            replay_time_interval: Duration::from_secs(10),
        }
    }
}

#[derive(Debug, Error)]
pub enum ProjectionError<E> {
    #[error(transparent)]
    Handler(E),
    #[error(transparent)]
    DCB(#[from] DCBError),
    #[error(transparent)]
    Deserialize(#[from] serde_json::Error),
    #[error(transparent)]
    Serialize(#[from] SerializationError),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}
