use crate::{ExecutionFacts, ProgramOutcome, SandboxError};

#[derive(Default)]
pub(crate) struct Metadata<'a> {
    pub(crate) facts: ExecutionFacts,
    status: Option<&'a str>,
    message: Option<&'a str>,
    oom_killed: bool,
}

impl<'a> Metadata<'a> {
    pub(crate) fn parse(bytes: &'a [u8]) -> Result<Self, SandboxError> {
        let mut metadata = Self::default();
        let text = std::str::from_utf8(bytes).map_err(|_| SandboxError::Metadata)?;
        for line in text.lines() {
            let (key, value) = line.split_once(':').ok_or(SandboxError::Metadata)?;
            match key {
                "status" => set(&mut metadata.status, value)?,
                "message" => set(&mut metadata.message, value)?,
                "exitcode" => set(&mut metadata.facts.exit_code, number(value)?)?,
                "exitsig" => {
                    let signal = number::<u8>(value)?;
                    if !(1..=64).contains(&signal) {
                        return Err(SandboxError::Metadata);
                    }
                    set(&mut metadata.facts.signal, signal)?;
                }
                "time" => set(&mut metadata.facts.cpu_time_ms, milliseconds(value)?)?,
                "time-wall" => set(&mut metadata.facts.wall_time_ms, milliseconds(value)?)?,
                "cg-mem" => set(&mut metadata.facts.peak_memory_kib, number(value)?)?,
                "cg-oom-killed" if value == "1" && !metadata.oom_killed => {
                    metadata.oom_killed = true
                }
                "cg-oom-killed" => return Err(SandboxError::Metadata),
                _ => {}
            }
        }
        Ok(metadata)
    }

    pub(crate) fn outcome(&self, helper_exit: Option<i32>) -> Result<ProgramOutcome, SandboxError> {
        use ProgramOutcome::{CpuTimeLimit, MemoryLimit, RuntimeError, Success, WallTimeLimit};

        if self.facts.cpu_time_ms.is_none() || self.facts.wall_time_ms.is_none() {
            return Err(SandboxError::Metadata);
        }
        let outcome = match (helper_exit, self.status, self.message) {
            (Some(0), None, None) if self.facts.exit_code == Some(0) && !self.oom_killed => Success,
            (Some(1), Some("SG" | "RE" | "TO"), _) if self.oom_killed => MemoryLimit,
            (Some(1), Some("TO"), Some("Time limit exceeded (wall clock)")) => WallTimeLimit,
            (Some(1), Some("TO"), Some("Time limit exceeded")) => CpuTimeLimit,
            (Some(1), Some("RE"), _) if self.facts.exit_code.is_some_and(|code| code > 0) => {
                RuntimeError
            }
            (Some(1), Some("SG"), _) if self.facts.signal.is_some() => RuntimeError,
            _ => return Err(SandboxError::Metadata),
        };
        Ok(outcome)
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    pub(crate) fn supervisor_failed(&self) -> bool {
        self.status == Some("XX")
    }
}

fn set<T>(field: &mut Option<T>, value: T) -> Result<(), SandboxError> {
    if field.replace(value).is_some() {
        return Err(SandboxError::Metadata);
    }
    Ok(())
}

fn number<T: std::str::FromStr>(value: &str) -> Result<T, SandboxError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(SandboxError::Metadata);
    }
    value.parse().map_err(|_| SandboxError::Metadata)
}

fn milliseconds(value: &str) -> Result<u64, SandboxError> {
    let (seconds, fraction) = value.split_once('.').ok_or(SandboxError::Metadata)?;
    if fraction.len() != 3 {
        return Err(SandboxError::Metadata);
    }
    number::<u64>(seconds)?
        .checked_mul(1_000)
        .and_then(|base| base.checked_add(number::<u64>(fraction).ok()?))
        .ok_or(SandboxError::Metadata)
}
