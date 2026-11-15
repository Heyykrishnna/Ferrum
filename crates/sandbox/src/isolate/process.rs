use std::{
    io::{self, Read},
    os::{fd::AsFd, unix::process::CommandExt},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use rustix::{
    fs::{OFlags, fcntl_getfl, fcntl_setfl},
    process::{Pid, Signal, kill_process},
};
use tokio_util::sync::CancellationToken;

use crate::OutputStream;

const POLL: Duration = Duration::from_millis(5);
const TERM_GRACE: Duration = Duration::from_secs(1);
const REAP_GRACE: Duration = Duration::from_secs(1);

#[derive(Clone, Copy)]
pub(super) enum Stop {
    Cancelled,
    Output(OutputStream),
    Deadline,
}

pub(super) struct Capture {
    pub(super) status: ExitStatus,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) stop: Option<Stop>,
}

#[derive(Debug)]
pub(super) enum ProcessError {
    Io,
    Unreaped,
}

pub(super) fn command(program: &str) -> Command {
    let mut command = Command::new(program);
    command
        .env_clear()
        .env("LC_ALL", "C")
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    command
}

pub(super) fn capture(
    command: &mut Command,
    deadline: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
    cancellation: &CancellationToken,
) -> Result<Capture, ProcessError> {
    let mut child = command.spawn().map_err(|_| ProcessError::Io)?;
    let result = poll_output(
        &mut child,
        deadline,
        stdout_limit,
        stderr_limit,
        cancellation,
    );
    if result.is_err() {
        reap_after_error(child)?;
    }
    result
}

fn poll_output(
    child: &mut Child,
    deadline: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
    cancellation: &CancellationToken,
) -> Result<Capture, ProcessError> {
    let mut stdout = Stream::new(child.stdout.take().ok_or(ProcessError::Io)?, stdout_limit)?;
    let mut stderr = Stream::new(child.stderr.take().ok_or(ProcessError::Io)?, stderr_limit)?;
    let end = Instant::now() + deadline;
    let mut stop = None;
    let mut status = None;
    let mut kill_at = end;
    let mut reap_at = None;
    loop {
        stdout.read_once()?;
        stderr.read_once()?;
        if status.is_none() {
            status = child.try_wait().map_err(|_| ProcessError::Io)?;
        }
        let now = Instant::now();
        let output_stop = if stdout.exceeded {
            Some(Stop::Output(OutputStream::Stdout))
        } else if stderr.exceeded {
            Some(Stop::Output(OutputStream::Stderr))
        } else {
            None
        };
        if stop.is_none() {
            stop = output_stop.or_else(|| {
                if status.is_some() {
                    None
                } else if cancellation.is_cancelled() {
                    Some(Stop::Cancelled)
                } else if now >= end {
                    Some(Stop::Deadline)
                } else {
                    None
                }
            });
            if stop.is_some() && status.is_none() {
                terminate(child);
                kill_at = now + TERM_GRACE;
            }
        }

        if let Some(status) = status {
            if stdout.done() && stderr.done() {
                return Ok(Capture {
                    status,
                    stdout: stdout.bytes,
                    stderr: stderr.bytes,
                    stop,
                });
            }
            if !stdout.progress && !stderr.progress {
                return Err(ProcessError::Io);
            }
        } else if let Some(reap_at) = reap_at {
            if now >= reap_at {
                return Err(ProcessError::Unreaped);
            }
        } else if stop.is_some() && now >= kill_at {
            let _ = child.kill();
            reap_at = Some(now + REAP_GRACE);
        }

        if !stdout.progress && !stderr.progress {
            thread::sleep(POLL);
        }
    }
}

fn terminate(child: &Child) {
    if let Some(pid) = i32::try_from(child.id()).ok().and_then(Pid::from_raw) {
        let _ = kill_process(pid, Signal::TERM);
    }
}

