use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecuteSubmission {
    pub version: u8,
    pub submission_id: Uuid,
    pub organization_id: Uuid,
    pub attempt_id: Uuid,
}

impl ExecuteSubmission {
    pub const VERSION: u8 = 1;
}

pub struct SubmissionCreated {
    pub event_id: Uuid,
    pub organization_id: Uuid,
    pub submission_id: Uuid,
}

impl SubmissionCreated {
    pub const EVENT_TYPE: &str = "submission.created";
    pub const VERSION: i16 = 1;
}

pub struct SubmissionCompleted {
    pub event_id: Uuid,
    pub organization_id: Uuid,
    pub submission_id: Uuid,
}

impl SubmissionCompleted {
    pub const EVENT_TYPE: &str = "submission.completed";
    pub const VERSION: i16 = 1;
}
