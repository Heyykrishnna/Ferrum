use std::net::IpAddr;

use ferrum_domain::{WebhookDelivery, WebhookEndpoint};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::{DatabaseError, log_sql_failure};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WebhookError {
    #[error("invalid webhook URL")]
    InvalidUrl,
    #[error("unsupported scheme, must be http or https")]
    InvalidScheme,
    #[error("destination blocked by SSRF policy")]
    BlockedDestination,
    #[error("invalid webhook secret")]
    InvalidSecret,
    #[error("database error: {0}")]
    Database(String),
}

impl From<DatabaseError> for WebhookError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error.to_string())
    }
}

pub fn generate_webhook_secret() -> String {
    format!(
        "whsec_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

fn compute_hmac_sha256(key_bytes: &[u8], message: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 64];
    if key_bytes.len() > 64 {
        let hash = Sha256::digest(key_bytes);
        key[..32].copy_from_slice(&hash);
    } else {
        key[..key_bytes.len()].copy_from_slice(key_bytes);
    }
    let mut ipad = [0u8; 64];
    let mut opad = [0u8; 64];
    for i in 0..64 {
        ipad[i] = key[i] ^ 0x36;
        opad[i] = key[i] ^ 0x5c;
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    outer.finalize().into()
}

pub fn sign_webhook_payload(
    secret: &str,
    timestamp: i64,
    payload: &[u8],
) -> Result<String, WebhookError> {
    if secret.is_empty() {
        return Err(WebhookError::InvalidSecret);
    }
    let mut message = Vec::with_capacity(32 + payload.len());
    message.extend_from_slice(timestamp.to_string().as_bytes());
    message.push(b'.');
    message.extend_from_slice(payload);
    let mac = compute_hmac_sha256(secret.as_bytes(), &message);
    let signature = hex::encode(mac);
    Ok(format!("t={timestamp},v1={signature}"))
}

pub fn validate_webhook_url(raw: &str) -> Result<(), WebhookError> {
    let parsed = url::Url::parse(raw).map_err(|_| WebhookError::InvalidUrl)?;
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err(WebhookError::InvalidScheme);
    }
    let host = parsed.host_str().ok_or(WebhookError::InvalidUrl)?;
    if host.eq_ignore_ascii_case("localhost")
        || host.eq_ignore_ascii_case("metadata.google.internal")
        || host.ends_with(".internal")
        || host.ends_with(".local")
    {
        return Err(WebhookError::BlockedDestination);
    }
    if host
        .parse::<IpAddr>()
        .is_ok_and(is_private_or_restricted_ip)
    {
        return Err(WebhookError::BlockedDestination);
    }
    Ok(())
}

fn is_private_or_restricted_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_loopback()
                || ipv4.is_private()
                || ipv4.is_link_local()
                || ipv4.is_broadcast()
                || ipv4.is_multicast()
                || ipv4.is_unspecified()
        }
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback()
                || ipv6.is_multicast()
                || ipv6.is_unspecified()
                || (ipv6.segments()[0] & 0xfe00) == 0xfc00
                || (ipv6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

#[derive(sqlx::FromRow)]
struct StoredEndpoint {
    id: Uuid,
    organization_id: Uuid,
    url: String,
    secret: String,
    events: Vec<String>,
    active: bool,
    created_at: i64,
}

impl StoredEndpoint {
    fn into_domain(self) -> WebhookEndpoint {
        WebhookEndpoint {
            id: self.id,
            organization_id: self.organization_id,
            url: self.url,
            secret: self.secret,
            events: self.events,
            active: self.active,
            created_at: self.created_at,
        }
    }
}

pub async fn create_webhook_endpoint(
    pool: &PgPool,
    organization_id: Uuid,
    url: &str,
    events: &[String],
) -> Result<WebhookEndpoint, WebhookError> {
    validate_webhook_url(url)?;
    let id = Uuid::new_v4();
    let secret = generate_webhook_secret();
    let events_slice: Vec<&str> = events.iter().map(String::as_str).collect();

    let created: StoredEndpoint = sqlx::query_as(
        "INSERT INTO webhook_endpoints (id, organization_id, url, secret, events)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING
            id, organization_id, url, secret, events, active,
            extract(epoch from created_at)::bigint AS created_at",
    )
    .bind(id)
    .bind(organization_id)
    .bind(url)
    .bind(&secret)
    .bind(&events_slice)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        log_sql_failure("create_webhook_endpoint", &error);
        DatabaseError::Initialize
    })?;

    Ok(created.into_domain())
}

