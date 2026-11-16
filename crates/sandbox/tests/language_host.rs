#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "fixtures/languages.rs"]
mod languages;

use ferrum_domain::{RequestedLimits, SubmissionInput, resolve_language};
use ferrum_sandbox::{
    CleanupDisposition, ExecutionResult, ProgramOutcome, Sandbox, isolate::Isolate,
};
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore = "requires a Phase 3-qualified Debian amd64 worker with all Phase 5 toolchains"]
async fn phase5_language_matrix() {
    let sandbox = Isolate::open_with_toolchains()
        .await
        .expect("qualified Phase 5 worker");
    for fixture in languages::FIXTURES {
        let profile = resolve_language(fixture.language).unwrap();
        let succeeded = run(
            &sandbox,
            fixture.language,
            fixture.echo,
            RequestedLimits::default(),
        )
        .await;
        assert_eq!(
            succeeded.outcome,
            ProgramOutcome::Success,
            "{} stdout {:?} stderr {:?}",
            fixture.language,
            succeeded.stdout,
            succeeded.stderr
        );
        assert_eq!(succeeded.stdout, b"ferrum phase 5\n");
        assert_eq!(succeeded.facts.exit_code, Some(0));
        assert_eq!(succeeded.compilation.is_some(), profile.compile.is_some());
        if let Some(compile) = succeeded.compilation {
            assert_eq!(compile.outcome, ProgramOutcome::Success);
            assert!(compile.facts.cpu_time_ms.is_some());
        }

        let failed = run(
            &sandbox,
            fixture.language,
            fixture.failure,
            RequestedLimits::default(),
        )
        .await;
        assert_eq!(
            failed.outcome,
            ProgramOutcome::RuntimeError,
            "{}",
            fixture.language
        );
        assert_eq!(failed.facts.exit_code, Some(7));

        let timed_out = run(
            &sandbox,
            fixture.language,
            fixture.wait,
            RequestedLimits {
                cpu_time_ms: Some(1_000),
                wall_time_ms: Some(1_000),
                memory_kib: None,
            },
        )
        .await;
        assert_eq!(
            timed_out.outcome,
            ProgramOutcome::WallTimeLimit,
            "{}",
            fixture.language
        );

        if profile.compile.is_some() {
            let malformed = run(
                &sandbox,
                fixture.language,
                "this is not valid source !",
                RequestedLimits::default(),
            )
            .await;
            assert_eq!(
                malformed.outcome,
                ProgramOutcome::CompilationError,
                "{}",
                fixture.language
            );
            assert!(malformed.stdout.is_empty() && malformed.stderr.is_empty());
            assert_eq!(malformed.facts.exit_code, None);
            let compilation = malformed.compilation.unwrap();
            assert_eq!(compilation.outcome, ProgramOutcome::CompilationError);
            assert!(!compilation.stderr.is_empty());
        }
        println!("PASS {} Isolate language profile", fixture.language);
    }
}

async fn run(
    sandbox: &Isolate,
    language: &str,
    source: &str,
    limits: RequestedLimits,
) -> ExecutionResult {
    let input = SubmissionInput::new(
        language,
        source.to_owned(),
        Some("ferrum phase 5\n".to_owned()),
        limits,
    )
    .unwrap();
    let report = sandbox.execute(input, CancellationToken::new()).await;
    assert_eq!(report.cleanup, CleanupDisposition::Verified, "{language}");
    report
        .result
        .unwrap_or_else(|error| panic!("{language}: {error}"))
}
