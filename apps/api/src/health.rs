use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    routing::get,
};
use serde::Serialize;
use tokio_util::sync::CancellationToken;

#[derive(Serialize)]
struct Health {
    status: &'static str,
}

type HealthResponse = (
    StatusCode,
    [(header::HeaderName, &'static str); 1],
    Json<Health>,
);

pub fn router(cancelled: CancellationToken) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .with_state(cancelled)
}

async fn live() -> HealthResponse {
    response(StatusCode::OK, "live")
}

async fn ready(State(cancelled): State<CancellationToken>) -> HealthResponse {
    if cancelled.is_cancelled() {
        response(StatusCode::SERVICE_UNAVAILABLE, "draining")
    } else {
        response(StatusCode::OK, "ready")
    }
}

fn response(code: StatusCode, status: &'static str) -> HealthResponse {
    (
        code,
        [(header::CACHE_CONTROL, "no-store")],
        Json(Health { status }),
    )
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn draining_only_changes_readiness() {
        let cancelled = CancellationToken::new();
        let app = router(cancelled.clone());
        cancelled.cancel();

        for (path, status) in [
            ("/health/ready", StatusCode::SERVICE_UNAVAILABLE),
            ("/health/live", StatusCode::OK),
        ] {
            let request = Request::get(path).body(Body::empty()).unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), status);
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        }
    }
}