pub async fn list_webhook_endpoints(
    pool: &PgPool,
    organization_id: Uuid,
) -> Result<Vec<WebhookEndpoint>, DatabaseError> {
    let rows: Vec<StoredEndpoint> = sqlx::query_as(
        "SELECT
            id, organization_id, url, secret, events, active,
            extract(epoch from created_at)::bigint AS created_at
         FROM webhook_endpoints
         WHERE organization_id = $1
         ORDER BY created_at DESC",
    )
    .bind(organization_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        log_sql_failure("list_webhook_endpoints", &error);
        DatabaseError::Connect
    })?;

    Ok(rows.into_iter().map(StoredEndpoint::into_domain).collect())
}

pub async fn delete_webhook_endpoint(
    pool: &PgPool,
    organization_id: Uuid,
    endpoint_id: Uuid,
) -> Result<bool, DatabaseError> {
    let result =
        sqlx::query("DELETE FROM webhook_endpoints WHERE organization_id = $1 AND id = $2")
            .bind(organization_id)
            .bind(endpoint_id)
            .execute(pool)
            .await
            .map_err(|error| {
                log_sql_failure("delete_webhook_endpoint", &error);
                DatabaseError::Connect
            })?;

    Ok(result.rows_affected() == 1)
}

pub async fn record_webhook_delivery(
    pool: &PgPool,
    organization_id: Uuid,
    endpoint_id: Uuid,
    event_id: Uuid,
    event_type: &str,
    status_code: Option<u16>,
    success: bool,
) -> Result<WebhookDelivery, DatabaseError> {
    let id = Uuid::new_v4();
    let code = status_code.map(|c| c as i16);

    let row = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO webhook_deliveries
            (id, organization_id, endpoint_id, event_id, event_type, status_code, success)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING extract(epoch from delivered_at)::bigint AS delivered_at",
    )
    .bind(id)
    .bind(organization_id)
    .bind(endpoint_id)
    .bind(event_id)
    .bind(event_type)
    .bind(code)
    .bind(success)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        log_sql_failure("record_webhook_delivery", &error);
        DatabaseError::Connect
    })?;

    Ok(WebhookDelivery {
        id,
        endpoint_id,
        event_id,
        event_type: event_type.to_string(),
        status_code,
        success,
        delivered_at: row.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssrf_validation_blocks_private_and_local_addresses() {
        assert_eq!(
            validate_webhook_url("http://127.0.0.1:8080/hook"),
            Err(WebhookError::BlockedDestination)
        );
        assert_eq!(
            validate_webhook_url("http://localhost:3000/hook"),
            Err(WebhookError::BlockedDestination)
        );
        assert_eq!(
            validate_webhook_url("https://10.0.0.1/hook"),
            Err(WebhookError::BlockedDestination)
        );
        assert_eq!(
            validate_webhook_url("https://192.168.1.1/hook"),
            Err(WebhookError::BlockedDestination)
        );
        assert_eq!(
            validate_webhook_url("http://169.254.169.254/latest/meta-data"),
            Err(WebhookError::BlockedDestination)
        );
        assert_eq!(
            validate_webhook_url("https://metadata.google.internal/computeMetadata"),
            Err(WebhookError::BlockedDestination)
        );
        assert_eq!(
            validate_webhook_url("ftp://example.com/hook"),
            Err(WebhookError::InvalidScheme)
        );
        assert_eq!(
            validate_webhook_url("not a url"),
            Err(WebhookError::InvalidUrl)
        );
    }

    #[test]
    fn ssrf_validation_permits_public_destinations() {
        assert!(validate_webhook_url("https://example.com/webhook").is_ok());
        assert!(validate_webhook_url("https://hooks.slack.com/services/123/456").is_ok());
    }

    #[test]
    fn webhook_payload_signing() {
        let secret = "whsec_test_secret_for_signatures_123456";
        let timestamp = 1700000000;
        let payload = b"{\"event\":\"submission.completed\"}";
        let signature = sign_webhook_payload(secret, timestamp, payload).unwrap();
        assert!(signature.starts_with("t=1700000000,v1="));
        let expected = sign_webhook_payload(secret, timestamp, payload).unwrap();
        assert_eq!(signature, expected);
    }
}
