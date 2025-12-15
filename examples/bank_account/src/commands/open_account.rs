use esruntime_sdk::prelude::*;
use serde::Deserialize;

use crate::events::OpenedAccount;

/// Events this command reads
#[derive(EventSet)]
pub enum Query {
    OpenedAccount(OpenedAccount),
}

/// Command payload with domain ID bindings
#[derive(CommandInput, Deserialize)]
pub struct OpenAccountInput {
    #[domain_id]
    pub account_id: String,
    pub initial_balance: f64,
}

/// Handler State
#[derive(Default)]
pub struct OpenAccount {
    is_open: bool,
}

/// Implementation
impl Command for OpenAccount {
    type Query = Query;
    type Input = OpenAccountInput;

    fn apply(&mut self, event: Query) {
        match event {
            Query::OpenedAccount(OpenedAccount { .. }) => {
                self.is_open = true;
            }
        }
    }

    fn handle(self, input: OpenAccountInput) -> Result<Emit, CommandError> {
        if self.is_open {
            return Err(CommandError::rejected("Account already open"));
        }

        Ok(emit![OpenedAccount {
            account_id: input.account_id,
            initial_balance: input.initial_balance,
        }])
    }
}
