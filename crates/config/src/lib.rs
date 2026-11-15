mod database;

pub use database::{DatabaseSettings, DevelopmentCaller};

use std::{
    env, fs, io,
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use database::DatabaseFile;
use serde::{Deserialize, de::DeserializeOwned};
use thiserror::Error;

const DEFAULT_SHUTDOWN_MS: u64 = 10_000;
const MAX_SHUTDOWN_MS: u64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Development,
    Production,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Production => "production",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug)]
pub struct ProcessSettings {
    pub mode: Mode,
    pub log_level: LogLevel,
    pub shutdown_timeout: Duration,
}

pub struct ApiSettings {
    pub listen_addr: SocketAddr,
    pub process: ProcessSettings,
    pub submissions: Option<SubmissionSettings>,
}

pub struct SubmissionSettings {
    pub database: DatabaseSettings,
    pub caller: DevelopmentCaller,
}

#[derive(Debug)]
pub struct WorkerSettings {
    pub process: ProcessSettings,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("{0} must contain valid Unicode")]
    InvalidEnvironment(&'static str),
    #[error("could not read the settings file selected by {variable}: {kind}")]
    ReadFile {
        variable: &'static str,
        kind: io::ErrorKind,
    },
    #[error("invalid TOML or unknown setting in the file selected by {0}")]
    InvalidFile(&'static str),
    #[error("invalid {setting}: {expected}")]
    InvalidSetting {
        setting: &'static str,
        expected: &'static str,
    },
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ApiFile {
    listen_addr: Option<String>,
    process: ProcessFile,
    database: DatabaseFile,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct WorkerFile {
    process: ProcessFile,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ProcessFile {
    mode: Option<String>,
    log_level: Option<String>,
    shutdown_timeout_ms: Option<u64>,
}

impl ApiSettings {
    pub fn load() -> Result<Self, ConfigError> {
        let file: ApiFile = read_file("FERRUM_API_CONFIG")?;
        let process = resolve_process(file.process, "FERRUM_API_SHUTDOWN_TIMEOUT_MS")?;
        let listen_addr = match read_env("FERRUM_API_LISTEN_ADDR")?.or(file.listen_addr) {
            Some(value) => value.parse().map_err(|_| ConfigError::InvalidSetting {
                setting: "listen_addr (FERRUM_API_LISTEN_ADDR)",
                expected: "use an IP address and a TCP port, for example 127.0.0.1:8080",
            })?,
            None => SocketAddr::from((Ipv4Addr::LOCALHOST, 8080)),
        };

        if process.mode == Mode::Production && listen_addr.port() == 0 {
            return Err(ConfigError::InvalidSetting {
                setting: "listen_addr (FERRUM_API_LISTEN_ADDR)",
                expected: "use a nonzero port in production mode",
            });
        }

        let submissions = match (
            DatabaseSettings::from_file(file.database)?,
            DevelopmentCaller::load()?,
        ) {
            (None, None) => None,
            (Some(database), Some(caller))
                if process.mode == Mode::Development && listen_addr.ip().is_loopback() =>
            {
                Some(SubmissionSettings { database, caller })
            }
            (Some(_), Some(_)) => {
                return Err(ConfigError::InvalidSetting {
                    setting: "submission intake",
                    expected: "use development mode and a loopback HTTP listener",
                });
            }
            _ => {
                return Err(ConfigError::InvalidSetting {
                    setting: "submission intake",
                    expected: "set FERRUM_DATABASE_URL, FERRUM_DEV_TENANT_ID and FERRUM_DEV_TOKEN together",
                });
            }
        };
        Ok(Self {
            listen_addr,
            process,
            submissions,
        })
    }
}

impl WorkerSettings {
    pub fn load() -> Result<Self, ConfigError> {
        let file: WorkerFile = read_file("FERRUM_WORKER_CONFIG")?;
        Ok(Self {
            process: resolve_process(file.process, "FERRUM_WORKER_SHUTDOWN_TIMEOUT_MS")?,
        })
    }
}

fn resolve_process(
    file: ProcessFile,
    timeout_variable: &'static str,
) -> Result<ProcessSettings, ConfigError> {
    let mode = match read_env("FERRUM_MODE")?.or(file.mode).as_deref() {
        None | Some("development") => Mode::Development,
        Some("production") => Mode::Production,
        _ => {
            return Err(ConfigError::InvalidSetting {
                setting: "process.mode (FERRUM_MODE)",
                expected: "use development or production",
            });
        }
    };
    let log_level = match read_env("FERRUM_LOG_LEVEL")?.or(file.log_level).as_deref() {
        Some("error") => LogLevel::Error,
        Some("warn") => LogLevel::Warn,
        None | Some("info") => LogLevel::Info,
        Some("debug") => LogLevel::Debug,
        Some("trace") => LogLevel::Trace,
        _ => {
            return Err(ConfigError::InvalidSetting {
                setting: "process.log_level (FERRUM_LOG_LEVEL)",
                expected: "use error, warn, info, debug or trace",
            });
        }
    };
    let invalid_timeout = || ConfigError::InvalidSetting {
        setting: timeout_variable,
        expected: "use process.shutdown_timeout_ms or an environment override from 1 to 60000 milliseconds",
    };
    let timeout_ms = match read_env(timeout_variable)? {
        Some(value) => value.parse().map_err(|_| invalid_timeout())?,
        None => file.shutdown_timeout_ms.unwrap_or(DEFAULT_SHUTDOWN_MS),
    };
    if !(1..=MAX_SHUTDOWN_MS).contains(&timeout_ms) {
        return Err(invalid_timeout());
    }
    Ok(ProcessSettings {
        mode,
        log_level,
        shutdown_timeout: Duration::from_millis(timeout_ms),
    })
}

fn read_file<T: Default + DeserializeOwned>(variable: &'static str) -> Result<T, ConfigError> {
    let Some(path) = read_env(variable)? else {
        return Ok(T::default());
    };
    let contents = fs::read_to_string(path).map_err(|error| ConfigError::ReadFile {
        variable,
        kind: error.kind(),
    })?;
    toml::from_str(&contents).map_err(|_| ConfigError::InvalidFile(variable))
}

fn read_env(variable: &'static str) -> Result<Option<String>, ConfigError> {
    match env::var(variable) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(ConfigError::InvalidEnvironment(variable)),
    }
}
