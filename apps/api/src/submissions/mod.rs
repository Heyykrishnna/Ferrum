pub mod error;

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Extension, Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode, header},
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use ferrum_application::{IdempotencyKey, SubmissionRepository};
use ferrum_config::DevelopmentCaller;
use ferrum_domain::{
    ApiKeyScope, AuthContext, ComparisonMode, LANGUAGES, ProcessingState, RequestedLimits,
    Submission, SubmissionInput, SubmissionResult, TestCase, Verdict,
};
use ferrum_infrastructure::PostgresSubmissions;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use self::error::ApiError;
use crate::auth::{AuthState, authenticate, require_scope, single_header};

const MAX_BODY_BYTES: usize = 800 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateSubmission {
    language: String,
    source: String,
    stdin: Option<String>,
    #[serde(default)]
    expected_output: Option<String>,
    #[serde(default)]
    comparison_mode: Option<ComparisonMode>,
    #[serde(default)]
    tests: Option<Vec<TestCaseInput>>,
    #[serde(default)]
    limits: RequestedLimits,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TestCaseInput {
    input: Option<String>,
    expected_output: String,
    #[serde(default = "default_weight")]
    weight: u32,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    comparison_mode: Option<ComparisonMode>,
}

fn default_weight() -> u32 {
    1
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
    let auth_state = AuthState {
        repository: repository.clone(),
        caller: Arc::new(caller),
    };

    Router::new()
        .route("/v1/languages", get(languages))
        .route("/v1/submissions", post(create))
        .route("/v1/submissions/{id}", get(fetch))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .route_layer(middleware::from_fn_with_state(
            auth_state,
            authenticate,
        ))
        .with_state(repository)
}

async fn create(
    State(repository): State<PostgresSubmissions>,
    Extension(auth): Extension<AuthContext>,
    headers: HeaderMap,
    request: Result<Json<CreateSubmission>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    require_scope(&auth, ApiKeyScope::SubmissionsWrite)?;

    let admitted = repository
        .check_tenant_concurrency(auth.organization_id, 50)
        .await?;
    if !admitted {
        return Err(ApiError::too_many_requests(
            "tenant_quota_exceeded",
            "Concurrent submission quota exceeded for organization",
        ));
    }

    let key = single_header(&headers, "idempotency-key").ok_or_else(ApiError::idempotency_key)?;
    let key = IdempotencyKey::parse(key)?;
    let Json(request) = request.map_err(ApiError::from_json)?;
    let cases = match request.tests {
        Some(tests) => tests
            .into_iter()
            .enumerate()
            .map(|(i, t)| TestCase {
                index: (i + 1) as u32,
                input: t.input.unwrap_or_default(),
                expected_output: t.expected_output,
                weight: t.weight,
                hidden: t.hidden,
                comparison_mode: t
                    .comparison_mode
                    .or(request.comparison_mode)
                    .unwrap_or(ComparisonMode::Exact),
            })
            .collect(),
        None => match request.expected_output {
            Some(expected_output) => vec![TestCase {
                index: 1,
                input: request.stdin.clone().unwrap_or_default(),
                expected_output,
                weight: 1,
                hidden: false,
                comparison_mode: request.comparison_mode.unwrap_or(ComparisonMode::Exact),
            }],
            None => Vec::new(),
        },
    };
    let input = SubmissionInput::with_stdin_and_cases(
        &request.language,
        request.source,
        request.stdin,
        cases,
        request.limits,
    )?;
    let submission = repository.create(auth.organization_id, key, input).await?;
    let location = format!("/v1/submissions/{}", submission.id);
    Ok((
        StatusCode::ACCEPTED,
        [(header::LOCATION, location)],
        Json(SubmissionResponse::from(submission)),
    ))
}

async fn fetch(
    State(repository): State<PostgresSubmissions>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> Result<Json<SubmissionResponse>, ApiError> {
    require_scope(&auth, ApiKeyScope::SubmissionsRead)?;
    let id = id.parse().map_err(|_| ApiError::not_found())?;
    let submission = repository
        .get(auth.organization_id, id)
        .await?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(SubmissionResponse::from(submission)))
}
