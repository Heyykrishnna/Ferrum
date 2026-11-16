mod error;

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Extension, Path, Request, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use ferrum_application::{IdempotencyKey, SubmissionRepository};
use ferrum_config::DevelopmentCaller;
use ferrum_domain::{
    LANGUAGES, ProcessingState, RequestedLimits, Submission, SubmissionInput, SubmissionResult,
    Verdict,
};
use ferrum_infrastructure::PostgresSubmissions;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use self::error::ApiError;

const MAX_BODY_BYTES: usize = 800 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateSubmission {
    language: String,
    source: String,
    stdin: Option<String>,
    #[serde(default)]
    limits: RequestedLimits,
}

#[derive(Serialize)]
struct SubmissionResponse {
    id: Uuid,
    state: ProcessingState,
    verdict: Option<Verdict>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<SubmissionResult>,
}

impl From<Submission> for SubmissionResponse {
    fn from(submission: Submission) -> Self {
        Self {
            id: submission.id,
            state: submission.state,
            verdict: submission.verdict,
            result: submission.result,
        }
    }
}

#[derive(Serialize)]
struct LanguageResponse {
    id: u16,
    language: &'static str,
    profile: &'static str,
    compiled: bool,
    runtime_limits: &'static ferrum_domain::EffectiveLimits,
}

async fn languages() -> Json<Vec<LanguageResponse>> {
    Json(
        LANGUAGES
            .iter()
            .map(|profile| LanguageResponse {
                id: profile.id,
                language: profile.language,
                profile: profile.revision,
                compiled: profile.compile.is_some(),
                runtime_limits: &profile.run_limits,
            })
            .collect(),
    )
}

pub fn router(repository: PostgresSubmissions, caller: DevelopmentCaller) -> Router {
    Router::new()
        .route("/v1/languages", get(languages))
        .route("/v1/submissions", post(create))
        .route("/v1/submissions/{id}", get(fetch))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .route_layer(middleware::from_fn_with_state(
            Arc::new(caller),
            authenticate,
        ))
        .with_state(repository)
}

async fn authenticate(
    State(caller): State<Arc<DevelopmentCaller>>,
    mut request: Request,
    next: Next,
) -> Response {
    let authenticated = single_header(request.headers(), header::AUTHORIZATION.as_str())
        .and_then(|header| header.split_once(' '))
        .is_some_and(|(scheme, value)| {
            scheme.eq_ignore_ascii_case("Bearer")
                && bool::from(value.as_bytes().ct_eq(caller.token.as_bytes()))
        });
    if !authenticated {
        return ApiError::unauthorized().into_response();
    }

    request.extensions_mut().insert(caller.organization_id);
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn create(
    State(repository): State<PostgresSubmissions>,
    Extension(organization_id): Extension<Uuid>,
    headers: HeaderMap,
    request: Result<Json<CreateSubmission>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let key = single_header(&headers, "idempotency-key").ok_or_else(ApiError::idempotency_key)?;
    let key = IdempotencyKey::parse(key)?;
    let Json(request) = request.map_err(ApiError::from_json)?;
    let input = SubmissionInput::new(
        &request.language,
        request.source,
        request.stdin,
        request.limits,
    )?;
    let submission = repository.create(organization_id, key, input).await?;
    let location = format!("/v1/submissions/{}", submission.id);
    Ok((
        StatusCode::ACCEPTED,
        [(header::LOCATION, location)],
        Json(SubmissionResponse::from(submission)),
    ))
}

async fn fetch(
    State(repository): State<PostgresSubmissions>,
    Extension(organization_id): Extension<Uuid>,
    Path(id): Path<String>,
) -> Result<Json<SubmissionResponse>, ApiError> {
    let id = id.parse().map_err(|_| ApiError::not_found())?;
    let submission = repository
        .get(organization_id, id)
        .await?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(SubmissionResponse::from(submission)))
}

fn single_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let all = headers.get_all(name);
    let mut values = all.iter();
    let value = values.next()?.to_str().ok()?;
    values.next().is_none().then_some(value)
}
