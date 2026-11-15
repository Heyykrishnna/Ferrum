mod host;
mod process;

pub use host::HostError;

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use ferrum_domain::{RequestedLimits, SubmissionInput};
use rustix::process::geteuid;
use tokio_util::sync::CancellationToken;

use crate::{
    CleanupDisposition, CleanupError, ExecutionFacts, ExecutionReport, ExecutionResult,
    ProgramOutcome, Sandbox, SandboxError, metadata::Metadata,
};
use host::{Host, ISOLATE, WORKSPACE, read_bounded};
use process::{ProcessError, Stop, capture, command};

const META_LIMIT: usize = 8192;
const STARTUP_PROBE: &str = r#"
import os
import resource
import socket
import sys
assert sys.version_info[:2] == (3, 13)
assert os.getuid() != 0
assert os.getcwd() == "/box"
assert os.statvfs("/usr").f_flag & os.ST_RDONLY
assert resource.getrlimit(resource.RLIMIT_NPROC) == (1, 1)
assert resource.getrlimit(resource.RLIMIT_FSIZE) == (65536, 65536)
assert resource.getrlimit(resource.RLIMIT_STACK) == (8388608, 8388608)
assert resource.getrlimit(resource.RLIMIT_NOFILE) == (64, 64)
assert set(os.environ) <= {"LC_CTYPE", "LIBC_FATAL_STDERR_"}
assert [name for _, name in socket.if_nameindex()] == ["lo"]
print(input())
"#;

pub struct Isolate {
    inner: Arc<Inner>,
}

struct Inner {
    host: Host,
    busy: AtomicBool,
    blocked: AtomicBool,
}

struct Slot(Arc<Inner>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::Release);
    }
}

impl Isolate {
    pub async fn open() -> Result<Self, HostError> {
        tokio::task::spawn_blocking(|| {
            let host = Host::open()?;
            let inner = Inner {
                host,
                busy: AtomicBool::new(false),
                blocked: AtomicBool::new(false),
            };
            let probe = SubmissionInput::new(
                SubmissionInput::LANGUAGE,
                STARTUP_PROBE.to_owned(),
                Some("ferrum-ready\n".to_owned()),
                RequestedLimits::default(),
            )
            .map_err(|_| HostError::Probe)?;
            let report = inner.run(probe, &CancellationToken::new());
            match report.cleanup {
                CleanupDisposition::Verified => {}
                CleanupDisposition::Unresolved(error) => return Err(HostError::Recovery(error)),
                CleanupDisposition::NotNeeded => return Err(HostError::Probe),
            }
            let probe = report.result.map_err(|_| HostError::Probe)?;
            if probe.outcome != ProgramOutcome::Success
                || probe.stdout != b"ferrum-ready\n"
                || !probe.stderr.is_empty()
                || probe.facts.peak_memory_kib.is_none()
            {
                return Err(HostError::Probe);
            }
            Ok(Self {
                inner: Arc::new(inner),
            })
        })
        .await
        .map_err(|_| HostError::Supervisor)?
    }
}

impl Sandbox for Isolate {
    async fn execute(
        &self,
        input: SubmissionInput,
        cancellation: CancellationToken,
    ) -> ExecutionReport {
        if self.inner.busy.swap(true, Ordering::AcqRel) {
            return report(Err(SandboxError::Busy), CleanupDisposition::NotNeeded);
        }
        let slot = Slot(Arc::clone(&self.inner));
        if self.inner.blocked.load(Ordering::Acquire) {
            return report(
                Err(SandboxError::RecoveryRequired),
                CleanupDisposition::NotNeeded,
            );
        }
        let token = cancellation.child_token();
        let _cancel_on_drop = token.clone().drop_guard();
        match tokio::task::spawn_blocking(move || slot.0.run(input, &token)).await {
            Ok(report) => report,
            Err(_) => {
                self.inner.blocked.store(true, Ordering::Release);
                report(
                    Err(SandboxError::Supervisor),
                    CleanupDisposition::Unresolved(CleanupError::Supervisor),
                )
            }
        }
    }
}

impl Inner {
    fn run(&self, input: SubmissionInput, cancellation: &CancellationToken) -> ExecutionReport {
        if cancellation.is_cancelled() {
            return report(Ok(cancelled()), CleanupDisposition::NotNeeded);
        }
        if let Err(error) = self.host.begin() {
            self.blocked.store(true, Ordering::Release);
            return report(
                Err(SandboxError::RecoveryRequired),
                CleanupDisposition::Unresolved(error),
            );
        }

        let (result, may_cleanup) = self.run_in_box(input, cancellation);
        let cleanup = if may_cleanup {
            self.host
                .cleanup()
                .map_or_else(CleanupDisposition::Unresolved, |()| {
                    CleanupDisposition::Verified
                })
        } else {
            CleanupDisposition::Unresolved(CleanupError::Supervisor)
        };
        if matches!(cleanup, CleanupDisposition::Unresolved(_)) {
            self.blocked.store(true, Ordering::Release);
        }
        report(result, cleanup)
    }

