use axum::{
    Json,
    extract::rejection::JsonRejection,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use ferrum_application::SubmissionError;
use ferrum_domain::ValidationError;
use serde::Serialize;

pub struct ApiError {
    status: StatusCode,
    details: ErrorDetails,
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetails,
}

#[derive(Serialize)]
struct ErrorDetails {
    code: &'static str,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            details: ErrorDetails {
                code,
                message: message.into(),
            },
        }
    }

    pub fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "A valid bearer token is required",
        )
    }

    pub fn forbidden(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, code, message)
    }

    pub fn too_many_requests(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, code, message)
    }

    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    pub fn not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "submission_not_found",
            "Submission not found",
        )
    }

    pub fn idempotency_key() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_idempotency_key",
            "Provide one Idempotency-Key of 1 to 128 ASCII letters, digits, or -_.:",
        )
    }

    pub fn from_json(error: JsonRejection) -> Self {
        match error.status() {
            StatusCode::PAYLOAD_TOO_LARGE => Self::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "body_too_large",
                "Request body exceeds 800 KiB",
            ),
            StatusCode::UNSUPPORTED_MEDIA_TYPE => Self::new(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
                "Use Content-Type: application/json",
            ),
            _ => Self::new(
                StatusCode::BAD_REQUEST,
                "invalid_json",
                "Invalid submission JSON",
            ),
        }
    }
}

impl From<ValidationError> for ApiError {
    fn from(error: ValidationError) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_submission",
            error.to_string(),
        )
    }
}

impl From<SubmissionError> for ApiError {
    fn from(error: SubmissionError) -> Self {
        match error {
            SubmissionError::InvalidIdempotencyKey => Self::idempotency_key(),
            SubmissionError::IdempotencyConflict => Self::new(
                StatusCode::CONFLICT,
                "idempotency_conflict",
                error.to_string(),
            ),
            SubmissionError::BatchLimitExceeded => Self::new(
                StatusCode::BAD_REQUEST,
                "batch_limit_exceeded",
                error.to_string(),
            ),
            SubmissionError::Unavailable => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "storage_unavailable",
                error.to_string(),
            ),
            SubmissionError::Storage => Self::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "storage_error",
                error.to_string(),
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(ErrorBody {
                error: self.details,
            }),
        )
            .into_response();
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        if self.status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}
