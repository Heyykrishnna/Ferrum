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

    pub fn as_storage(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Queued => "queued",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
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

    pub fn as_storage(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::WrongAnswer => "wrong_answer",
            Self::CompilationError => "compilation_error",
            Self::RuntimeError => "runtime_error",
            Self::TimeLimitExceeded => "time_limit_exceeded",
            Self::MemoryLimitExceeded => "memory_limit_exceeded",
            Self::OutputLimitExceeded => "output_limit_exceeded",
            Self::InternalError => "internal_error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubmissionResult {
    pub outcome: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<u8>,
    pub signal: Option<u8>,
    pub cpu_time_ms: Option<u64>,
    pub wall_time_ms: Option<u64>,
    pub peak_memory_kib: Option<u64>,
}

pub struct Submission {
    pub id: Uuid,
    pub state: ProcessingState,
    pub verdict: Option<Verdict>,
    pub result: Option<SubmissionResult>,
}
