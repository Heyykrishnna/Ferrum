use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Extension, Path, State, rejection::JsonRejection},
    http::StatusCode,
    middleware,
    routing::{delete, get, post},
};
use ferrum_config::DevelopmentCaller;
use ferrum_domain::{ApiKeyScope, AuthContext, WebhookEndpoint};
use ferrum_infrastructure::{PostgresSubmissions, WebhookError};
use serde::Deserialize;

use crate::auth::{AuthState, authenticate, require_scope};
use crate::submissions::error::ApiError;

const MAX_BODY_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateWebhookRequest {
    pub url: String,
    #[serde(default)]
    pub events: Vec<String>,
}

pub fn router(repository: PostgresSubmissions, caller: Arc<DevelopmentCaller>) -> Router {
    let auth_state = AuthState {
        repository: repository.clone(),
        caller,
    };

    Router::new()
        .route("/v1/webhooks", post(create))
        .route("/v1/webhooks", get(list))
        .route("/v1/webhooks/{id}", delete(delete_endpoint))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .route_layer(middleware::from_fn_with_state(auth_state, authenticate))
        .with_state(repository)
}

async fn create(
    State(repository): State<PostgresSubmissions>,
    Extension(auth): Extension<AuthContext>,
    request: Result<Json<CreateWebhookRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<WebhookEndpoint>), ApiError> {
    require_scope(&auth, ApiKeyScope::WebhooksManage)?;
    let Json(request) = request.map_err(ApiError::from_json)?;
    if request.url.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_url",
            "Webhook URL cannot be empty",
        ));
    }

    let endpoint = ferrum_infrastructure::create_webhook_endpoint(
        repository.pool(),
        auth.organization_id,
        &request.url,
        &request.events,
    )
    .await
    .map_err(|err| match err {
        WebhookError::BlockedDestination => ApiError::bad_request(
            "blocked_destination",
            "Destination is blocked by SSRF policy",
        ),
        WebhookError::InvalidScheme => {
            ApiError::bad_request("invalid_scheme", "Webhook URL scheme must be http or https")
        }
        WebhookError::InvalidUrl => ApiError::bad_request("invalid_url", "Malformed webhook URL"),
        _ => ApiError::internal("storage_error", "Failed to create webhook endpoint"),
    })?;

    Ok((StatusCode::CREATED, Json(endpoint)))
}

async fn list(
    State(repository): State<PostgresSubmissions>,
    Extension(auth): Extension<AuthContext>,
) -> Result<Json<Vec<WebhookEndpoint>>, ApiError> {
    require_scope(&auth, ApiKeyScope::WebhooksManage)?;
    let endpoints =
        ferrum_infrastructure::list_webhook_endpoints(repository.pool(), auth.organization_id)
            .await
            .map_err(|_| ApiError::internal("storage_error", "Failed to list webhooks"))?;
    Ok(Json(endpoints))
}

async fn delete_endpoint(
    State(repository): State<PostgresSubmissions>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    require_scope(&auth, ApiKeyScope::WebhooksManage)?;
    let id = id
        .parse()
        .map_err(|_| ApiError::not_found_with("webhook_not_found", "Webhook endpoint not found"))?;
    let deleted =
        ferrum_infrastructure::delete_webhook_endpoint(repository.pool(), auth.organization_id, id)
            .await
            .map_err(|_| ApiError::internal("storage_error", "Failed to delete webhook"))?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found_with(
            "webhook_not_found",
            "Webhook endpoint not found",
        ))
    }
}
