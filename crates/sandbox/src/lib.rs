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
    RuntimeError,
    CpuTimeLimit,
    WallTimeLimit,
    MemoryLimit,
    OutputLimit(OutputStream),
    Cancelled,
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
    #[error("could not stage the validated Python input")]
    Stage,
    #[error("the sandbox supervisor failed")]
    Supervisor,
    #[error("Isolate did not return a valid program result")]
    Metadata,
}
