use esruntime_sdk::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use crate::events::{TaskCreated, TaskDeleted, TaskRenamed};

#[derive(EventSet)]
pub enum Query {
    TaskCreated(TaskCreated),
    TaskRenamed(TaskRenamed),
    TaskDeleted(TaskDeleted),
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

    fn apply(&mut self, event: Query) {
        match event {
            Query::TaskCreated(TaskCreated { name, .. }) => {
                self.created = true;
                self.name = Some(name);
            }
            Query::TaskRenamed(TaskRenamed { name, .. }) => {
                self.name = Some(name);
            }
            Query::TaskDeleted(TaskDeleted { .. }) => {
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
