use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

use rustix::{fs::OFlags, process::geteuid};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use super::process::{Capture, ProcessError, capture, command};
use crate::CleanupError;

pub(super) const ISOLATE: &str = "/usr/bin/isolate";
pub(super) const WORKSPACE: &str = "/var/lib/isolate/0";
const LOCK: &str = "/run/isolate/locks/0";
const STATE: &str = "/var/lib/ferrum-sandbox";
const CGROUP_ROOT: &str = "/sys/fs/cgroup";
const JOURNAL: &str = "box-0.pending";
const RETRY: &str = "box-0.recoverable";
const METADATA: &str = "box-0.meta";
const MARKER: &str = "ferrum-isolate-2.7-box-0:";
const JOURNAL_LIMIT: usize = 128;
const CONTROL_LIMIT: usize = 8192;
const COMMAND_BUDGET: Duration = Duration::from_secs(3);

#[derive(Debug, Error)]
pub enum HostError {
    #[error("Isolate requires a non-root Debian 13 amd64 worker with the Linux 6.12 profile")]
    Platform,
    #[error("the required Isolate 2.7 package or configuration is unavailable")]
    Isolate,
    #[error(
        "the required Isolate cgroup v2 service, controllers or process cleanup are unavailable"
    )]
    Cgroups,
    #[error(
        "the dedicated quota-enabled sandbox tmpfs or required kernel settings are unavailable"
    )]
    Workspace,
    #[error("create /var/lib/ferrum-sandbox as the worker account with mode 0700")]
    State,
    #[error("another Ferrum process owns Isolate box 0")]
    Busy,
    #[error("Isolate box 0 is occupied without a Ferrum recovery record")]
    UnownedBox,
    #[error("pending sandbox recovery failed: {0}")]
    Recovery(CleanupError),
    #[error("Isolate failed the startup execution probe")]
    Probe,
    #[error("the startup supervisor failed")]
    Supervisor,
}

pub(super) struct Host {
    pub(super) group: PathBuf,
    state: PathBuf,
    boot_id: String,
    _lock: File,
}

