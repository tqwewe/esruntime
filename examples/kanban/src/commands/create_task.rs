use esruntime_sdk::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use crate::{events::TaskCreated, types::TaskStatus};

#[derive(EventSet)]
pub enum Query {
    Created(TaskCreated),
}

#[derive(CommandInput, Deserialize)]
pub struct CreateTaskInput {
    #[domain_id]
    pub task_id: Uuid,
    pub name: String,
    pub status: TaskStatus,
}

#[derive(Default)]
pub struct CreateTask {
    created: bool,
}

impl Command for CreateTask {
    type Query = Query;
    type Input = CreateTaskInput;
    type Error = CommandError;

    fn apply(&mut self, event: Query) {
        match event {
            Query::Created(TaskCreated { .. }) => {
                self.created = true;
            }
        }
    }

    fn handle(self, input: CreateTaskInput) -> Result<Emit, CommandError> {
        if self.created {
            return Err(CommandError::rejected("Task already created"));
        }

        Ok(emit![TaskCreated {
            task_id: input.task_id,
            name: input.name,
            status: input.status
        }])
    }
}
