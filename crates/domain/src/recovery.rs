use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaleAttemptAction {
    Requeued,
    MarkedFailed,
}

impl StaleAttemptAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Requeued => "requeued",
            Self::MarkedFailed => "marked_failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconciledAttempt {
    pub submission_id: Uuid,
    pub attempt_id: Uuid,
    pub action: StaleAttemptAction,
    pub attempt_number: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuarantinedAttempt {
    pub organization_id: Uuid,
    pub submission_id: Uuid,
    pub attempt_id: Uuid,
    pub worker_id: Uuid,
    pub failure_reason: Option<String>,
    pub cleanup_error: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReconciliationSummary {
    pub stale_attempts_reconciled: usize,
    pub stuck_outbox_events_reconciled: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_attempt_action_display() {
        assert_eq!(StaleAttemptAction::Requeued.as_str(), "requeued");
        assert_eq!(StaleAttemptAction::MarkedFailed.as_str(), "marked_failed");
    }

    #[test]
    fn reconciliation_summary_serialization() {
        let summary = ReconciliationSummary {
            stale_attempts_reconciled: 2,
            stuck_outbox_events_reconciled: 1,
        };
        let serialized = serde_json::to_string(&summary).unwrap();
        assert!(serialized.contains("\"stale_attempts_reconciled\":2"));
        assert!(serialized.contains("\"stuck_outbox_events_reconciled\":1"));
    }
}
