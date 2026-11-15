mod health;

use std::{error::Error, process::ExitCode};

use ferrum_config::ApiSettings;
use ferrum_runtime::{Shutdown, init_tracing};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ferrum-api: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    let settings = ApiSettings::load()?;
    init_tracing(settings.process.log_level)?;
    let shutdown = Shutdown::install()?;
    let listener = TcpListener::bind(settings.listen_addr)
        .await
        .map_err(|error| format!("could not bind the HTTP listener: {error}"))?;
    let cancelled = shutdown.token();
    let router = health::router(cancelled.clone());

    tracing::info!(
        event = "api_listening",
        mode = settings.process.mode.as_str(),
        listen_addr = %listener.local_addr()?,
    );

    shutdown
        .run(settings.process.shutdown_timeout, async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(cancelled.cancelled_owned())
                .await
        })
        .await?;
    tracing::info!(event = "api_stopped");
    Ok(())
}
