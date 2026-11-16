use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecuteSubmission {
    pub version: u8,
    pub submission_id: Uuid,
    pub organization_id: Uuid,
    pub attempt_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traceparent: Option<String>,
}

impl ExecuteSubmission {
    pub const VERSION: u8 = 1;

    pub fn new(submission_id: Uuid, organization_id: Uuid, attempt_id: Uuid) -> Self {
        Self {
            version: Self::VERSION,
            submission_id,
            organization_id,
            attempt_id,
            traceparent: None,
        }
    }

    pub fn with_traceparent(mut self, traceparent: impl Into<String>) -> Self {
        self.traceparent = Some(traceparent.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionCreated {
    pub event_id: Uuid,
    pub organization_id: Uuid,
    pub submission_id: Uuid,
    pub traceparent: Option<String>,
}

impl SubmissionCreated {
    pub const EVENT_TYPE: &str = "submission.created";
    pub const VERSION: i16 = 1;

    pub fn new(event_id: Uuid, organization_id: Uuid, submission_id: Uuid) -> Self {
        Self {
            event_id,
            organization_id,
            submission_id,
            traceparent: None,
        }
    }

    pub fn with_traceparent(mut self, traceparent: impl Into<String>) -> Self {
        self.traceparent = Some(traceparent.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionCompleted {
    pub event_id: Uuid,
    pub organization_id: Uuid,
    pub submission_id: Uuid,
    pub traceparent: Option<String>,
}

impl SubmissionCompleted {
    pub const EVENT_TYPE: &str = "submission.completed";
    pub const VERSION: i16 = 1;

    pub fn new(event_id: Uuid, organization_id: Uuid, submission_id: Uuid) -> Self {
        Self {
            event_id,
            organization_id,
            submission_id,
            traceparent: None,
        }
    }

    pub fn with_traceparent(mut self, traceparent: impl Into<String>) -> Self {
        self.traceparent = Some(traceparent.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_submission_backwards_compatible() {
        let legacy_json = r#"{"version":1,"submission_id":"00000000-0000-0000-0000-000000000001","organization_id":"00000000-0000-0000-0000-000000000002","attempt_id":"00000000-0000-0000-0000-000000000003"}"#;
        let parsed: ExecuteSubmission = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.traceparent, None);
    }

    #[test]
    fn execute_submission_with_traceparent_roundtrip() {
        let job = ExecuteSubmission::new(Uuid::nil(), Uuid::nil(), Uuid::nil())
            .with_traceparent("00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01");
        let json = serde_json::to_string(&job).unwrap();
        let parsed: ExecuteSubmission = serde_json::from_str(&json).unwrap();
        assert_eq!(job, parsed);
    }

    #[test]
    fn execute_submission_rejects_unknown_fields() {
        let invalid_json = r#"{"version":1,"submission_id":"00000000-0000-0000-0000-000000000001","organization_id":"00000000-0000-0000-0000-000000000002","attempt_id":"00000000-0000-0000-0000-000000000003","extra":"bad"}"#;
        let err = serde_json::from_str::<ExecuteSubmission>(invalid_json);
        assert!(err.is_err());
    }
}
