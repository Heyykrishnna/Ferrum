mod health;
mod submissions;

use std::{env, error::Error, ffi::OsStr, process::ExitCode};

use ferrum_config::{ApiSettings, DatabaseSettings};
use ferrum_infrastructure::PostgresSubmissions;
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
    let mut arguments = env::args_os().skip(1);
    match (arguments.next().as_deref(), arguments.next()) {
        (None, None) => serve().await,
        (Some(command), None) if command == OsStr::new("migrate") => {
            let settings = DatabaseSettings::load()?;
            ferrum_infrastructure::migrate(&settings).await?;
            println!("PostgreSQL migrations applied");
            Ok(())
        }
        _ => Err("usage: ferrum-api [migrate]".into()),
    }
}

async fn serve() -> Result<(), Box<dyn Error + Send + Sync>> {
    let settings = ApiSettings::load()?;
    init_tracing(settings.process.log_level)?;
    let shutdown = Shutdown::install()?;
    let submission_state = if let Some(settings) = settings.submissions {
        let pool = ferrum_infrastructure::connect(&settings.database).await?;
        let repository =
            PostgresSubmissions::initialize(pool, settings.caller.organization_id).await?;
        Some((repository, settings.caller))
    } else {
        None
    };
    let listener = TcpListener::bind(settings.listen_addr)
        .await
        .map_err(|error| format!("could not bind the HTTP listener: {error}"))?;
    let cancelled = shutdown.token();
    let mut router = health::router(
        cancelled.clone(),
        submission_state
            .as_ref()
            .map(|(repository, _)| repository.clone()),
    );
    if let Some((repository, caller)) = submission_state {
        router = router.merge(submissions::router(repository, caller));
    }

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
