pub mod api_keys;
pub mod dispatcher;
pub mod execution;
mod submissions;
pub mod webhooks;

use std::time::Duration;

use ferrum_config::DatabaseSettings;
use sqlx::{
    PgPool,
    migrate::Migrator,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use thiserror::Error;

pub use api_keys::{
    authenticate_api_key, create_api_key, generate_raw_key, hash_key, list_api_keys,
    revoke_api_key,
};
pub use dispatcher::{dispatch_execution, execution_queue, setup_apalis};
pub use execution::{
    AttemptReport, ExecutionError, ExecutionInput, ExecutionOwner, claim_execution,
    finalize_execution, recover_after_host_probe, renew_attempt_lease,
};
pub use submissions::PostgresSubmissions;
pub use webhooks::{
    WebhookError, create_webhook_endpoint, delete_webhook_endpoint, generate_webhook_secret,
    list_webhook_endpoints, record_webhook_delivery, sign_webhook_payload, validate_webhook_url,
};

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("invalid FERRUM_DATABASE_URL")]
    InvalidUrl,
    #[error("could not connect to PostgreSQL")]
    Connect,
    #[error("could not apply PostgreSQL migrations")]
    Migrate,
    #[error(
        "could not initialize the development tenant; verify that migrations are applied and the database role has access"
    )]
    Initialize,
}

pub async fn connect(settings: &DatabaseSettings) -> Result<PgPool, DatabaseError> {
    let options = settings
        .url
        .parse::<PgConnectOptions>()
        .map_err(|_| DatabaseError::InvalidUrl)?
        .options([
            ("statement_timeout", "5000"),
            ("idle_in_transaction_session_timeout", "10000"),
        ]);
    PgPoolOptions::new()
        .max_connections(settings.max_connections)
        .acquire_timeout(settings.acquire_timeout)
        .connect_with(options)
        .await
        .map_err(|error| {
            log_sql_failure("connect", &error);
            DatabaseError::Connect
        })
}

pub async fn migrate(settings: &DatabaseSettings) -> Result<(), DatabaseError> {
    let pool = connect(settings).await?;
    let result = MIGRATOR.run(&pool).await.map_err(|error| {
        if let sqlx::migrate::MigrateError::Execute(error) = &error {
            log_sql_failure("migrate", error);
        }
        DatabaseError::Migrate
    });
    pool.close().await;
    result
}

async fn database_ready(pool: &PgPool) -> bool {
    matches!(
        tokio::time::timeout(
            Duration::from_secs(1),
            sqlx::query("SELECT 1").execute(pool)
        )
        .await,
        Ok(Ok(_))
    )
}

fn log_sql_failure(operation: &'static str, error: &sqlx::Error) {
    let code = error.as_database_error().and_then(|error| error.code());
    tracing::warn!(
        event = "postgres_operation_failed",
        operation,
        sqlstate = code.as_deref()
    );
}
