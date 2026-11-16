#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "fixtures/languages.rs"]
mod languages;

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use ferrum_domain::{CommandPlan, resolve_language};
use uuid::Uuid;

const INPUT: &[u8] = b"ferrum phase 5\n";

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("ferrum-toolchain-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "requires the disposable Debian amd64 toolchain container"]
fn trusted_toolchain_commands_echo_and_report_errors() {
    assert!(Path::new("/.dockerenv").exists());
    for fixture in languages::FIXTURES {
        let profile = resolve_language(fixture.language).unwrap();
        let toolchain = &profile.toolchain;
        let version = Command::new(toolchain.program)
            .args(toolchain.version_args)
            .output()
            .unwrap();
        assert!(version.status.success(), "{}", fixture.language);
        assert!(
            [version.stdout, version.stderr].iter().any(|bytes| {
                String::from_utf8_lossy(bytes)
                    .trim()
                    .starts_with(toolchain.version_prefix)
            }),
            "{}",
            fixture.language
        );

        for (source, expected_exit) in [
            (fixture.echo, Some(0)),
            (fixture.failure, Some(7)),
            (fixture.wait, None),
        ] {
            let workspace = Workspace::new();
            fs::write(workspace.0.join(profile.source_file), source).unwrap();
            if let Some(compile) = &profile.compile {
                let output = execute(&compile.command, &workspace.0, b"");
                assert!(
                    output.status.success(),
                    "{} compiler: {}",
                    fixture.language,
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(workspace.0.join(compile.artifact_file).is_file());
            }
            let Some(expected_exit) = expected_exit else {
                continue;
            };
            let output = execute(&profile.run, &workspace.0, INPUT);
            assert_eq!(
                output.status.code(),
                Some(expected_exit),
                "{}: {}",
                fixture.language,
                String::from_utf8_lossy(&output.stderr)
            );
            if expected_exit == 0 {
                assert_eq!(output.stdout, INPUT, "{}", fixture.language);
            }
        }

        if let Some(compile) = &profile.compile {
            let workspace = Workspace::new();
            fs::write(
                workspace.0.join(profile.source_file),
                "this is not valid source !",
            )
            .unwrap();
            let output = execute(&compile.command, &workspace.0, b"");
            assert!(
                !output.status.success() && !output.stderr.is_empty(),
                "{}",
                fixture.language
            );
            assert!(!workspace.0.join(compile.artifact_file).exists());
        }
    }
}

fn execute(plan: &CommandPlan, workspace: &Path, stdin: &[u8]) -> Output {
    let expand = |text: &str| text.replace("/box", workspace.to_str().unwrap());
    let mut child = Command::new(expand(plan.program))
        .args(plan.args.iter().map(|arg| expand(arg)))
        .current_dir(workspace)
        .env_clear()
        .env("LC_ALL", "C")
        .env("PATH", "/usr/bin")
        .env("TMPDIR", workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}
