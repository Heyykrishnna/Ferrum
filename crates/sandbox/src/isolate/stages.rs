use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    time::Duration,
};

use ferrum_domain::{CommandPlan, CompilationPlan, EffectiveLimits, SubmissionInput};
use rustix::process::geteuid;
use tokio_util::sync::CancellationToken;

use crate::{
    ExecutionFacts, ExecutionResult, ProgramOutcome, SandboxError, StageResult, metadata::Metadata,
};

use super::{
    host::{Host, ISOLATE, WORKSPACE, read_bounded},
    process::{ProcessError, Stop, capture, command},
};

const META_LIMIT: usize = 8192;

type StageResponse = Result<StageResult, (SandboxError, bool)>;

pub(super) fn run_in_box(
    host: &Host,
    input: SubmissionInput,
    cancellation: &CancellationToken,
) -> (Result<ExecutionResult, SandboxError>, bool) {
    let result = run_stages(host, &input, input.profile().compile.as_ref(), cancellation);
    match result {
        Ok(result) => (Ok(result), true),
        Err((error, may_cleanup)) => (Err(error), may_cleanup),
    }
}

fn run_stages(
    host: &Host,
    input: &SubmissionInput,
    compiler: Option<&CompilationPlan>,
    cancellation: &CancellationToken,
) -> Result<ExecutionResult, (SandboxError, bool)> {
    let limits = input.limits();
    let quota = format!(
        "--quota={},{}",
        limits.workspace_kib, limits.workspace_inodes
    );
    host.initialize(quota).map_err(|err| {
        (
            SandboxError::Initialize,
            !matches!(err, ProcessError::Unreaped),
        )
    })?;
    stage_file(input.profile().source_file, input.source())
        .map_err(|_| (SandboxError::Stage, true))?;
    if cancellation.is_cancelled() {
        return Ok(cancelled());
    }

    let profile = input.profile();
    let mut compilation = None;
    if let Some(plan) = compiler {
        let limits = &plan.limits;
        let mut compiled = run_stage(host, &plan.command, limits, false, cancellation)?;
        if compiled.outcome == ProgramOutcome::RuntimeError {
            compiled.outcome = ProgramOutcome::CompilationError;
        }
        if compiled.outcome == ProgramOutcome::Success && !has_artifact(plan.artifact_file) {
            compiled.outcome = ProgramOutcome::CompilationError;
            let remaining = (limits.stderr_bytes as usize).saturating_sub(compiled.stderr.len());
            let message = b"Compilation did not produce the required entry point.\n";
            compiled
                .stderr
                .extend_from_slice(&message[..message.len().min(remaining)]);
        }
        if compiled.outcome != ProgramOutcome::Success {
            return Ok(skipped(compiled.outcome, Some(compiled)));
        }
        compilation = Some(compiled);
        if cancellation.is_cancelled() {
            return Ok(skipped(ProgramOutcome::Cancelled, compilation));
        }
        fs::remove_file(host.metadata_path()).map_err(|_| (SandboxError::Stage, true))?;
    }

    stage_file("input", input.stdin()).map_err(|_| (SandboxError::Stage, true))?;
    let runtime = run_stage(host, &profile.run, limits, true, cancellation)?;
    let mut facts = runtime.facts;
    if compilation.is_some() {
        facts.peak_memory_kib = None;
    }
    Ok(ExecutionResult {
        outcome: runtime.outcome,
        stdout: runtime.stdout,
        stderr: runtime.stderr,
        facts,
        compilation,
    })
}

