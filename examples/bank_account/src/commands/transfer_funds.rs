use std::collections::HashMap;

use esruntime_sdk::prelude::*;
use serde::Deserialize;

use crate::events::{OpenedAccount, ReceivedFunds, SentFunds};

/// Events this command reads
#[derive(EventSet)]
pub enum Query {
    OpenedAccount(OpenedAccount),
    SentFunds(SentFunds),
    ReceivedFunds(ReceivedFunds),
}

/// Command payload with domain ID bindings
#[derive(CommandInput, Deserialize)]
pub struct TransferFundsInput {
    #[domain_id("account_id")]
    pub source_account: String,
    #[domain_id("account_id")]
    pub dest_account: String,
    pub amount: f64,
}

/// Handler State
#[derive(Default)]
pub struct TransferFunds {
    /// Balance per account_id
    balances: HashMap<String, f64>,
    /// Which accounts are open
    open_accounts: HashMap<String, bool>,
}

/// Impementation
impl Command for TransferFunds {
    type Query = Query;
    type Input = TransferFundsInput;
    type Error = CommandError;

    fn apply(&mut self, event: Query, _meta: EventMeta) {
        match event {
            Query::OpenedAccount(ev) => {
                self.balances
                    .insert(ev.account_id.clone(), ev.initial_balance);
                self.open_accounts.insert(ev.account_id, true);
            }
            Query::SentFunds(ev) => {
                if let Some(balance) = self.balances.get_mut(&ev.account_id) {
                    *balance -= ev.amount;
                }
            }
            Query::ReceivedFunds(ev) => {
                if let Some(balance) = self.balances.get_mut(&ev.account_id) {
                    *balance += ev.amount;
                }
            }
        }
    }

    fn handle(&self, input: &TransferFundsInput) -> Result<Emit, CommandError> {
        // Validate source account
        if input.source_account != "god"
            && !self
                .open_accounts
                .get(&input.source_account)
                .copied()
                .unwrap_or(false)
        {
            return Err(CommandError::rejected("Source account not open"));
        }

        // Validate destination account
        if !self
            .open_accounts
            .get(&input.dest_account)
            .copied()
            .unwrap_or(false)
        {
            return Err(CommandError::rejected("Destination account not open"));
        }

        if input.dest_account == "god" {
            return Err(CommandError::rejected(
                "God has enough money, please dont send him more",
            ));
        }

        // Ensure not sending to the same person
        if input.source_account == input.dest_account {
            return Err(CommandError::rejected("You cannot send money to yourself"));
        }

        // Validate amount
        if input.amount <= 0.0 {
            return Err(CommandError::invalid_input("Amount must be positive"));
        }

        // Check sufficient funds
        let source_balance = self
            .balances
            .get(&input.source_account)
            .copied()
            .unwrap_or(0.0);
        if input.source_account != "god" && source_balance < input.amount {
            return Err(CommandError::rejected(format!(
                "Insufficient funds: available {}, requested {}",
                source_balance, input.amount
            )));
        }

        // Emit both events atomically
        Ok(Emit::new()
            .event(SentFunds {
                account_id: input.source_account.clone(),
                amount: input.amount,
                recipient_id: input.dest_account.clone(),
            })
            .event(ReceivedFunds {
                account_id: input.dest_account.clone(),
                amount: input.amount,
                sender_id: input.source_account.clone(),
            }))
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    fn apply_all(handler: &mut TransferFunds, events: impl IntoIterator<Item = Query>) {
        for event in events {
            handler.apply(
                event,
                EventMeta {
                    timestamp: Utc::now(),
                },
            );
        }
    }

    fn opened(account_id: &str, balance: f64) -> Query {
        Query::OpenedAccount(OpenedAccount {
            account_id: account_id.into(),
            initial_balance: balance,
        })
    }

    fn sent(account_id: &str, amount: f64, recipient_id: &str) -> Query {
        Query::SentFunds(SentFunds {
            account_id: account_id.into(),
            amount,
            recipient_id: recipient_id.into(),
        })
    }

    fn received(account_id: &str, amount: f64, sender_id: &str) -> Query {
        Query::ReceivedFunds(ReceivedFunds {
            account_id: account_id.into(),
            amount,
            sender_id: sender_id.into(),
        })
    }

    fn transfer(source: &str, dest: &str, amount: f64) -> TransferFundsInput {
        TransferFundsInput {
            source_account: source.into(),
            dest_account: dest.into(),
            amount,
        }
    }

    // =========================================================================
    // Success Cases
    // =========================================================================

    #[test]
    fn successful_transfer() {
        let mut handler = TransferFunds::default();
        apply_all(&mut handler, [opened("alice", 100.0), opened("bob", 50.0)]);

        let result = handler.handle(&transfer("alice", "bob", 30.0));

        let events = result.unwrap().into_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "SentFunds");
        assert_eq!(events[1].event_type, "ReceivedFunds");
    }

