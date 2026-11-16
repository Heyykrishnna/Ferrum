mod auth;
mod batch;
mod languages;
mod policy;
mod webhooks;

pub use auth::{ApiKey, ApiKeyScope, AuthContext};
pub use batch::{BatchItemResult, BatchSubmissionResult, MAX_BATCH_SIZE, SubmissionCancelOutcome};
pub use languages::{CommandPlan, CompilationPlan, LANGUAGES, LanguageProfile, resolve_language};
pub use policy::{EffectiveLimits, RequestedLimits, SubmissionInput, ValidationError};
pub use webhooks::{WebhookDelivery, WebhookEndpoint};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonMode {
    #[default]
    Exact,
    Whitespace,
    Tokens,
}

impl ComparisonMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Whitespace => "whitespace",
            Self::Tokens => "tokens",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        value.parse().ok()
    }
}

impl std::str::FromStr for ComparisonMode {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "exact" => Ok(Self::Exact),
            "whitespace" => Ok(Self::Whitespace),
            "tokens" => Ok(Self::Tokens),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestCase {
    pub index: u32,
    pub input: String,
    pub expected_output: String,
    #[serde(default = "default_case_weight")]
    pub weight: u32,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub comparison_mode: ComparisonMode,
}

fn default_case_weight() -> u32 {
    1
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseTelemetry {
    pub exit_code: Option<u8>,
    pub signal: Option<u8>,
    pub cpu_time_ms: Option<u64>,
    pub wall_time_ms: Option<u64>,
    pub peak_memory_kib: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseContent {
    pub input: String,
    pub expected_output: String,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseFinding {
    pub index: u32,
    pub verdict: Verdict,
    pub score: u32,
    pub max_score: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_time_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wall_time_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_memory_kib: Option<u64>,
}

impl CaseFinding {
    pub fn public(
        index: u32,
        verdict: Verdict,
        score: u32,
        max_score: u32,
        content: CaseContent,
        telemetry: CaseTelemetry,
    ) -> Self {
        Self {
            index,
            verdict,
            score,
            max_score,
            input: Some(content.input),
            expected_output: Some(content.expected_output),
            stdout: Some(content.stdout),
            stderr: Some(content.stderr),
            exit_code: telemetry.exit_code,
            signal: telemetry.signal,
            cpu_time_ms: telemetry.cpu_time_ms,
            wall_time_ms: telemetry.wall_time_ms,
            peak_memory_kib: telemetry.peak_memory_kib,
        }
    }

    pub fn hidden(
        index: u32,
        verdict: Verdict,
        score: u32,
        max_score: u32,
        telemetry: CaseTelemetry,
    ) -> Self {
        Self {
            index,
            verdict,
            score,
            max_score,
            input: None,
            expected_output: None,
            stdout: None,
            stderr: None,
            exit_code: None,
            signal: None,
            cpu_time_ms: telemetry.cpu_time_ms,
            wall_time_ms: telemetry.wall_time_ms,
            peak_memory_kib: telemetry.peak_memory_kib,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubmissionResult {
    #[serde(flatten)]
    pub execution: SubmissionStageResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compilation: Option<SubmissionStageResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_score: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cases: Option<Vec<CaseFinding>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SubmissionStageResult {
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