fn run_stage(
    host: &Host,
    plan: &CommandPlan,
    limits: &EffectiveLimits,
    runtime: bool,
    cancellation: &CancellationToken,
) -> StageResponse {
    let metadata_path = host.metadata_path();
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&metadata_path)
        .map_err(|_| (SandboxError::Stage, true))?;

    let mut cmd = command(ISOLATE);
    let workspace = if runtime && plan.program == "/box/program" {
        "--dir=/box=./box:norec"
    } else {
        "--dir=/box=./box:rw:noexec:norec"
    };
    cmd.args([
        "--cg",
        "--box-id=0",
        "-s",
        "--core=0",
        "--syscalls=65535",
        "--no-default-dirs",
        workspace,
        "--dir=/usr:norec",
        "--dir=/lib:norec",
        "--dir=/lib64:norec",
        "--dir=/proc=proc:fs",
    ]);
    if runtime {
        cmd.arg("--stdin=/box/input");
    } else {
        cmd.args(["--env=PATH=/usr/bin", "--env=TMPDIR=/box"]);
    }
    if plan.program.starts_with("/usr/lib/jvm/") {
        cmd.arg("--dir=/etc/java-21-openjdk:norec:noexec");
    }
    cmd.arg(format!("--meta={}", metadata_path.display()))
        .arg(format!("--time={}", seconds(limits.cpu_time_ms)))
        .arg(format!("--wall-time={}", seconds(limits.wall_time_ms)))
        .arg(format!("--cg-mem={}", limits.memory_kib))
        .arg(format!("--processes={}", limits.processes))
        .arg(format!("--open-files={}", limits.open_files))
        .arg(format!("--stack={}", limits.stack_kib))
        .arg(format!("--fsize={}", limits.file_bytes / 1024))
        .args(["--run", "--", plan.program])
        .args(plan.args);

    let captured = capture(
        &mut cmd,
        Duration::from_millis(u64::from(limits.wall_time_ms)) + Duration::from_secs(2),
        limits.stdout_bytes as usize,
        limits.stderr_bytes as usize,
        cancellation,
    )
    .map_err(|err| {
        (
            SandboxError::Supervisor,
            !matches!(err, ProcessError::Unreaped),
        )
    })?;

    if !host.unpopulated() || matches!(captured.stop, Some(Stop::Deadline)) {
        return Err((SandboxError::Supervisor, true));
    }
    let bytes = read_bounded(metadata_path, META_LIMIT);
    let metadata = bytes
        .as_deref()
        .map_err(|_| SandboxError::Metadata)
        .and_then(Metadata::parse);
    if captured.status.code().is_some_and(|code| code >= 2)
        || metadata.as_ref().is_ok_and(Metadata::supervisor_failed)
    {
        return Err((SandboxError::Supervisor, true));
    }

    let (outcome, facts) = match captured.stop {
        Some(Stop::Cancelled) => (
            ProgramOutcome::Cancelled,
            metadata.map(|data| data.facts).unwrap_or_default(),
        ),
        Some(Stop::Output(stream)) => (
            ProgramOutcome::OutputLimit(stream),
            metadata.map(|data| data.facts).unwrap_or_default(),
        ),
        Some(Stop::Deadline) => return Err((SandboxError::Supervisor, true)),
        None => {
            let data = metadata.map_err(|err| (err, true))?;
            let outcome = data
                .outcome(captured.status.code())
                .map_err(|err| (err, true))?;
            (outcome, data.facts)
        }
    };

    Ok(StageResult {
        outcome,
        stdout: captured.stdout,
        stderr: captured.stderr,
        facts,
    })
}

fn stage_file(name: &str, data: &str) -> io::Result<()> {
    let workspace = Path::new(WORKSPACE).join("box");
    let metadata = fs::symlink_metadata(&workspace)?;
    if !metadata.is_dir() || metadata.uid() != geteuid().as_raw() || metadata.mode() & 0o022 != 0 {
        return Err(io::Error::other("invalid Isolate staging directory"));
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(workspace.join(name))?
        .write_all(data.as_bytes())
}

fn has_artifact(name: &str) -> bool {
    fs::symlink_metadata(Path::new(WORKSPACE).join("box").join(name)).is_ok_and(|metadata| {
        metadata.is_file()
            && metadata.uid() == geteuid().as_raw()
            && metadata.nlink() == 1
            && metadata.len() != 0
    })
}

fn seconds(milliseconds: u32) -> String {
    format!("{}.{:03}", milliseconds / 1000, milliseconds % 1000)
}

pub(super) fn cancelled() -> ExecutionResult {
    skipped(ProgramOutcome::Cancelled, None)
}

fn skipped(outcome: ProgramOutcome, compilation: Option<StageResult>) -> ExecutionResult {
    ExecutionResult {
        outcome,
        stdout: Vec::new(),
        stderr: Vec::new(),
        facts: ExecutionFacts::default(),
        compilation,
    }
}

#[cfg(test)]
mod tests {
    use ferrum_domain::{LANGUAGES, RequestedLimits};

    use super::*;
    use crate::CleanupDisposition;

    #[test]
    #[ignore = "requires the dedicated Debian amd64 Isolate worker and all toolchains"]
    fn compiler_deadlines_skip_run_for_every_compiled_profile() {
        let host = Host::open().expect("qualified worker");
        for profile in LANGUAGES {
            let Some(plan) = &profile.compile else {
                continue;
            };
            let source = match profile.language {
                "java" => "public class Main { public static void main(String[] args) {} }",
                "rust" => "fn main() {}",
                _ => "int main() { return 0; }",
            };
            let input = SubmissionInput::new(
                profile.language,
                source.into(),
                None,
                RequestedLimits::default(),
            )
            .unwrap();
            let compiler = CompilationPlan {
                command: CommandPlan {
                    program: plan.command.program,
                    args: plan.command.args,
                },
                artifact_file: plan.artifact_file,
                limits: EffectiveLimits {
                    cpu_time_ms: 1,
                    wall_time_ms: 1,
                    ..plan.limits.clone()
                },
            };
            host.begin().expect("reserve sandbox");
            let execution = run_stages(&host, &input, Some(&compiler), &CancellationToken::new());
            let cleanup = host
                .cleanup()
                .map_or_else(CleanupDisposition::Unresolved, |()| {
                    CleanupDisposition::Verified
                });
            assert_eq!(cleanup, CleanupDisposition::Verified);
            let result = execution.unwrap();
            assert!(
                matches!(
                    result.outcome,
                    ProgramOutcome::WallTimeLimit | ProgramOutcome::CpuTimeLimit
                ),
                "{}: {:?}",
                profile.language,
                result.outcome
            );
            assert_eq!(result.compilation.unwrap().outcome, result.outcome);
            assert!(result.stdout.is_empty() && result.stderr.is_empty());
            assert_eq!(result.facts, ExecutionFacts::default());
        }
    }
}
