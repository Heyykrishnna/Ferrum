use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ProcessingState;

pub const MAX_BATCH_SIZE: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionCancelOutcome {
    Cancelled,
    CancellationRequested,
    AlreadyTerminal,
}

impl SubmissionCancelOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cancelled => "cancelled",
            Self::CancellationRequested => "cancellation_requested",
            Self::AlreadyTerminal => "already_terminal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchItemResult {
    pub idempotency_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<ProcessingState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchSubmissionResult {
    pub items: Vec<BatchItemResult>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_outcome_serialization() {
        assert_eq!(SubmissionCancelOutcome::Cancelled.as_str(), "cancelled");
        assert_eq!(
            SubmissionCancelOutcome::CancellationRequested.as_str(),
            "cancellation_requested"
        );
        assert_eq!(
            SubmissionCancelOutcome::AlreadyTerminal.as_str(),
            "already_terminal"
        );
    }

    #[test]
    fn batch_result_serialization() {
        let item = BatchItemResult {
            idempotency_key: "k1".into(),
            id: Some(Uuid::new_v4()),
            state: Some(ProcessingState::Created),
            error: None,
        };
        let batch = BatchSubmissionResult { items: vec![item] };
        let serialized = serde_json::to_string(&batch).unwrap();
        assert!(serialized.contains("k1"));
        assert!(serialized.contains("created"));
    }
}
