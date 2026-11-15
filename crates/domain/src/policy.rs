use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_INPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestedLimits {
    pub cpu_time_ms: Option<u32>,
    pub wall_time_ms: Option<u32>,
    pub memory_kib: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EffectiveLimits {
    pub cpu_time_ms: u32,
    pub wall_time_ms: u32,
    pub memory_kib: u32,
    pub stack_kib: u32,
    pub processes: u32,
    pub open_files: u32,
    pub file_bytes: u32,
    pub workspace_kib: u32,
    pub workspace_inodes: u32,
    pub stdout_bytes: u32,
    pub stderr_bytes: u32,
}

pub struct SubmissionInput {
    source: String,
    stdin: String,
    limits: EffectiveLimits,
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("language must be python")]
    Language,
    #[error("source must contain 1 to 65536 UTF-8 bytes and no NUL characters")]
    Source,
    #[error("stdin must contain at most 65536 UTF-8 bytes and no NUL characters")]
    Stdin,
    #[error(
        "limits require CPU 1–2000 ms, wall 1–6000 ms and memory 1–131072 KiB; wall time must be at least CPU time"
    )]
    Limits,
}

impl SubmissionInput {
    pub const LANGUAGE: &str = "python";
    pub const PROFILE: &str = "python3.13-v1";

    pub fn new(
        language: &str,
        source: String,
        stdin: Option<String>,
        requested: RequestedLimits,
    ) -> Result<Self, ValidationError> {
        if language != Self::LANGUAGE {
            return Err(ValidationError::Language);
        }
        if source.is_empty() || source.len() > MAX_INPUT_BYTES || source.contains('\0') {
            return Err(ValidationError::Source);
        }
        let stdin = stdin.unwrap_or_default();
        if stdin.len() > MAX_INPUT_BYTES || stdin.contains('\0') {
            return Err(ValidationError::Stdin);
        }
        let limits = EffectiveLimits {
            cpu_time_ms: requested.cpu_time_ms.unwrap_or(2_000),
            wall_time_ms: requested.wall_time_ms.unwrap_or(6_000),
            memory_kib: requested.memory_kib.unwrap_or(131_072),
            stack_kib: 8_192,
            processes: 1,
            open_files: 64,
            file_bytes: 65_536,
            workspace_kib: 4_096,
            workspace_inodes: 128,
            stdout_bytes: 65_536,
            stderr_bytes: 65_536,
        };
        if !(1..=2_000).contains(&limits.cpu_time_ms)
            || !(limits.cpu_time_ms..=6_000).contains(&limits.wall_time_ms)
            || !(1..=131_072).contains(&limits.memory_kib)
        {
            return Err(ValidationError::Limits);
        }

        Ok(Self {
            source,
            stdin,
            limits,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn stdin(&self) -> &str {
        &self.stdin
    }

    pub fn limits(&self) -> &EffectiveLimits {
        &self.limits
    }
}
