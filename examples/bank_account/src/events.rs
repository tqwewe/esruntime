use esruntime_sdk::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Event, Serialize, Deserialize)]
#[event_type("OpenedAccount")]
pub struct OpenedAccount {
    #[domain_id]
    pub account_id: String,
    pub initial_balance: f64,
}

#[derive(Clone, Debug, PartialEq, Event, Serialize, Deserialize)]
pub struct SentFunds {
    #[domain_id]
    pub account_id: String,
    pub amount: f64,
    #[domain_id]
    pub recipient_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Event, Serialize, Deserialize)]
pub struct ReceivedFunds {
    #[domain_id]
    pub account_id: String,
    pub amount: f64,
    #[domain_id]
    pub sender_id: Option<String>,
}
