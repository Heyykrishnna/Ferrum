use ferrum_runtime::MetricsRegistry;
use sqlx::PgPool;

pub async fn collect_storage_metrics(pool: &PgPool) -> Result<(), sqlx::Error> {
    let metrics = MetricsRegistry::global();

    let total_conns = pool.size() as i64;
    let idle_conns = pool.num_idle() as i64;
    let active_conns = (total_conns - idle_conns).max(0);

    metrics.database_connections_active.set(&[], active_conns);
    metrics.database_connections_idle.set(&[], idle_conns);

    let (unpublished_count, oldest_age): (i64, f64) = sqlx::query_as(
        "SELECT COUNT(*)::int8,
                COALESCE(EXTRACT(EPOCH FROM (clock_timestamp() - MIN(created_at))), 0.0)::float8
         FROM outbox_events
         WHERE published_at IS NULL",
    )
    .fetch_one(pool)
    .await?;

    metrics.outbox_unpublished_count.set(&[], unpublished_count);
    metrics
        .outbox_oldest_event_age_seconds
        .set(&[], oldest_age.max(0.0) as i64);

    let quarantined_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::int8
         FROM execution_attempts
         WHERE cleanup_disposition = 'unresolved' AND cleanup_recovered_at IS NULL",
    )
    .fetch_one(pool)
    .await?;

    metrics
        .worker_capacity_quarantined_slots
        .set(&[], quarantined_count);

    Ok(())
}
