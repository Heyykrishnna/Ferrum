use std::time::Duration;

use serde::Deserialize;
use uuid::Uuid;

use super::{ApiFile, ConfigError, read_env, read_file};

pub struct DatabaseSettings {
    pub url: String,
    pub max_connections: u32,
    pub acquire_timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct DevelopmentCaller {
    pub organization_id: Uuid,
    pub token: String,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct DatabaseFile {
    max_connections: Option<u32>,
    acquire_timeout_ms: Option<u64>,
}

impl DatabaseSettings {
    pub fn load() -> Result<Self, ConfigError> {
        let file: ApiFile = read_file("FERRUM_API_CONFIG")?;
        Self::from_file(file.database)?.ok_or(ConfigError::InvalidSetting {
            setting: "FERRUM_DATABASE_URL",
            expected: "provide a PostgreSQL connection URL",
        })
    }

    pub(super) fn from_file(file: DatabaseFile) -> Result<Option<Self>, ConfigError> {
        let Some(url) = read_env("FERRUM_DATABASE_URL")? else {
            return Ok(None);
        };
        let invalid_url = || ConfigError::InvalidSetting {
            setting: "FERRUM_DATABASE_URL",
            expected: "provide a PostgreSQL connection URL",
        };
        if !(url.starts_with("postgres://") || url.starts_with("postgresql://")) {
            return Err(invalid_url());
        }
        let max_connections = bounded_pool_setting(
            file.max_connections.map(u64::from),
            "FERRUM_DB_MAX_CONNECTIONS",
            10,
            100,
            "use a connection count from 1 to 100",
        )?;
        let acquire_timeout_ms = bounded_pool_setting(
            file.acquire_timeout_ms,
            "FERRUM_DB_ACQUIRE_TIMEOUT_MS",
            5_000,
            30_000,
            "use an acquire timeout from 1 to 30000 milliseconds",
        )?;
        Ok(Some(Self {
            url,
            max_connections: u32::try_from(max_connections).map_err(|_| {
                ConfigError::InvalidSetting {
                    setting: "FERRUM_DB_MAX_CONNECTIONS",
                    expected: "use a connection count from 1 to 100",
                }
            })?,
            acquire_timeout: Duration::from_millis(acquire_timeout_ms),
        }))
    }
}

impl DevelopmentCaller {
    pub(super) fn load() -> Result<Option<Self>, ConfigError> {
        let organization_id = read_env("FERRUM_DEV_TENANT_ID")?;
        let token = read_env("FERRUM_DEV_TOKEN")?;
        if organization_id.is_none() && token.is_none() {
            return Ok(None);
        }
        let organization_id = organization_id
            .and_then(|value| value.parse::<Uuid>().ok())
            .filter(|id| !id.is_nil())
            .ok_or(ConfigError::InvalidSetting {
                setting: "FERRUM_DEV_TENANT_ID",
                expected: "provide a non-nil tenant UUID",
            })?;
        let token = token
            .filter(|value| value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or(ConfigError::InvalidSetting {
                setting: "FERRUM_DEV_TOKEN",
                expected: "provide a 64-character hexadecimal bearer token",
            })?;
        Ok(Some(Self {
            organization_id,
            token,
        }))
    }
}

fn bounded_pool_setting(
    file: Option<u64>,
    variable: &'static str,
    default: u64,
    max: u64,
    expected: &'static str,
) -> Result<u64, ConfigError> {
    let invalid = || ConfigError::InvalidSetting {
        setting: variable,
        expected,
    };
    let value = match read_env(variable)? {
        Some(value) => value.parse().map_err(|_| invalid())?,
        None => file.unwrap_or(default),
    };
    if (1..=max).contains(&value) {
        Ok(value)
    } else {
        Err(invalid())
    }
}
