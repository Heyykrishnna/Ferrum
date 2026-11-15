mod policy;

pub use policy::{EffectiveLimits, RequestedLimits, SubmissionInput, ValidationError};

use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingState {
    Created,
    Queued,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

impl ProcessingState {
    pub fn from_storage(value: &str) -> Option<Self> {
        Some(match value {
            "created" => Self::Created,
            "queued" => Self::Queued,
            "processing" => Self::Processing,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Accepted,
    WrongAnswer,
    CompilationError,
    RuntimeError,
    TimeLimitExceeded,
    MemoryLimitExceeded,
    OutputLimitExceeded,
    InternalError,
}

impl Verdict {
    pub fn from_storage(value: &str) -> Option<Self> {
        Some(match value {
            "accepted" => Self::Accepted,
            "wrong_answer" => Self::WrongAnswer,
            "compilation_error" => Self::CompilationError,
            "runtime_error" => Self::RuntimeError,
            "time_limit_exceeded" => Self::TimeLimitExceeded,
            "memory_limit_exceeded" => Self::MemoryLimitExceeded,
            "output_limit_exceeded" => Self::OutputLimitExceeded,
            "internal_error" => Self::InternalError,
            _ => return None,
        })
    }
}

pub struct Submission {
    pub id: Uuid,
    pub state: ProcessingState,
    pub verdict: Option<Verdict>,
}