    #[test]
    fn transfer_entire_balance() {
        let mut handler = TransferFunds::default();
        apply_all(&mut handler, [opened("alice", 100.0), opened("bob", 0.0)]);

        let result = handler.handle(&transfer("alice", "bob", 100.0));

        assert!(result.is_ok());
    }

    #[test]
    fn transfer_after_receiving_funds() {
        let mut handler = TransferFunds::default();
        apply_all(
            &mut handler,
            [
                opened("alice", 50.0),
                opened("bob", 100.0),
                received("alice", 60.0, "bob"),
            ],
        );

        // Alice now has 50 + 60 = 110
        let result = handler.handle(&transfer("alice", "bob", 100.0));

        assert!(result.is_ok());
    }

    #[test]
    fn transfer_after_sending_funds() {
        let mut handler = TransferFunds::default();
        apply_all(
            &mut handler,
            [
                opened("alice", 100.0),
                opened("bob", 50.0),
                sent("alice", 30.0, "bob"),
            ],
        );

        // Alice now has 100 - 30 = 70
        let result = handler.handle(&transfer("alice", "bob", 70.0));

        assert!(result.is_ok());
    }

    // =========================================================================
    // Account Validation
    // =========================================================================

    #[test]
    fn fails_when_source_account_not_open() {
        let mut handler = TransferFunds::default();
        apply_all(&mut handler, [opened("bob", 50.0)]);

        let result = handler.handle(&transfer("alice", "bob", 30.0));

        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::Rejected);
        assert!(err.message.contains("Source account not open"));
    }

    #[test]
    fn fails_when_destination_account_not_open() {
        let mut handler = TransferFunds::default();
        apply_all(&mut handler, [opened("alice", 100.0)]);

        let result = handler.handle(&transfer("alice", "bob", 30.0));

        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::Rejected);
        assert!(err.message.contains("Destination account not open"));
    }

    #[test]
    fn fails_when_neither_account_open() {
        let handler = TransferFunds::default();

        let result = handler.handle(&transfer("alice", "bob", 30.0));

        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::Rejected);
    }

    #[test]
    fn fails_when_sending_to_self() {
        let mut handler = TransferFunds::default();
        apply_all(&mut handler, [opened("alice", 100.0)]);

        let result = handler.handle(&transfer("alice", "alice", 30.0));

        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::Rejected);
        assert!(err.message.contains("cannot send money to yourself"));
    }

    // =========================================================================
    // Amount Validation
    // =========================================================================

    #[test]
    fn fails_with_zero_amount() {
        let mut handler = TransferFunds::default();
        apply_all(&mut handler, [opened("alice", 100.0), opened("bob", 50.0)]);

        let result = handler.handle(&transfer("alice", "bob", 0.0));

        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
        assert!(err.message.contains("Amount must be positive"));
    }

    #[test]
    fn fails_with_negative_amount() {
        let mut handler = TransferFunds::default();
        apply_all(&mut handler, [opened("alice", 100.0), opened("bob", 50.0)]);

        let result = handler.handle(&transfer("alice", "bob", -50.0));

        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
        assert!(err.message.contains("Amount must be positive"));
    }

    // =========================================================================
    // Balance Validation
    // =========================================================================

    #[test]
    fn fails_with_insufficient_funds() {
        let mut handler = TransferFunds::default();
        apply_all(&mut handler, [opened("alice", 50.0), opened("bob", 50.0)]);

        let result = handler.handle(&transfer("alice", "bob", 100.0));

        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::Rejected);
        assert!(err.message.contains("Insufficient funds"));
        assert!(err.message.contains("available 50"));
        assert!(err.message.contains("requested 100"));
    }

    #[test]
    fn fails_when_balance_depleted_by_previous_transfers() {
        let mut handler = TransferFunds::default();
        apply_all(
            &mut handler,
            [
                opened("alice", 100.0),
                opened("bob", 50.0),
                sent("alice", 80.0, "bob"),
            ],
        );

        // Alice now has 100 - 80 = 20
        let result = handler.handle(&transfer("alice", "bob", 50.0));

        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::Rejected);
        assert!(err.message.contains("Insufficient funds"));
    }

    // =========================================================================
    // God Account Special Cases
    // =========================================================================

    #[test]
    fn god_can_send_without_open_account() {
        let mut handler = TransferFunds::default();
        apply_all(&mut handler, [opened("bob", 0.0)]);

        let result = handler.handle(&transfer("god", "bob", 1000.0));

        assert!(result.is_ok());
    }

    #[test]
    fn god_can_send_without_sufficient_balance() {
        let mut handler = TransferFunds::default();
        apply_all(&mut handler, [opened("bob", 0.0)]);

        let result = handler.handle(&transfer("god", "bob", 999_999_999.0));

        assert!(result.is_ok());
    }

    #[test]
    fn god_cannot_receive_funds() {
        let mut handler = TransferFunds::default();
        apply_all(&mut handler, [opened("alice", 100.0), opened("god", 0.0)]);

        let result = handler.handle(&transfer("alice", "god", 50.0));

        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::Rejected);
        assert!(err.message.contains("God has enough money"));
    }

    // =========================================================================
    // Event Content Verification
    // =========================================================================

    #[test]
    fn emitted_events_contain_correct_data() {
        let mut handler = TransferFunds::default();
        apply_all(&mut handler, [opened("alice", 100.0), opened("bob", 50.0)]);

        let result = handler.handle(&transfer("alice", "bob", 30.0));
        let events = result.unwrap().into_events();

        // Verify SentFunds
        let sent: SentFunds = serde_json::from_value(events[0].data.clone()).unwrap();
        assert_eq!(sent.account_id, "alice");
        assert_eq!(sent.amount, 30.0);
        assert_eq!(sent.recipient_id, "bob");

        // Verify ReceivedFunds
        let received: ReceivedFunds = serde_json::from_value(events[1].data.clone()).unwrap();
        assert_eq!(received.account_id, "bob");
        assert_eq!(received.amount, 30.0);
        assert_eq!(received.sender_id, "alice");
    }

    #[test]
    fn emitted_events_have_correct_domain_ids() {
        let mut handler = TransferFunds::default();
        apply_all(&mut handler, [opened("alice", 100.0), opened("bob", 50.0)]);

        let result = handler.handle(&transfer("alice", "bob", 30.0));
        let events = result.unwrap().into_events();

        // SentFunds should have account_id:alice and recipient_id:bob
        assert!(events[0].domain_ids.contains_key("account_id"));

        // ReceivedFunds should have account_id:bob and sender_id:alice
        assert!(events[1].domain_ids.contains_key("account_id"));
    }
}
