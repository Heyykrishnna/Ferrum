mod host;
mod process;
mod stages;

pub use host::HostError;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use ferrum_domain::{LANGUAGES, RequestedLimits, SubmissionInput};
use tokio_util::sync::CancellationToken;

use crate::{
    CleanupDisposition, CleanupError, ExecutionReport, ExecutionResult, ProgramOutcome, Sandbox,
    SandboxError,
};
use host::Host;
use process::{capture, command};
use stages::{cancelled, run_in_box};
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
                "python",
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
    pub async fn open_with_toolchains() -> Result<Self, HostError> {
        let sandbox = Self::open().await?;
        tokio::task::spawn_blocking(move || {
            for profile in LANGUAGES {
                let toolchain = &profile.toolchain;
                let mut probe = command(toolchain.program);
                probe.args(toolchain.version_args);
                let output = capture(
                    &mut probe,
                    Duration::from_secs(3),
                    4096,
                    4096,
                    &CancellationToken::new(),
                )
                .map_err(|_| HostError::Toolchain(profile.language))?;
                let matches_version = [output.stdout.as_slice(), output.stderr.as_slice()]
                    .into_iter()
                    .filter_map(|bytes| std::str::from_utf8(bytes).ok())
                    .any(|text| text.trim().starts_with(toolchain.version_prefix));
                if !output.status.success() || output.stop.is_some() || !matches_version {
                    return Err(HostError::Toolchain(profile.language));
                }
            }
            Ok(sandbox)
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

        let (result, may_cleanup) = run_in_box(&self.host, input, cancellation);
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
}

fn report(
    result: Result<ExecutionResult, SandboxError>,
    cleanup: CleanupDisposition,
) -> ExecutionReport {
    ExecutionReport { result, cleanup }
}
