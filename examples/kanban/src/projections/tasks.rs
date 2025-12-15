use std::{sync::Mutex, time::Duration};

use esruntime_sdk::prelude::EventSet;
use indexmap::IndexMap;
use umadb_client::SyncUmaDBClient;
use umadb_dcb::{DCBError, DCBEventStoreSync, DCBQuery, DCBQueryItem, DCBSequencedEvent};
use uuid::Uuid;

use crate::{
    events::{TaskCreated, TaskDeleted, TaskRenamed, TaskStatusChanged},
    types::TaskStatus,
};

#[derive(Default)]
pub struct TasksProjection {
    pub tasks: Mutex<IndexMap<Uuid, (String, TaskStatus)>>,
}

#[derive(EventSet)]
#[allow(clippy::enum_variant_names)]
enum Query {
    TaskCreated(TaskCreated),
    TaskRenamed(TaskRenamed),
    TaskStatusChanged(TaskStatusChanged),
    TaskDeleted(TaskDeleted),
}

impl TasksProjection {
    pub fn run(&self, client: &SyncUmaDBClient) -> Result<(), DCBError> {
        let mut stream = client.read(
            Some(
                DCBQuery::new().item(DCBQueryItem::new().types(Query::EVENT_TYPES.iter().copied())),
            ),
            None,
            false,
            None,
            true,
        )?;

        while let Some(DCBSequencedEvent { position: _, event }) = stream.next().transpose()? {
            let query = Query::from_event(&event.event_type, &event.data)
                .unwrap()
                .unwrap();

            {
                let mut tasks = self.tasks.lock().unwrap();
                match query {
                    Query::TaskCreated(TaskCreated {
                        task_id,
                        name,
                        status,
                    }) => {
                        tasks.insert(task_id, (name, status));
                    }
                    Query::TaskRenamed(TaskRenamed { task_id, name }) => {
                        if let Some(task) = tasks.get_mut(&task_id) {
                            task.0 = name;
                        }
                    }
                    Query::TaskStatusChanged(TaskStatusChanged { task_id, status }) => {
                        if let Some(task) = tasks.get_mut(&task_id) {
                            task.1 = status;
                        }
                    }
                    Query::TaskDeleted(TaskDeleted { task_id }) => {
                        tasks.swap_remove(&task_id);
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(250));
        }

        Ok(())
    }
}
