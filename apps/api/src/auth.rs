use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use ferrum_config::DevelopmentCaller;
use ferrum_domain::{ApiKeyScope, AuthContext};
use ferrum_infrastructure::PostgresSubmissions;
use subtle::ConstantTimeEq;

use crate::submissions::error::ApiError;

#[derive(Clone)]
pub struct AuthState {
    pub repository: PostgresSubmissions,
    pub caller: Arc<DevelopmentCaller>,
}

pub async fn authenticate(
    State(state): State<AuthState>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = single_header(request.headers(), header::AUTHORIZATION.as_str())
        .and_then(|h| h.split_once(' '))
        .and_then(|(scheme, val)| {
            if scheme.eq_ignore_ascii_case("Bearer") {
                Some(val)
            } else {
                None
            }
        });

    let Some(raw_token) = token else {
        ferrum_runtime::MetricsRegistry::global()
            .auth_failures_total
            .inc(&[("reason", "missing_token")]);
        return ApiError::unauthorized().into_response();
    };

    let auth_context = if raw_token.starts_with("ferrum_") {
        match ferrum_infrastructure::authenticate_api_key(state.repository.pool(), raw_token).await
        {
            Ok(Some(ctx)) => ctx,
            _ => {
                ferrum_runtime::MetricsRegistry::global()
                    .auth_failures_total
                    .inc(&[("reason", "invalid_api_key")]);
                return ApiError::unauthorized().into_response();
            }
        }
    } else if bool::from(raw_token.as_bytes().ct_eq(state.caller.token.as_bytes())) {
        AuthContext::full_access(state.caller.organization_id)
    } else {
        ferrum_runtime::MetricsRegistry::global()
            .auth_failures_total
            .inc(&[("reason", "invalid_dev_token")]);
        return ApiError::unauthorized().into_response();
    };

    request
        .extensions_mut()
        .insert(auth_context.organization_id);
    request.extensions_mut().insert(auth_context);

    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

pub fn require_scope(auth: &AuthContext, scope: ApiKeyScope) -> Result<(), ApiError> {
    if auth.has_scope(scope) {
        Ok(())
    } else {
        Err(ApiError::forbidden(
            "insufficient_scope",
            format!("Scope '{scope}' is required for this operation"),
        ))
    }
}

pub fn single_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let all = headers.get_all(name);
    let mut values = all.iter();
    let value = values.next()?.to_str().ok()?;
    values.next().is_none().then_some(value)
}
