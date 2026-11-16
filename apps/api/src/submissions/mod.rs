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
use ferrum_application::{IdempotencyKey, SubmissionError, SubmissionRepository};
use ferrum_config::DevelopmentCaller;
use ferrum_domain::{
    ApiKeyScope, AuthContext, BatchItemResult, BatchSubmissionResult, ComparisonMode, LANGUAGES,
    MAX_BATCH_SIZE, ProcessingState, RequestedLimits, Submission, SubmissionCancelOutcome,
    SubmissionInput, SubmissionResult, TestCase, Verdict,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchSubmissionItem {
    idempotency_key: String,
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
struct BatchSubmissionRequest {
    submissions: Vec<BatchSubmissionItem>,
}

#[derive(Serialize)]
struct CancelResponse {
    id: Uuid,
    outcome: SubmissionCancelOutcome,
}

fn default_weight() -> u32 {
    1
}

fn parse_test_cases(
    tests: Option<Vec<TestCaseInput>>,
    expected_output: Option<String>,
    stdin: Option<&str>,
    comparison_mode: Option<ComparisonMode>,
) -> Vec<TestCase> {
    match tests {
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
                    .or(comparison_mode)
                    .unwrap_or(ComparisonMode::Exact),
            })
            .collect(),
        None => match expected_output {
            Some(expected_output) => vec![TestCase {
                index: 1,
                input: stdin.unwrap_or_default().to_string(),
                expected_output,
                weight: 1,
                hidden: false,
                comparison_mode: comparison_mode.unwrap_or(ComparisonMode::Exact),
            }],
            None => Vec::new(),
        },
    }
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
        .route("/v1/submissions/batch", post(create_batch))
        .route("/v1/submissions/{id}", get(fetch))
        .route("/v1/submissions/{id}/cancel", post(cancel))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .route_layer(middleware::from_fn_with_state(auth_state, authenticate))
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
        ferrum_runtime::MetricsRegistry::global()
            .rate_limit_rejections_total
            .inc(&[("reason", "concurrency_quota")]);
        return Err(ApiError::too_many_requests(
            "tenant_quota_exceeded",
            "Concurrent submission quota exceeded for organization",
        ));
    }

    let key = single_header(&headers, "idempotency-key").ok_or_else(ApiError::idempotency_key)?;
    let key = IdempotencyKey::parse(key)?;
    let Json(request) = request.map_err(ApiError::from_json)?;
    let cases = parse_test_cases(
        request.tests,
        request.expected_output,
        request.stdin.as_deref(),
        request.comparison_mode,
    );
    let trace_header = headers.get("traceparent").and_then(|v| v.to_str().ok());
    let trace_ctx = ferrum_domain::TraceContext::from_traceparent_or_generate(trace_header);
    let input = SubmissionInput::with_stdin_and_cases(
        &request.language,
        request.source,
        request.stdin,
        cases,
        request.limits,
    )?
    .with_traceparent(Some(trace_ctx.to_traceparent()));
    let submission = repository.create(auth.organization_id, key, input).await?;
    ferrum_runtime::MetricsRegistry::global()
        .submissions_created_total
        .inc(&[("language", &request.language)]);
    let location = format!("/v1/submissions/{}", submission.id);
    Ok((
        StatusCode::ACCEPTED,
        [(header::LOCATION, location)],
        Json(SubmissionResponse::from(submission)),
    ))
}

async fn create_batch(
    State(repository): State<PostgresSubmissions>,
    Extension(auth): Extension<AuthContext>,
    request: Result<Json<BatchSubmissionRequest>, JsonRejection>,
) -> Result<Json<BatchSubmissionResult>, ApiError> {
    require_scope(&auth, ApiKeyScope::SubmissionsWrite)?;
    let Json(request) = request.map_err(ApiError::from_json)?;
    if request.submissions.is_empty() {
        return Err(ApiError::bad_request(
            "empty_batch",
            "Batch must contain at least 1 item",
        ));
    }
    if request.submissions.len() > MAX_BATCH_SIZE {
        return Err(ApiError::bad_request(
            "batch_too_large",
            format!("Batch exceeds maximum size of {MAX_BATCH_SIZE}"),
        ));
    }

    let admitted = repository
        .check_tenant_concurrency(auth.organization_id, 50)
        .await?;
    if !admitted {
        return Err(ApiError::too_many_requests(
            "tenant_quota_exceeded",
            "Concurrent submission quota exceeded for organization",
        ));
    }

    let mut results = Vec::with_capacity(request.submissions.len());
    for item in request.submissions {
        let key_str = item.idempotency_key.clone();
        let key = match IdempotencyKey::parse(&item.idempotency_key) {
            Ok(k) => k,
            Err(e) => {
                results.push(BatchItemResult {
                    idempotency_key: key_str,
                    id: None,
                    state: None,
                    error: Some(e.to_string()),
                });
                continue;
            }
        };
        let cases = parse_test_cases(
            item.tests,
            item.expected_output,
            item.stdin.as_deref(),
            item.comparison_mode,
        );
        let input = match SubmissionInput::with_stdin_and_cases(
            &item.language,
            item.source,
            item.stdin,
            cases,
            item.limits,
        ) {
            Ok(i) => i,
            Err(e) => {
                results.push(BatchItemResult {
                    idempotency_key: key_str,
                    id: None,
                    state: None,
                    error: Some(e.to_string()),
                });
                continue;
            }
        };

        match repository.create(auth.organization_id, key, input).await {
            Ok(sub) => {
                results.push(BatchItemResult {
                    idempotency_key: key_str,
                    id: Some(sub.id),
                    state: Some(sub.state),
                    error: None,
                });
            }
            Err(SubmissionError::IdempotencyConflict) => {
                results.push(BatchItemResult {
                    idempotency_key: key_str,
                    id: None,
                    state: None,
                    error: Some("idempotency_conflict".to_string()),
                });
            }
            Err(SubmissionError::Unavailable) => {
                return Err(SubmissionError::Unavailable.into());
            }
            Err(err) => {
                results.push(BatchItemResult {
                    idempotency_key: key_str,
                    id: None,
                    state: None,
                    error: Some(err.to_string()),
                });
            }
        }
    }

    Ok(Json(BatchSubmissionResult { items: results }))
}

async fn cancel(
    State(repository): State<PostgresSubmissions>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> Result<Json<CancelResponse>, ApiError> {
    require_scope(&auth, ApiKeyScope::SubmissionsCancel)?;
    let id = id.parse().map_err(|_| ApiError::not_found())?;
    let outcome = repository
        .cancel(auth.organization_id, id)
        .await?
        .ok_or_else(ApiError::not_found)?;
    Ok(Json(CancelResponse { id, outcome }))
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
