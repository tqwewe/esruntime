pub mod error;

use std::{sync::Arc, time::Duration};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    routing::post,
};
use axum_idempotent::{IdempotentLayer, IdempotentOptions};
use base64::{Engine, prelude::BASE64_STANDARD};
use esruntime_sdk::prelude::*;
use ruts::{
    CookieOptions, SessionLayer, store::memory::MemoryStore, tower_cookies::CookieManagerLayer,
};
use serde_json::{Value, json};
use tokio::{io, net::ToSocketAddrs};
use tower_http::timeout::TimeoutLayer;
use umadb_client::AsyncUmaDBClient;

use crate::error::{Error, ErrorStatus};

pub struct CommandRouter {
    router: Router<CommandState>,
    umadb_client: Arc<AsyncUmaDBClient>,
}

impl CommandRouter {
    pub fn new(umadb_client: Arc<AsyncUmaDBClient>) -> Self {
        let router = Router::new();

        CommandRouter {
            router,
            umadb_client,
        }
    }

    pub fn build(self) -> Router {
        let store = Arc::new(MemoryStore::new());
        let idempotent_options = IdempotentOptions::default()
            .use_idempotency_key_header(Some("X-Idempotency-Key"))
            .ignore_response_status_code(StatusCode::CONFLICT)
            .expire_after(60 * 5);

        let router = self
            .router
            .layer(DefaultBodyLimit::max(256 * 1024))
            .layer(TimeoutLayer::with_status_code(
                StatusCode::REQUEST_TIMEOUT,
                Duration::from_secs(30),
            ))
            .layer(IdempotentLayer::<MemoryStore>::new(idempotent_options))
            .layer(
                SessionLayer::new(store)
                    .with_cookie_options(CookieOptions::build().name("session")),
            )
            .layer(CookieManagerLayer::new());

        router.with_state(CommandState {
            umadb_client: self.umadb_client,
        })
    }

    pub async fn serve<A: ToSocketAddrs>(self, addr: A) -> io::Result<()> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, self.build()).await
    }

    pub fn register_command<C>(mut self, name: &str) -> Self
    where
        C: Command + Send,
        C::Input: Send + 'static,
    {
        let route = |State(state): State<CommandState>, Json(input): Json<Value>| async move {
            let input: C::Input = serde_json::from_value(input).map_err(|err| {
                Error::new(ErrorStatus::InvalidInput, "invalid_command")
                    .with_message(err.to_string())
            })?;

            let result = C::execute(state.umadb_client.as_ref(), input).await?;

            if result.events.is_empty() {
                return Ok(Json(json!({
                    "status": "ok",
                    "events": [],
                    "position": result.position,
                })));
            }

            let resp_events: Vec<_> = result
                .events
                .into_iter()
                .map(|event| {
                    let data = match serde_json::from_slice(&event.data) {
                        Ok(data) => data,
                        Err(_) => Value::String(BASE64_STANDARD.encode(&event.data)),
                    };

                    json!({
                        "id": event.uuid,
                        "type": event.event_type,
                        "data": data,
                        "tags": event.tags,
                    })
                })
                .collect();

            Ok::<_, Error>(Json(json!({
                "status": "ok",
                "events": resp_events,
                "position": result.position,
            })))
        };

        self.router = self.router.route(&format!("/{name}"), post(route));
        self
    }
}

#[derive(Clone)]
struct CommandState {
    umadb_client: Arc<AsyncUmaDBClient>,
}
