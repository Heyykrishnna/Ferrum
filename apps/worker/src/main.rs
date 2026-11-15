use std::{convert::Infallible, error::Error, process::ExitCode};

use ferrum_config::WorkerSettings;
use ferrum_runtime::{Shutdown, init_tracing};

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

async fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    let settings = WorkerSettings::load()?;
    init_tracing(settings.process.log_level)?;
    let shutdown = Shutdown::install()?;
    let cancelled = shutdown.token();

    tracing::info!(
        event = "worker_started",
        mode = settings.process.mode.as_str()
    );
    shutdown
        .run(settings.process.shutdown_timeout, async move {
            cancelled.cancelled().await;
            Ok::<(), Infallible>(())
        })
        .await?;
    tracing::info!(event = "worker_stopped");
    Ok(())
}