fn reap_after_error(mut child: Child) -> Result<(), ProcessError> {
    if matches!(child.try_wait(), Ok(Some(_))) {
        return Ok(());
    }
    terminate(&child);
    let term_end = Instant::now() + TERM_GRACE;
    let reap_end = term_end + REAP_GRACE;
    loop {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return Ok(());
        }
        if Instant::now() >= reap_end {
            thread::spawn(move || {
                let _ = child.wait();
            });
            return Err(ProcessError::Unreaped);
        }
        if Instant::now() >= term_end {
            let _ = child.kill();
        }
        thread::sleep(POLL);
    }
}

struct Stream<R> {
    reader: R,
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
    eof: bool,
    progress: bool,
}

impl<R: Read + AsFd> Stream<R> {
    fn new(reader: R, limit: usize) -> Result<Self, ProcessError> {
        let flags = fcntl_getfl(&reader).map_err(|_| ProcessError::Io)?;
        fcntl_setfl(&reader, flags | OFlags::NONBLOCK).map_err(|_| ProcessError::Io)?;
        Ok(Self {
            reader,
            bytes: Vec::new(),
            limit,
            exceeded: false,
            eof: false,
            progress: false,
        })
    }

    fn done(&self) -> bool {
        self.eof || self.exceeded
    }

    fn read_once(&mut self) -> Result<(), ProcessError> {
        self.progress = false;
        if self.done() {
            return Ok(());
        }
        let mut chunk = [0_u8; 8192];
        let remaining = self.limit - self.bytes.len();
        let read_limit = (remaining + 1).min(chunk.len());
        match self.reader.read(&mut chunk[..read_limit]) {
            Ok(0) => self.eof = true,
            Ok(count) => {
                self.bytes.extend_from_slice(&chunk[..count.min(remaining)]);
                self.exceeded = count > remaining;
                self.progress = true;
            }
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err(ProcessError::Io),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::ExitStatusExt;

    use super::*;

    #[test]
    fn capture_preserves_exact_cap_and_reports_overflow() {
        let token = CancellationToken::new();
        let mut exact = command("/usr/bin/printf");
        exact.arg("1234");
        let captured = capture(&mut exact, Duration::from_secs(1), 4, 4, &token).unwrap();
        assert!(captured.status.success());
        assert!(captured.stop.is_none());
        assert_eq!(captured.stdout, b"1234");

        let mut overflow = command("/usr/bin/printf");
        overflow.arg("12345");
        let captured = capture(&mut overflow, Duration::from_secs(1), 4, 4, &token).unwrap();
        assert!(matches!(
            captured.stop,
            Some(Stop::Output(OutputStream::Stdout))
        ));
        assert_eq!(captured.stdout, b"1234");
    }

    #[test]
    fn capture_reaps_cancelled_process() {
        let token = CancellationToken::new();
        token.cancel();
        let mut child = command("/usr/bin/sleep");
        child.arg("10");
        let captured = capture(&mut child, Duration::from_secs(5), 8, 8, &token).unwrap();
        assert!(matches!(captured.stop, Some(Stop::Cancelled)));
    }

    #[test]
    fn overflow_stops_and_reaps_a_live_writer() {
        let mut writer = command("/usr/bin/yes");
        writer.arg("fixture");
        let captured = capture(
            &mut writer,
            Duration::from_secs(5),
            4096,
            4096,
            &CancellationToken::new(),
        )
        .unwrap();

        assert!(matches!(
            captured.stop,
            Some(Stop::Output(OutputStream::Stdout))
        ));
        assert_eq!(captured.stdout.len(), 4096);
        assert_eq!(captured.status.signal(), Some(Signal::TERM.as_raw()));
        assert!(captured.stderr.is_empty());
    }

    #[test]
    fn deadline_kills_and_reaps_a_child_that_ignores_termination() {
        let mut child = command("/usr/bin/env");
        child.args(["--ignore-signal=TERM", "/usr/bin/sleep", "10"]);
        let captured = capture(
            &mut child,
            Duration::from_secs(1),
            8,
            8,
            &CancellationToken::new(),
        )
        .unwrap();

        assert!(matches!(captured.stop, Some(Stop::Deadline)));
        assert_eq!(captured.status.signal(), Some(Signal::KILL.as_raw()));
    }
}
