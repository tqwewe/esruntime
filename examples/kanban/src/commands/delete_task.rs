use esruntime_sdk::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use crate::events::{TaskCreated, TaskDeleted};

#[derive(EventSet)]
pub enum Query {
    Created(TaskCreated),
    Deleted(TaskDeleted),
}

#[derive(CommandInput, Deserialize)]
pub struct DeleteTaskInput {
    #[domain_id]
    pub task_id: Uuid,
}

#[derive(Default)]
pub struct DeleteTask {
    created: bool,
    deleted: bool,
}

impl Command for DeleteTask {
    type Query = Query;
    type Input = DeleteTaskInput;
    type Error = CommandError;

    fn apply(&mut self, event: Query, _meta: EventMeta) {
        match event {
            Query::Created(TaskCreated { .. }) => {
                self.created = true;
            }
            Query::Deleted(TaskDeleted { .. }) => {
                self.deleted = true;
            }
        }
    }

    fn handle(&self, input: &DeleteTaskInput) -> Result<Emit, CommandError> {
        if !self.created {
            return Err(CommandError::rejected("Task not created"));
        }

        if self.deleted {
            return Ok(emit![]);
        }

        Ok(emit![TaskDeleted {
            task_id: input.task_id,
        }])
    }
}
