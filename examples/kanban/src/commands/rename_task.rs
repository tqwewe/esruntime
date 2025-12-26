use esruntime_sdk::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use crate::events::{TaskCreated, TaskDeleted, TaskRenamed};

#[derive(EventSet)]
pub enum Query {
    Created(TaskCreated),
    Renamed(TaskRenamed),
    Deleted(TaskDeleted),
}

#[derive(CommandInput, Deserialize)]
pub struct RenameTaskInput {
    #[domain_id]
    pub task_id: Uuid,
    pub name: String,
}

#[derive(Default)]
pub struct RenameTask {
    created: bool,
    deleted: bool,
    name: Option<String>,
}

impl Command for RenameTask {
    type Query = Query;
    type Input = RenameTaskInput;
    type Error = CommandError;

    fn apply(&mut self, event: Query) {
        match event {
            Query::Created(TaskCreated { name, .. }) => {
                self.created = true;
                self.name = Some(name);
            }
            Query::Renamed(TaskRenamed { name, .. }) => {
                self.name = Some(name);
            }
            Query::Deleted(TaskDeleted { .. }) => {
                self.deleted = true;
            }
        }
    }

    fn handle(self, input: RenameTaskInput) -> Result<Emit, CommandError> {
        if !self.created {
            return Err(CommandError::rejected("Task not created"));
        }

        if self.deleted {
            return Err(CommandError::rejected("Task deleted"));
        }

        if self.name.as_ref() == Some(&input.name) {
            return Ok(emit![]);
        }

        Ok(emit![TaskRenamed {
            task_id: input.task_id,
            name: input.name,
        }])
    }
}
