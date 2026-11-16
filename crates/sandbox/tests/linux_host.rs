#![cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

use std::{fs, net::TcpListener, path::Path, time::Duration};

use ferrum_domain::{RequestedLimits, SubmissionInput};
use ferrum_sandbox::{
    CleanupDisposition, ExecutionResult, OutputStream, ProgramOutcome, Sandbox, isolate::Isolate,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn input(source: &str, stdin: &str, limits: RequestedLimits) -> SubmissionInput {
    SubmissionInput::new("python", source.to_owned(), Some(stdin.to_owned()), limits).unwrap()
}

async fn run(
    sandbox: &Isolate,
    source: &str,
    stdin: &str,
    limits: RequestedLimits,
) -> ExecutionResult {
    let report = sandbox
        .execute(input(source, stdin, limits), CancellationToken::new())
        .await;
    assert_eq!(report.cleanup, CleanupDisposition::Verified);
    report.result.unwrap()
}

#[tokio::test]
#[ignore = "requires the dedicated Debian 13 amd64 or arm64 Isolate 2.7 host"]
async fn verifies_the_real_isolate_adapter() {
    let sandbox = Isolate::open()
        .await
        .expect("Isolate host and startup probe");
    let result = run(
        &sandbox,
        "import sys; print(input()); print('diagnostic', file=sys.stderr)",
        "hello Ferrum\n",
        RequestedLimits::default(),
    )
    .await;
    assert_eq!(result.outcome, ProgramOutcome::Success);
    assert_eq!(result.stdout, b"hello Ferrum\n");
    assert_eq!(result.stderr, b"diagnostic\n");
    assert_eq!(result.facts.exit_code, Some(0));
    assert!(result.facts.wall_time_ms.is_some() && result.facts.cpu_time_ms.is_some());
    assert!(result.facts.peak_memory_kib.is_some());
    println!("PASS echo, separated output, resource facts, cleanup");

    let result = run(
        &sandbox,
        "raise SystemExit(7)",
        "",
        RequestedLimits::default(),
    )
    .await;
    assert_eq!(result.outcome, ProgramOutcome::RuntimeError);
    assert_eq!(result.facts.exit_code, Some(7));
    println!("PASS nonzero program exit");

    let cpu = RequestedLimits {
        cpu_time_ms: Some(200),
        wall_time_ms: Some(3500),
        memory_kib: None,
    };
    let result = run(
        &sandbox,
        "import time\nend=time.monotonic()+2\nwhile time.monotonic()<end: pass\n",
        "",
        cpu,
    )
    .await;
    assert_eq!(result.outcome, ProgramOutcome::CpuTimeLimit);

    let wall = RequestedLimits {
        cpu_time_ms: Some(100),
        wall_time_ms: Some(700),
        memory_kib: None,
    };
    let result = run(&sandbox, "import time; time.sleep(3)", "", wall).await;
    assert_eq!(result.outcome, ProgramOutcome::WallTimeLimit);
    println!("PASS CPU and wall time limits");

    let memory = RequestedLimits {
        memory_kib: Some(32768),
        ..RequestedLimits::default()
    };
    let result = run(
        &sandbox,
        "buffer = bytearray(64 * 1024 * 1024)\nfor i in range(0, len(buffer), 4096): buffer[i] = 1",
        "",
        memory,
    )
    .await;
    assert_eq!(result.outcome, ProgramOutcome::MemoryLimit);
    println!("PASS cgroup memory limit");

    for (stream, fd) in [(OutputStream::Stdout, 1), (OutputStream::Stderr, 2)] {
        let result = run(
            &sandbox,
            &format!("import os; os.write({fd}, b'x' * 65537)"),
            "",
            RequestedLimits::default(),
        )
        .await;
        assert_eq!(result.outcome, ProgramOutcome::OutputLimit(stream));
        assert_eq!(
            if fd == 1 {
                result.stdout.len()
            } else {
                result.stderr.len()
            },
            65536
        );
    }
    let exact = run(
        &sandbox,
        "import os; os.write(1, b'x' * 65536)",
        "",
        RequestedLimits::default(),
    )
    .await;
    assert_eq!(exact.outcome, ProgramOutcome::Success);
    assert_eq!(exact.stdout.len(), 65536);
    println!("PASS independent output caps and exact-cap output");

    let caps = r#"
import errno
import os
try:
    pid = os.fork()
except BlockingIOError as error:
    assert error.errno == errno.EAGAIN
else:
    if pid == 0:
        os._exit(0)
    os.waitpid(pid, 0)
    raise AssertionError("a second process was allowed")
with open("file", "wb", buffering=0) as out:
    assert out.write(b"x" * 65536) == 65536
    try:
        out.write(b"x")
    except OSError as error:
        assert error.errno == errno.EFBIG
    else:
        raise AssertionError("file size limit absent")
for index in range(80):
    try:
        with open(f"quota-{index}", "wb") as out:
            out.write(b"x" * 65536)
    except OSError as error:
        assert error.errno == errno.EDQUOT
        break
else:
    raise AssertionError("block quota absent")
"#;
    assert_eq!(
        run(&sandbox, caps, "", RequestedLimits::default())
            .await
            .outcome,
        ProgramOutcome::Success
    );
    let inodes = r#"
import errno
for index in range(150):
    try:
        with open(f"inode-{index}", "xb"):
            pass
    except OSError as error:
        assert error.errno == errno.EDQUOT
        break
else:
    raise AssertionError("inode quota absent")
"#;
    assert_eq!(
        run(&sandbox, inodes, "", RequestedLimits::default())
            .await
            .outcome,
        ProgramOutcome::Success
    );
    println!("PASS process count, file size, disk quota and inode quota");

    let marker = format!("/tmp/ferrum-canary-{}", Uuid::new_v4());
    fs::write(&marker, b"synthetic").unwrap();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let visibility = r#"
import os
import socket
try:
    open(input(), "rb")
except FileNotFoundError:
    pass
else:
    raise AssertionError("host canary is visible")
assert os.readlink("/proc/self/ns/net") != input()
assert [name for _, name in socket.if_nameindex()] == ["lo"]
try:
    with socket.create_connection(("127.0.0.1", int(input())), timeout=0.5):
        raise AssertionError("host loopback is reachable")
except ConnectionRefusedError:
    pass
"#;
    let host_namespace = fs::read_link("/proc/self/ns/net").unwrap();
    let probe_stdin = format!(
        "{marker}\n{}\n{}\n",
        host_namespace.display(),
        listener.local_addr().unwrap().port()
    );
    let result = run(
        &sandbox,
        visibility,
        &probe_stdin,
        RequestedLimits::default(),
    )
    .await;
    fs::remove_file(marker).unwrap();
    assert_eq!(result.outcome, ProgramOutcome::Success);
    println!("PASS host filesystem and network separation");

    let group_root = fs::read_to_string("/run/isolate/cgroup").unwrap();
    let events = Path::new(group_root.trim()).join("box-0/cgroup.events");
    let token = CancellationToken::new();
    let task = sandbox.execute(
        input("import time; time.sleep(4)", "", RequestedLimits::default()),
        token.clone(),
    );
    let cancel_running = async {
        let started = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if fs::read_to_string(&events)
                    .is_ok_and(|value| value.lines().any(|line| line == "populated 1"))
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        token.cancel();
        assert!(started.is_ok(), "fixture never entered its cgroup");
    };
    let (cancelled, ()) = tokio::join!(task, cancel_running);
    assert_eq!(cancelled.cleanup, CleanupDisposition::Verified);
    assert_eq!(cancelled.result.unwrap().outcome, ProgramOutcome::Cancelled);
    assert!(!events.exists());
    assert_eq!(
        run(&sandbox, "print('reused')", "", RequestedLimits::default())
            .await
            .stdout,
        b"reused\n"
    );
    drop(sandbox);

    assert!(!Path::new("/var/lib/ferrum-sandbox/box-0.pending").exists());
    assert!(!Path::new("/var/lib/isolate/0").exists());
    assert!(!fs::metadata("/run/isolate/locks/0").is_ok_and(|metadata| metadata.len() != 0));
    println!("PASS cancellation, cgroup teardown and slot reuse");
}

#[tokio::test]
async fn root_and_docker_cannot_open_an_adapter() {
    if rustix::process::geteuid().is_root() || Path::new("/.dockerenv").exists() {
        assert!(matches!(
            Isolate::open().await,
            Err(ferrum_sandbox::isolate::HostError::Platform)
        ));
    }
}
