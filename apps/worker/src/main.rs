#[cfg(any(test, all(target_os = "linux", target_arch = "x86_64")))]
mod pipeline;

use std::{error::Error, process::ExitCode};

use ferrum_config::DatabaseSettings;
use ferrum_infrastructure::{connect, dispatcher::setup_apalis};

type Failure = Box<dyn Error + Send + Sync>;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ferrum-worker: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Failure> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.as_slice() {
        [] => serve().await,
        [command] if command == "migrate" => {
            let settings = DatabaseSettings::load()?;
            let pool = connect(&settings).await?;
            setup_apalis(&pool).await?;
            Ok(())
        }
        [command] if command == "reconcile" => reconcile_cli().await,
        [command] if command == "metrics" => metrics_cli().await,
        [cmd, sub] if cmd == "quarantine" && sub == "list" => quarantine_list_cli().await,
        [cmd, sub, id] if cmd == "quarantine" && sub == "resolve" => {
            quarantine_resolve_cli(id).await
        }
        [cmd, sub, id] if cmd == "quarantine" && sub == "resolve-host" => {
            quarantine_resolve_host_cli(id).await
        }
        _ => Err(
            "usage: ferrum-worker [migrate | reconcile | metrics | quarantine list | quarantine resolve <attempt_id> | quarantine resolve-host <host_id>]".into(),
        ),
    }
}

async fn reconcile_cli() -> Result<(), Failure> {
    let settings = DatabaseSettings::load()?;
    let pool = connect(&settings).await?;
    let attempts = ferrum_infrastructure::reconcile_stale_attempts(&pool).await?;
    let outbox_count =
        ferrum_infrastructure::reconcile_stuck_outbox(&pool, std::time::Duration::from_secs(30))
            .await?;
    let summary = ferrum_domain::ReconciliationSummary {
        stale_attempts_reconciled: attempts.len(),
        stuck_outbox_events_reconciled: outbox_count,
    };
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

async fn metrics_cli() -> Result<(), Failure> {
    let settings = DatabaseSettings::load()?;
    let pool = connect(&settings).await?;
    ferrum_infrastructure::collect_storage_metrics(&pool).await?;
    print!("{}", ferrum_runtime::render_metrics());
    Ok(())
}

pub fn metrics_router() -> axum::Router {
    axum::Router::new()
        .route(
            "/metrics",
            axum::routing::get(|| async {
                (
                    axum::http::StatusCode::OK,
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/plain; version=0.0.4; charset=utf-8",
                    )],
                    ferrum_runtime::render_metrics(),
                )
            }),
        )
        .route(
            "/health/live",
            axum::routing::get(|| async { "{\"status\":\"live\"}" }),
        )
}

async fn quarantine_list_cli() -> Result<(), Failure> {
    let settings = DatabaseSettings::load()?;
    let pool = connect(&settings).await?;
    let quarantined = ferrum_infrastructure::list_quarantined_attempts(&pool).await?;
    println!("{}", serde_json::to_string_pretty(&quarantined)?);
    Ok(())
}

async fn quarantine_resolve_cli(attempt_id_str: &std::ffi::OsStr) -> Result<(), Failure> {
    let s = attempt_id_str
        .to_str()
        .ok_or("invalid UTF-8 in attempt_id")?;
    let attempt_id: uuid::Uuid = s.parse().map_err(|_| "invalid UUID for attempt_id")?;
    let settings = DatabaseSettings::load()?;
    let pool = connect(&settings).await?;
    let resolved = ferrum_infrastructure::resolve_quarantine(&pool, attempt_id).await?;
    if resolved {
        println!("quarantine resolved for attempt {attempt_id}");
        Ok(())
    } else {
        Err(format!("no active quarantine found for attempt {attempt_id}").into())
    }
}

async fn quarantine_resolve_host_cli(host_id_str: &std::ffi::OsStr) -> Result<(), Failure> {
    let s = host_id_str.to_str().ok_or("invalid UTF-8 in host_id")?;
    let host_id: uuid::Uuid = s.parse().map_err(|_| "invalid UUID for host_id")?;
    let settings = DatabaseSettings::load()?;
    let pool = connect(&settings).await?;
    let count = ferrum_infrastructure::resolve_all_quarantines_for_host(&pool, host_id).await?;
    println!("resolved {count} quarantined attempts for host {host_id}");
    Ok(())
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
async fn serve() -> Result<(), Failure> {
    Err("execution requires the qualified Debian 13 amd64 Isolate host described in docs/phase3-isolate.md".into())
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
async fn serve() -> Result<(), Failure> {
    use ferrum_config::WorkerSettings;
    use ferrum_infrastructure::execution::{ExecutionOwner, recover_after_host_probe};
    use ferrum_runtime::{Shutdown, init_tracing};
    use ferrum_sandbox::isolate::Isolate;
    use uuid::Uuid;

    let settings = WorkerSettings::load()?;
    init_tracing(settings.process.log_level)?;
    let machine_id = std::fs::read_to_string("/etc/machine-id")
        .ok()
        .and_then(|id| id.trim().parse::<Uuid>().ok())
        .filter(|id| !id.is_nil())
        .ok_or("configure a unique /etc/machine-id on this worker host")?;
    let owner = ExecutionOwner {
        host_id: Uuid::new_v5(
            &uuid::uuid!("c217250a-0ca1-410d-9ead-9a4a33099dba"),
            machine_id.as_bytes(),
        ),
        worker_id: Uuid::new_v4(),
    };
    let database = DatabaseSettings::load()?;
    let pool = connect(&database).await?;
    let schema_ready = sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('execution_attempts') IS NOT NULL AND to_regclass('compilation_results') IS NOT NULL AND to_regclass('submission_case_results') IS NOT NULL AND to_regclass('api_keys') IS NOT NULL AND to_regclass('apalis.jobs') IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .map_err(|_| "could not verify the worker database schema")?;
    if !schema_ready {
        return Err(
            "apply the ferrum-api and ferrum-worker migrations before starting the worker".into(),
        );
    }
    let shutdown = Shutdown::install()?;
    let stop = shutdown.token();

    if let Some(metrics_addr) = settings.metrics_listen_addr {
        let metrics_stop = stop.clone();
        tokio::spawn(async move {
            if let Ok(listener) = tokio::net::TcpListener::bind(metrics_addr).await {
                tracing::info!(event = "worker_metrics_listening", addr = %metrics_addr);
                let app = metrics_router();
                let _ = axum::serve(listener, app)
                    .with_graceful_shutdown(metrics_stop.cancelled_owned())
                    .await;
            }
        });
    }

    shutdown.run(settings.process.shutdown_timeout, async move {
        let sandbox = Isolate::open_with_toolchains().await?;
        recover_after_host_probe(&pool, owner).await?;
        tracing::info!(event = "worker_started", mode = settings.process.mode.as_str(), worker_id = %owner.worker_id);
        pipeline::run(pool, owner, sandbox, stop).await?;
        tracing::info!(event = "worker_stopped");
        Ok::<(), Failure>(())
    }).await.map_err(|error| -> Failure { error.to_string().into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower::ServiceExt;

    #[tokio::test]
    async fn worker_metrics_router_serves_metrics_and_live() {
        let app = metrics_router();
        let request = axum::http::Request::builder()
            .uri("/metrics")
            .body(axum::body::Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let live_request = axum::http::Request::builder()
            .uri("/health/live")
            .body(axum::body::Body::empty())
            .unwrap();
        let live_response = app.oneshot(live_request).await.unwrap();
        assert_eq!(live_response.status(), axum::http::StatusCode::OK);
    }
}
