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
        _ => Err("usage: ferrum-worker [migrate]".into()),
    }
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
        "SELECT to_regclass('execution_attempts') IS NOT NULL AND to_regclass('apalis.jobs') IS NOT NULL",
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

    shutdown.run(settings.process.shutdown_timeout, async move {
        let sandbox = Isolate::open().await?;
        recover_after_host_probe(&pool, owner).await?;
        tracing::info!(event = "worker_started", mode = settings.process.mode.as_str(), worker_id = %owner.worker_id);
        pipeline::run(pool, owner, sandbox, stop).await?;
        tracing::info!(event = "worker_stopped");
        Ok::<(), Failure>(())
    }).await.map_err(|error| -> Failure { error.to_string().into() })
}