    fn run_in_box(
        &self,
        input: SubmissionInput,
        cancellation: &CancellationToken,
    ) -> (Result<ExecutionResult, SandboxError>, bool) {
        let limits = input.limits();
        let quota = format!(
            "--quota={},{}",
            limits.workspace_kib, limits.workspace_inodes
        );
        if let Err(error) = self.host.initialize(quota) {
            return (
                Err(SandboxError::Initialize),
                !matches!(error, ProcessError::Unreaped),
            );
        }
        if stage(&input).is_err() {
            return (Err(SandboxError::Stage), true);
        }
        if cancellation.is_cancelled() {
            return (Ok(cancelled()), true);
        }
        let metadata_path = self.host.metadata_path();
        if OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&metadata_path)
            .is_err()
        {
            return (Err(SandboxError::Stage), true);
        }

        let mut cmd = command(ISOLATE);
        cmd.args([
            "--cg",
            "--box-id=0",
            "-s",
            "--core=0",
            "--syscalls=65535",
            "--no-default-dirs",
            "--dir=/box=./box:rw:noexec",
            "--dir=/usr:norec",
            "--dir=/lib:norec",
            "--dir=/lib64:norec",
            "--dir=/proc=proc:fs",
            "--stdin=/box/input",
        ])
        .arg(format!("--meta={}", metadata_path.display()))
        .arg(format!("--time={}", seconds(limits.cpu_time_ms)))
        .arg(format!("--wall-time={}", seconds(limits.wall_time_ms)))
        .arg(format!("--cg-mem={}", limits.memory_kib))
        .arg(format!("--processes={}", limits.processes))
        .arg(format!("--open-files={}", limits.open_files))
        .arg(format!("--stack={}", limits.stack_kib))
        .arg(format!("--fsize={}", limits.file_bytes / 1024))
        .args([
            "--run",
            "--",
            "/usr/bin/python3",
            "-I",
            "-B",
            "/box/main.py",
        ]);

        let captured = match capture(
            &mut cmd,
            Duration::from_millis(u64::from(limits.wall_time_ms)) + Duration::from_secs(2),
            limits.stdout_bytes as usize,
            limits.stderr_bytes as usize,
            cancellation,
        ) {
            Ok(captured) => captured,
            Err(error) => {
                return (
                    Err(SandboxError::Supervisor),
                    !matches!(error, ProcessError::Unreaped),
                );
            }
        };

        if !self.host.unpopulated() || matches!(captured.stop, Some(Stop::Deadline)) {
            return (Err(SandboxError::Supervisor), true);
        }

        let bytes = read_bounded(metadata_path, META_LIMIT);
        let metadata = bytes
            .as_deref()
            .map_err(|_| SandboxError::Metadata)
            .and_then(Metadata::parse);
        let result = if captured.status.code().is_some_and(|code| code >= 2)
            || metadata.as_ref().is_ok_and(Metadata::supervisor_failed)
        {
            Err(SandboxError::Supervisor)
        } else if let Some(stop) = captured.stop {
            let outcome = match stop {
                Stop::Cancelled => Ok(ProgramOutcome::Cancelled),
                Stop::Output(stream) => Ok(ProgramOutcome::OutputLimit(stream)),
                Stop::Deadline => Err(SandboxError::Supervisor),
            };
            outcome.map(|outcome| ExecutionResult {
                outcome,
                stdout: captured.stdout,
                stderr: captured.stderr,
                facts: metadata.map(|value| value.facts).unwrap_or_default(),
            })
        } else {
            metadata.and_then(|metadata| {
                Ok(ExecutionResult {
                    outcome: metadata.outcome(captured.status.code())?,
                    stdout: captured.stdout,
                    stderr: captured.stderr,
                    facts: metadata.facts,
                })
            })
        };
        (result, true)
    }
}

fn stage(input: &SubmissionInput) -> io::Result<()> {
    let workspace = Path::new(WORKSPACE).join("box");
    let metadata = fs::symlink_metadata(&workspace)?;
    if !metadata.is_dir() || metadata.uid() != geteuid().as_raw() || metadata.mode() & 0o022 != 0 {
        return Err(io::Error::other("invalid Isolate staging directory"));
    }
    for (name, data) in [("main.py", input.source()), ("input", input.stdin())] {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(workspace.join(name))?
            .write_all(data.as_bytes())?;
    }
    Ok(())
}

fn seconds(milliseconds: u32) -> String {
    format!("{}.{:03}", milliseconds / 1000, milliseconds % 1000)
}

fn report(
    result: Result<ExecutionResult, SandboxError>,
    cleanup: CleanupDisposition,
) -> ExecutionReport {
    ExecutionReport { result, cleanup }
}

fn cancelled() -> ExecutionResult {
    ExecutionResult {
        outcome: ProgramOutcome::Cancelled,
        stdout: Vec::new(),
        stderr: Vec::new(),
        facts: ExecutionFacts::default(),
    }
}