impl Host {
    pub(super) fn open() -> Result<Self, HostError> {
        let uid = geteuid().as_raw();
        let distro = read_text("/etc/os-release").map_err(|_| HostError::Platform)?;
        let kernel = read_text("/proc/sys/kernel/osrelease").map_err(|_| HostError::Platform)?;
        if uid == 0
            || !distro.lines().any(|line| line == "ID=debian")
            || !distro.lines().any(|line| line == "VERSION_ID=\"13\"")
            || !kernel.starts_with("6.12.")
            || host_command("/usr/bin/systemd-detect-virt", &["--container", "--quiet"])
                .is_none_or(|result| result.status.code() != Some(1))
        {
            return Err(HostError::Platform);
        }

        let helper = fs::symlink_metadata(ISOLATE).map_err(|_| HostError::Isolate)?;
        if !helper.is_file()
            || helper.uid() != 0
            || helper.mode() & 0o4022 != 0o4000
            || !host_success("/usr/bin/test", &["-x", "/usr/bin/python3"])
            || !host_text(ISOLATE, &["--version"])
                .is_some_and(|value| value.lines().next() == Some("The process isolator 2.7"))
            || !host_text(ISOLATE, &["-vv", "--check-config"]).is_some_and(|value| {
                value.lines().any(|line| {
                    line.starts_with(
                        "Config: box_root=/var/lib/isolate lock_root=/run/isolate/locks ",
                    )
                }) && value
                    .lines()
                    .any(|line| line == "Config: restricted_init=0 syscall_flags=65535")
            })
        {
            return Err(HostError::Isolate);
        }
        if !host_success(
            "/usr/bin/systemctl",
            &["is-active", "--quiet", "isolate.service"],
        ) {
            return Err(HostError::Cgroups);
        }
        let group_root = host_text(ISOLATE, &["--cg", "--box-id=0", "--print-cg-root"])
            .ok_or(HostError::Cgroups)?;
        let group_root = fs::canonicalize(group_root.trim()).map_err(|_| HostError::Cgroups)?;
        let controllers =
            read_text(group_root.join("cgroup.subtree_control")).map_err(|_| HostError::Cgroups)?;
        if !group_root.starts_with(CGROUP_ROOT)
            || group_root == Path::new(CGROUP_ROOT)
            || !["memory", "cpuset"].iter().all(|controller| {
                controllers
                    .split_whitespace()
                    .any(|current| current == *controller)
            })
        {
            return Err(HostError::Cgroups);
        }

        let mount = host_text(
            "/usr/bin/findmnt",
            &[
                "--noheadings",
                "--target",
                "/var/lib/isolate",
                "--output",
                "TARGET,FSTYPE,OPTIONS",
            ],
        )
        .ok_or(HostError::Workspace)?;
        let mut columns = mount.split_ascii_whitespace();
        if columns.next() != Some("/var/lib/isolate")
            || columns.next() != Some("tmpfs")
            || !columns.next().is_some_and(|options| {
                let options = options.split(',').collect::<Vec<_>>();
                (options.contains(&"usrquota") || options.contains(&"quota"))
                    && options.contains(&"nodev")
                    && options.contains(&"nosuid")
            })
            || !read_text("/proc/swaps").is_ok_and(|value| value.lines().count() == 1)
            || !read_text("/proc/sys/fs/protected_hardlinks").is_ok_and(|value| value.trim() == "1")
            || !read_text("/proc/sys/kernel/core_pattern")
                .is_ok_and(|value| !value.trim().starts_with('|'))
        {
            return Err(HostError::Workspace);
        }

        let state = PathBuf::from(STATE);
        let metadata = fs::symlink_metadata(&state).map_err(|_| HostError::State)?;
        if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o777 != 0o700 {
            return Err(HostError::State);
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(OFlags::NOFOLLOW.bits() as i32)
            .open(state.join("box-0.owner"))
            .map_err(|_| HostError::State)?;
        if !lock
            .metadata()
            .is_ok_and(|meta| meta.is_file() && meta.uid() == uid && meta.nlink() == 1)
        {
            return Err(HostError::State);
        }
        lock.try_lock().map_err(|err| match err {
            std::fs::TryLockError::WouldBlock => HostError::Busy,
            std::fs::TryLockError::Error(_) => HostError::State,
        })?;
        let boot_id =
            read_text("/proc/sys/kernel/random/boot_id").map_err(|_| HostError::Platform)?;
        let boot_id = boot_id.trim().to_owned();
        if !valid_boot_id(&boot_id) {
            return Err(HostError::Platform);
        }
        let host = Self {
            state,
            group: group_root.join("box-0"),
            boot_id,
            _lock: lock,
        };
        host.reconcile()?;
        Ok(host)
    }

    fn reconcile(&self) -> Result<(), HostError> {
        let pending = match read_bounded(self.state.join(JOURNAL), JOURNAL_LIMIT) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                if self.is_clear().map_err(HostError::Recovery)? {
                    return self.remove_retry().map_err(HostError::Recovery);
                }
                return Err(HostError::UnownedBox);
            }
            Err(_) => return Err(HostError::State),
        };
        let owner_boot = std::str::from_utf8(&pending)
            .ok()
            .and_then(|value| value.strip_prefix(MARKER))
            .and_then(|value| value.strip_suffix('\n'))
            .filter(|value| valid_boot_id(value))
            .ok_or(HostError::State)?;
        if owner_boot == self.boot_id && !self.retry_allowed(&pending)? {
            return Err(HostError::Recovery(CleanupError::Supervisor));
        }
        self.cleanup().map_err(HostError::Recovery)
    }

    fn retry_allowed(&self, pending: &[u8]) -> Result<bool, HostError> {
        match read_bounded(self.state.join(RETRY), JOURNAL_LIMIT) {
            Ok(bytes) => Ok(bytes == pending),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(_) => Err(HostError::State),
        }
    }

    fn journal_record(&self) -> String {
        format!("{MARKER}{}\n", self.boot_id)
    }

    pub(super) fn begin(&self) -> Result<(), CleanupError> {
        if !self.is_clear()? {
            return Err(CleanupError::Residual);
        }
        self.remove_retry()?;
        self.write_record(JOURNAL, self.journal_record().as_bytes())
    }

    fn write_record(&self, name: &str, contents: &[u8]) -> Result<(), CleanupError> {
        let mut marker = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.state.join(name))
            .map_err(|_| CleanupError::Journal)?;
        marker
            .write_all(contents)
            .and_then(|()| marker.sync_all())
            .and_then(|()| File::open(&self.state)?.sync_all())
            .map_err(|_| CleanupError::Journal)
    }

    fn remove_retry(&self) -> Result<(), CleanupError> {
        match fs::remove_file(self.state.join(RETRY)) {
            Ok(()) => File::open(&self.state)
                .and_then(|state| state.sync_all())
                .map_err(|_| CleanupError::Journal),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(CleanupError::Journal),
        }
    }

    pub(super) fn cleanup(&self) -> Result<(), CleanupError> {
        let record = read_bounded(self.state.join(JOURNAL), JOURNAL_LIMIT)
            .map_err(|_| CleanupError::Journal)?;
        self.remove_retry()?;
        let mut cmd = command(ISOLATE);
        cmd.args(["--cg", "--box-id=0", "--cleanup"]);
        let captured = capture(
            &mut cmd,
            COMMAND_BUDGET,
            CONTROL_LIMIT,
            CONTROL_LIMIT,
            &CancellationToken::new(),
        );

        let result = match captured {
            Err(ProcessError::Unreaped) => return Err(CleanupError::Supervisor),
            Ok(report) if report.status.success() && report.stop.is_none() => self.verify_cleanup(),
            _ => Err(CleanupError::Helper),
        };
        if let Err(error) = result {
            self.write_record(RETRY, &record)?;
            return Err(error);
        }
        fs::remove_file(self.state.join(JOURNAL))
            .and_then(|()| File::open(&self.state)?.sync_all())
            .map_err(|_| CleanupError::Journal)
    }

    fn verify_cleanup(&self) -> Result<(), CleanupError> {
        if !self.is_clear()? {
            return Err(CleanupError::Residual);
        }
        match fs::remove_file(self.metadata_path()) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(CleanupError::Journal),
        }
    }

    pub(super) fn metadata_path(&self) -> PathBuf {
        self.state.join(METADATA)
    }

    fn is_clear(&self) -> Result<bool, CleanupError> {
        let lock_clear = match fs::symlink_metadata(LOCK) {
            Ok(meta) => meta.is_file() && meta.len() == 0 && meta.uid() == 0,
            Err(err) if err.kind() == io::ErrorKind::NotFound => true,
            Err(_) => return Err(CleanupError::Residual),
        };
        Ok(lock_clear && absent(Path::new(WORKSPACE))? && absent(&self.group)?)
    }

    pub(super) fn initialize(&self, quota: String) -> Result<(), ProcessError> {
        let mut cmd = command(ISOLATE);
        cmd.args(["--cg", "--box-id=0", &quota, "--init"]);
        let report = capture(
            &mut cmd,
            COMMAND_BUDGET,
            CONTROL_LIMIT,
            CONTROL_LIMIT,
            &CancellationToken::new(),
        )?;
        if !report.status.success()
            || report.stop.is_some()
            || std::str::from_utf8(&report.stdout).ok().map(str::trim) != Some(WORKSPACE)
            || ![
                "cpu.stat",
                "cgroup.kill",
                "cgroup.events",
                "memory.max",
                "memory.swap.max",
                "memory.peak",
                "memory.events",
            ]
            .iter()
            .all(|name| self.group.join(name).is_file())
            || !self.unpopulated()
        {
            return Err(ProcessError::Io);
        }
        Ok(())
    }

    pub(super) fn unpopulated(&self) -> bool {
        read_text(self.group.join("cgroup.events"))
            .is_ok_and(|value| value.lines().any(|line| line == "populated 0"))
    }
}

fn valid_boot_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn absent(path: &Path) -> Result<bool, CleanupError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(false),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(_) => Err(CleanupError::Residual),
    }
}

fn host_command(program: &str, args: &[&str]) -> Option<Capture> {
    capture(
        command(program).args(args),
        COMMAND_BUDGET,
        CONTROL_LIMIT,
        CONTROL_LIMIT,
        &CancellationToken::new(),
    )
    .ok()
    .filter(|output| output.stop.is_none())
}

fn host_success(program: &str, args: &[&str]) -> bool {
    host_command(program, args).is_some_and(|output| output.status.success())
}

fn host_text(program: &str, args: &[&str]) -> Option<String> {
    let output = host_command(program, args)?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn read_text(path: impl AsRef<Path>) -> io::Result<String> {
    String::from_utf8(read_bounded(path, CONTROL_LIMIT)?).map_err(io::Error::other)
}

pub(super) fn read_bounded(path: impl AsRef<Path>, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::other("control record exceeded its size limit"));
    }
    Ok(bytes)
}
