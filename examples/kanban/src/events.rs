use esruntime_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::TaskStatus;

#[derive(Clone, Debug, PartialEq, Event, Serialize, Deserialize)]
pub struct TaskCreated {
    #[domain_id]
    pub task_id: Uuid,
    pub name: String,
    pub status: TaskStatus,
}

#[derive(Clone, Debug, PartialEq, Event, Serialize, Deserialize)]
pub struct TaskRenamed {
    #[domain_id]
    pub task_id: Uuid,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Event, Serialize, Deserialize)]
pub struct TaskStatusChanged {
    #[domain_id]
    pub task_id: Uuid,
    pub status: TaskStatus,
}

#[derive(Clone, Debug, PartialEq, Event, Serialize, Deserialize)]
pub struct TaskDeleted {
    #[domain_id]
    pub task_id: Uuid,
}
