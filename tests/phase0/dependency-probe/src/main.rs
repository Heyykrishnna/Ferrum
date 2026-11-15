use std::{env, error::Error, time::Duration};

use apalis::prelude::WorkerBuilder;
use apalis_postgres::PostgresStorage;
use axum::{Router, routing::get};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let database_url = env::var("FERRUM_PHASE0_DATABASE_URL")
        .map_err(|_| "set FERRUM_PHASE0_DATABASE_URL to a local PostgreSQL 17 test database")?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await
        .map_err(|_| "could not connect to the test database")?;

    let version: String = sqlx::query_scalar("SHOW server_version_num")
        .fetch_one(&pool)
        .await?;
    if version.parse::<u32>()? / 10_000 != 17 {
        return Err("the Phase 0 baseline requires PostgreSQL 17".into());
    }

    let _: Router = Router::new().route("/", get(|| async { "phase0" }));
    let _worker = WorkerBuilder::new("ferrum-phase0-probe")
        .backend(PostgresStorage::new(&pool))
        .build(|_: u32| async {});

    pool.close().await;
    println!("SQLx connected to PostgreSQL {version}; Axum router and Apalis worker built");
    Ok(())
}
