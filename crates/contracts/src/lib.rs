use uuid::Uuid;

pub struct SubmissionCreated {
    pub event_id: Uuid,
    pub organization_id: Uuid,
    pub submission_id: Uuid,
}

impl SubmissionCreated {
    pub const EVENT_TYPE: &str = "submission.created";
    pub const VERSION: i16 = 1;
}
