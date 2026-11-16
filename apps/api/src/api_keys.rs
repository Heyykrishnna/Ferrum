use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Extension, Path, State, rejection::JsonRejection},
    http::StatusCode,
    middleware,
    routing::{delete, get, post},
};
use ferrum_config::DevelopmentCaller;
use ferrum_domain::{ApiKey, ApiKeyScope, AuthContext};
use ferrum_infrastructure::PostgresSubmissions;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{AuthState, authenticate, require_scope};
use crate::submissions::error::ApiError;

const MAX_BODY_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub scopes: Vec<ApiKeyScope>,
    #[serde(default)]
    pub expires_in_seconds: Option<i64>,
}

#[derive(Serialize)]
pub struct CreateApiKeyResponse {
    pub id: Uuid,
    pub name: String,
    pub key_prefix: String,
    pub raw_key: String,
    pub scopes: Vec<ApiKeyScope>,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

pub fn router(repository: PostgresSubmissions, caller: Arc<DevelopmentCaller>) -> Router {
    let auth_state = AuthState {
        repository: repository.clone(),
        caller,
    };

    Router::new()
        .route("/v1/api-keys", post(create))
        .route("/v1/api-keys", get(list))
        .route("/v1/api-keys/{id}", delete(revoke))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .route_layer(middleware::from_fn_with_state(auth_state, authenticate))
        .with_state(repository)
}

async fn create(
    State(repository): State<PostgresSubmissions>,
    Extension(auth): Extension<AuthContext>,
    request: Result<Json<CreateApiKeyRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), ApiError> {
    require_scope(&auth, ApiKeyScope::Admin)?;
    let Json(request) = request.map_err(ApiError::from_json)?;
    if request.name.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_name",
            "API key name cannot be empty",
        ));
    }
    if request.scopes.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_scopes",
            "At least one scope is required",
        ));
    }

    let (api_key, raw_key) = ferrum_infrastructure::create_api_key(
        repository.pool(),
        auth.organization_id,
        &request.name,
        &request.scopes,
        request.expires_in_seconds,
    )
    .await
    .map_err(|_| ApiError::internal("storage_error", "Failed to create API key"))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateApiKeyResponse {
            id: api_key.id,
            name: api_key.name,
            key_prefix: api_key.key_prefix,
            raw_key,
            scopes: api_key.scopes,
            created_at: api_key.created_at,
            expires_at: api_key.expires_at,
        }),
    ))
}

async fn list(
    State(repository): State<PostgresSubmissions>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<ApiKey>>, ApiError> {
    require_scope(&auth, ApiKeyScope::Admin)?;
    let keys = ferrum_infrastructure::list_api_keys(repository.pool(), auth.organization_id)
        .await
        .map_err(|_| ApiError::internal("storage_error", "Failed to list API keys"))?;
    Ok(Json(keys))
}

async fn revoke(
    State(repository): State<PostgresSubmissions>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_scope(&auth, ApiKeyScope::Admin)?;
    let id = id
        .parse()
        .map_err(|_| ApiError::not_found_with("api_key_not_found", "API key not found"))?;
    let revoked =
        ferrum_infrastructure::revoke_api_key(repository.pool(), auth.organization_id, id)
            .await
            .map_err(|_| ApiError::internal("storage_error", "Failed to revoke API key"))?;

    if revoked {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found_with(
            "api_key_not_found",
            "API key not found",
        ))
    }
}
