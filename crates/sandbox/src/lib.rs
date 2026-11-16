#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod isolate;

#[cfg(any(test, all(target_os = "linux", target_arch = "x86_64")))]
mod metadata;

use std::future::Future;

use ferrum_domain::SubmissionInput;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

pub trait Sandbox {
    fn execute(
        &self,
        input: SubmissionInput,
        cancellation: CancellationToken,
    ) -> impl Future<Output = ExecutionReport> + Send;
}

pub struct ExecutionReport {
    pub result: Result<ExecutionResult, SandboxError>,
    pub cleanup: CleanupDisposition,
}

pub struct ExecutionResult {
    pub outcome: ProgramOutcome,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub facts: ExecutionFacts,
    pub compilation: Option<StageResult>,
    pub cases: Option<Vec<ferrum_domain::CaseFinding>>,
    pub score: Option<u32>,
    pub max_score: Option<u32>,
}

pub struct StageResult {
    pub outcome: ProgramOutcome,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub facts: ExecutionFacts,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ExecutionFacts {
    pub exit_code: Option<u8>,
    pub signal: Option<u8>,
    pub cpu_time_ms: Option<u64>,
    pub wall_time_ms: Option<u64>,
    pub peak_memory_kib: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramOutcome {
    Success,
    CompilationError,
    RuntimeError,
    CpuTimeLimit,
    WallTimeLimit,
    MemoryLimit,
    OutputLimit(OutputStream),
    Cancelled,
}

impl ProgramOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::CompilationError => "compilation_error",
            Self::RuntimeError => "runtime_error",
            Self::CpuTimeLimit => "cpu_time_limit",
            Self::WallTimeLimit => "wall_time_limit",
            Self::MemoryLimit => "memory_limit",
            Self::OutputLimit(OutputStream::Stdout) => "output_limit_stdout",
            Self::OutputLimit(OutputStream::Stderr) => "output_limit_stderr",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupDisposition {
    NotNeeded,
    Verified,
    Unresolved(CleanupError),
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CleanupError {
    #[error("Isolate did not complete cleanup")]
    Helper,
    #[error("sandbox workspace, cgroup or active lock remains")]
    Residual,
    #[error("could not update the host recovery record")]
    Journal,
    #[error("the sandbox supervisor did not return a cleanup result")]
    Supervisor,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SandboxError {
    #[error("the sandbox slot is already running")]
    Busy,
    #[error("the sandbox slot requires host recovery")]
    RecoveryRequired,
    #[error("Isolate could not initialize a sandbox with the required controls")]
    Initialize,
    #[error("could not stage the validated submission input")]
    Stage,
    #[error("the sandbox supervisor failed")]
    Supervisor,
    #[error("Isolate did not return a valid program result")]
    Metadata,
}

pub const STERILE_ENV: &[(&str, &str)] = &[
    ("PATH", "/usr/local/bin:/usr/bin:/bin"),
    ("HOME", "/box"),
    ("TMPDIR", "/box"),
    ("LANG", "C.UTF-8"),
    ("LC_ALL", "C.UTF-8"),
];

pub fn validate_relative_sandbox_path(path: &str) -> Result<std::path::PathBuf, SandboxError> {
    use std::path::{Component, Path, PathBuf};

    let p = Path::new(path);
    if path.is_empty() || p.is_absolute() {
        return Err(SandboxError::Stage);
    }

    let mut clean = PathBuf::new();
    for component in p.components() {
        match component {
            Component::Normal(c) => clean.push(c),
            Component::CurDir => continue,
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(SandboxError::Stage);
            }
        }
    }

    if clean.as_os_str().is_empty() {
        return Err(SandboxError::Stage);
    }

    Ok(clean)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn validates_valid_relative_paths() {
        assert_eq!(
            validate_relative_sandbox_path("main.py").unwrap(),
            PathBuf::from("main.py")
        );
        assert_eq!(
            validate_relative_sandbox_path("src/lib.rs").unwrap(),
            PathBuf::from("src/lib.rs")
        );
        assert_eq!(
            validate_relative_sandbox_path("./src/./lib.rs").unwrap(),
            PathBuf::from("src/lib.rs")
        );
    }

    #[test]
    fn rejects_path_traversal_and_absolute_paths() {
        assert!(validate_relative_sandbox_path("../main.py").is_err());
        assert!(validate_relative_sandbox_path("src/../../etc/passwd").is_err());
        assert!(validate_relative_sandbox_path("/etc/passwd").is_err());
        assert!(validate_relative_sandbox_path("").is_err());
        assert!(validate_relative_sandbox_path(".").is_err());
        assert!(validate_relative_sandbox_path("./..").is_err());
    }
}
