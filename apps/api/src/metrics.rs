use std::time::Instant;

use axum::{
    Router,
    extract::Request,
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
};

pub fn router() -> Router {
    Router::new().route("/metrics", get(metrics_handler))
}

async fn metrics_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "text/plain; version=0.0.4; charset=utf-8",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        ferrum_runtime::render_metrics(),
    )
}

pub async fn track_http_metrics(request: Request, next: Next) -> Response {
    let method = match request.method().as_str() {
        "GET" => "GET",
        "POST" => "POST",
        "DELETE" => "DELETE",
        "PUT" => "PUT",
        "PATCH" => "PATCH",
        _ => "OTHER",
    };
    let path = normalize_path(request.uri().path());
    let incoming_trace = request
        .headers()
        .get("traceparent")
        .and_then(|v| v.to_str().ok());
    let trace_context = ferrum_domain::TraceContext::from_traceparent_or_generate(incoming_trace);
    let traceparent_str = trace_context.to_traceparent();
    let trace_id = trace_context.trace_id.clone();

    let start = Instant::now();
    let mut response = next.run(request).await;
    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    let labels = [
        ("method", method),
        ("path", path),
        ("status", status.as_str()),
    ];
    let metrics = ferrum_runtime::MetricsRegistry::global();
    metrics.http_requests_total.inc(&labels);
    metrics
        .http_request_duration_seconds
        .observe(&labels, duration);

    if let Ok(val) = HeaderValue::from_str(&traceparent_str) {
        response.headers_mut().insert("traceparent", val);
    }
    if let Ok(val) = HeaderValue::from_str(&trace_id) {
        response.headers_mut().insert("x-request-id", val);
    }

    response
}

fn normalize_path(path: &str) -> &'static str {
    if path == "/health/live" {
        "/health/live"
    } else if path == "/health/ready" {
        "/health/ready"
    } else if path == "/metrics" {
        "/metrics"
    } else if path == "/v1/languages" {
        "/v1/languages"
    } else if path == "/v1/statuses" {
        "/v1/statuses"
    } else if path == "/v1/me" {
        "/v1/me"
    } else if path == "/v1/submissions" {
        "/v1/submissions"
    } else if path == "/v1/submissions/batch" {
        "/v1/submissions/batch"
    } else if path.starts_with("/v1/submissions/") && path.ends_with("/cancel") {
        "/v1/submissions/{id}/cancel"
    } else if path.starts_with("/v1/submissions/") {
        "/v1/submissions/{id}"
    } else if path == "/v1/api-keys" {
        "/v1/api-keys"
    } else if path.starts_with("/v1/api-keys/") {
        "/v1/api-keys/{id}"
    } else if path == "/v1/webhooks" {
        "/v1/webhooks"
    } else if path.starts_with("/v1/webhooks/") {
        "/v1/webhooks/{id}"
    } else {
        "other"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_normalization_prevents_high_cardinality() {
        assert_eq!(normalize_path("/v1/submissions"), "/v1/submissions");
        assert_eq!(
            normalize_path("/v1/submissions/c217250a-0ca1-410d-9ead-9a4a33099dba"),
            "/v1/submissions/{id}"
        );
        assert_eq!(
            normalize_path("/v1/submissions/c217250a-0ca1-410d-9ead-9a4a33099dba/cancel"),
            "/v1/submissions/{id}/cancel"
        );
        assert_eq!(normalize_path("/unknown/path"), "other");
    }

    #[tokio::test]
    async fn metrics_endpoint_serves_prometheus_text() {
        use axum::body::Body;
        use tower::ServiceExt;

        let app = router();
        let request = axum::http::Request::builder()
            .uri("/metrics")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/plain; version=0.0.4; charset=utf-8"
        );
    }
}
