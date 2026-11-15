use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    routing::get,
};
use ferrum_infrastructure::PostgresSubmissions;
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

#[derive(Clone)]
struct HealthState {
    cancelled: CancellationToken,
    database: Option<PostgresSubmissions>,
}

pub fn router(cancelled: CancellationToken, database: Option<PostgresSubmissions>) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .with_state(HealthState {
            cancelled,
            database,
        })
}

async fn live() -> HealthResponse {
    response(StatusCode::OK, "live")
}

async fn ready(State(state): State<HealthState>) -> HealthResponse {
    let ready = tokio::select! {
        biased;
        _ = state.cancelled.cancelled() => return response(StatusCode::SERVICE_UNAVAILABLE, "draining"),
        ready = async {
            match state.database {
                Some(database) => database.is_ready().await,
                None => true,
            }
        } => ready,
    };
    if ready {
        response(StatusCode::OK, "ready")
    } else {
        response(StatusCode::SERVICE_UNAVAILABLE, "database_unavailable")
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
        let app = router(cancelled.clone(), None);
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
