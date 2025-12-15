use esruntime_sdk::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    events::{TaskCreated, TaskDeleted, TaskStatusChanged},
    types::TaskStatus,
};

#[derive(EventSet)]
pub enum Query {
    TaskCreated(TaskCreated),
    TaskStatusChanged(TaskStatusChanged),
    TaskDeleted(TaskDeleted),
}

#[derive(CommandInput, Deserialize)]
pub struct ChangeTaskStatusInput {
    #[domain_id]
    pub task_id: Uuid,
    pub status: TaskStatus,
}

#[derive(Default)]
pub struct ChangeTaskStatus {
    created: bool,
    deleted: bool,
    status: Option<TaskStatus>,
}

impl Command for ChangeTaskStatus {
    type Query = Query;
    type Input = ChangeTaskStatusInput;

    fn apply(&mut self, event: Query) {
        match event {
            Query::TaskCreated(TaskCreated { status, .. }) => {
                self.created = true;
                self.status = Some(status);
            }
            Query::TaskStatusChanged(TaskStatusChanged { status, .. }) => {
                self.status = Some(status);
            }
            Query::TaskDeleted(TaskDeleted { .. }) => {
                self.deleted = true;
            }
        }
    }

    fn handle(self, input: ChangeTaskStatusInput) -> Result<Emit, CommandError> {
        if !self.created {
            return Err(CommandError::rejected("Task not created"));
        }

        if self.deleted {
            return Err(CommandError::rejected("Task deleted"));
        }

        if self.status == Some(input.status) {
            return Ok(emit![]);
        }

        Ok(emit![TaskStatusChanged {
            task_id: input.task_id,
            status: input.status
        }])
    }
}
