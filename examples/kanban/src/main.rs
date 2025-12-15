use std::sync::Arc;

use umadb_client::UmaDBClient;

use crate::{projections::tasks::TasksProjection, tui::KanbanApp};

mod commands;
mod events;
mod projections;
mod tui;
mod types;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Arc::new(UmaDBClient::new("http://0.0.0.0:50051".to_string()).connect()?);
    let tasks_projection = Arc::new(TasksProjection::default());

    std::thread::spawn({
        let tasks_projection = tasks_projection.clone();
        let client = client.clone();
        move || {
            tasks_projection.run(&client).unwrap();
        }
    });

    KanbanApp::new(tasks_projection, client).run()?;

    Ok(())
}
