use std::sync::Arc;

use axum::{Router, routing::get};
use esruntime_sdk::prelude::Command;
use esruntime_server::CommandRouter;
use umadb_client::UmaDBClient;

use crate::commands::{
    open_account::{OpenAccount, OpenAccountInput},
    transfer_funds::TransferFunds,
};

mod commands;
mod events;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Arc::new(
        UmaDBClient::new("http://0.0.0.0:50051".to_string())
            .connect_async()
            .await?,
    );

    for (account_id, initial_balance) in [("ari", 10_000.0), ("salina", 35_000.0)] {
        OpenAccount::execute(
            client.as_ref(),
            OpenAccountInput {
                account_id: account_id.to_string(),
                initial_balance,
            },
        )
        .await?;
    }

    let command_router = CommandRouter::new(client)
        .register_command::<OpenAccount>("open_account")
        .register_command::<TransferFunds>("transfer_funds")
        .build();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    let server = axum::serve(
        listener,
        Router::new()
            .route(
                "/health",
                get(|| async { r#"{"status":"healthy","version":"0.1.0"}"# }),
            )
            .nest("/commands", command_router),
    );

    tokio::select! {
        res = server => {
           res?;
        }
        _ = tokio::signal::ctrl_c() => {}
    }

    Ok(())
}
