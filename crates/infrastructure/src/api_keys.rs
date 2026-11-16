use ferrum_domain::{ApiKey, ApiKeyScope, AuthContext};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::{DatabaseError, log_sql_failure};

pub fn generate_raw_key() -> (String, String) {
    let raw = format!(
        "ferrum_live_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    let prefix = raw[..16].to_string();
    (raw, prefix)
}

pub fn hash_key(raw_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw_key.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(sqlx::FromRow)]
struct StoredApiKey {
    id: Uuid,
    organization_id: Uuid,
    name: String,
    key_prefix: String,
    scopes: Vec<String>,
    created_at: i64,
    expires_at: Option<i64>,
    revoked_at: Option<i64>,
}

impl StoredApiKey {
    fn into_domain(self) -> ApiKey {
        let scopes = self
            .scopes
            .into_iter()
            .filter_map(|s| ApiKeyScope::parse(&s))
            .collect();
        ApiKey {
            id: self.id,
            organization_id: self.organization_id,
            name: self.name,
            key_prefix: self.key_prefix,
            scopes,
            created_at: self.created_at,
            expires_at: self.expires_at,
            revoked_at: self.revoked_at,
        }
    }
}

pub async fn create_api_key(
    pool: &PgPool,
    organization_id: Uuid,
    name: &str,
    scopes: &[ApiKeyScope],
    expires_at: Option<i64>,
) -> Result<(ApiKey, String), DatabaseError> {
    let id = Uuid::new_v4();
    let (raw_key, prefix) = generate_raw_key();
    let hash = hash_key(&raw_key);
    let scope_strings: Vec<&str> = scopes.iter().map(ApiKeyScope::as_str).collect();

    let created: StoredApiKey = sqlx::query_as(
        "INSERT INTO api_keys
            (id, organization_id, name, key_prefix, key_hash, scopes, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, CASE WHEN $7::bigint IS NOT NULL THEN to_timestamp($7::bigint) ELSE NULL END)
         RETURNING
            id, organization_id, name, key_prefix, scopes,
            extract(epoch from created_at)::bigint AS created_at,
            extract(epoch from expires_at)::bigint AS expires_at,
            extract(epoch from revoked_at)::bigint AS revoked_at",
    )
    .bind(id)
    .bind(organization_id)
    .bind(name)
    .bind(&prefix)
    .bind(&hash)
    .bind(&scope_strings)
    .bind(expires_at)
    .fetch_one(pool)
    .await
    .map_err(|error| {
        log_sql_failure("create_api_key", &error);
        DatabaseError::Initialize
    })?;

    Ok((created.into_domain(), raw_key))
}

pub async fn authenticate_api_key(
    pool: &PgPool,
    raw_key: &str,
) -> Result<Option<AuthContext>, DatabaseError> {
    if !raw_key.starts_with("ferrum_live_") {
        return Ok(None);
    }
    let hash = hash_key(raw_key);

    let row = sqlx::query_as::<_, (Uuid, Vec<String>, String)>(
        "SELECT organization_id, scopes, key_hash
         FROM api_keys
         WHERE key_hash = $1
           AND revoked_at IS NULL
           AND (expires_at IS NULL OR expires_at > now())",
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await
    .map_err(|error| {
        log_sql_failure("authenticate_api_key", &error);
        DatabaseError::Connect
    })?;

    let Some((organization_id, scope_strings, stored_hash)) = row else {
        return Ok(None);
    };

    if !bool::from(hash.as_bytes().ct_eq(stored_hash.as_bytes())) {
        return Ok(None);
    }

    let scopes = scope_strings
        .into_iter()
        .filter_map(|s| ApiKeyScope::parse(&s))
        .collect();

    Ok(Some(AuthContext::new(organization_id, scopes)))
}

pub async fn list_api_keys(
    pool: &PgPool,
    organization_id: Uuid,
) -> Result<Vec<ApiKey>, DatabaseError> {
    let rows: Vec<StoredApiKey> = sqlx::query_as(
        "SELECT
            id, organization_id, name, key_prefix, scopes,
            extract(epoch from created_at)::bigint AS created_at,
            extract(epoch from expires_at)::bigint AS expires_at,
            extract(epoch from revoked_at)::bigint AS revoked_at
         FROM api_keys
         WHERE organization_id = $1
         ORDER BY created_at DESC",
    )
    .bind(organization_id)
    .fetch_all(pool)
    .await
    .map_err(|error| {
        log_sql_failure("list_api_keys", &error);
        DatabaseError::Connect
    })?;

    Ok(rows.into_iter().map(StoredApiKey::into_domain).collect())
}

pub async fn revoke_api_key(
    pool: &PgPool,
    organization_id: Uuid,
    key_id: Uuid,
) -> Result<bool, DatabaseError> {
    let result = sqlx::query(
        "UPDATE api_keys
         SET revoked_at = now()
         WHERE organization_id = $1 AND id = $2 AND revoked_at IS NULL",
    )
    .bind(organization_id)
    .bind(key_id)
    .execute(pool)
    .await
    .map_err(|error| {
        log_sql_failure("revoke_api_key", &error);
        DatabaseError::Connect
    })?;

    Ok(result.rows_affected() == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_key_format_and_prefix() {
        let (raw, prefix) = generate_raw_key();
        assert!(raw.starts_with("ferrum_live_"));
        assert_eq!(prefix.len(), 16);
        assert_eq!(&raw[..16], prefix);
        assert_eq!(raw.len(), 76);
    }

    #[test]
    fn hash_key_is_deterministic_and_constant_time_matches() {
        let key = "ferrum_live_0123456789abcdef0123456789abcdef";
        let h1 = hash_key(key);
        let h2 = hash_key(key);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
        assert!(bool::from(h1.as_bytes().ct_eq(h2.as_bytes())));
    }
}
