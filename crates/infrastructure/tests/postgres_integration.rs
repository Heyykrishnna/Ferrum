use std::time::Duration;

use ferrum_application::{IdempotencyKey, SubmissionError, SubmissionRepository};
use ferrum_config::DatabaseSettings;
use ferrum_domain::{
    ApiKeyScope, ProcessingState, RequestedLimits, SubmissionCancelOutcome, SubmissionInput,
};
use ferrum_infrastructure::{
    PostgresSubmissions, WebhookError, authenticate_api_key, collect_storage_metrics, connect,
    create_api_key, create_webhook_endpoint, delete_webhook_endpoint, dispatch_execution,
    list_api_keys, list_quarantined_attempts, list_webhook_endpoints, migrate,
    reconcile_stale_attempts, reconcile_stuck_outbox, revoke_api_key, setup_apalis,
};
use uuid::Uuid;

fn python_input(source: &str) -> SubmissionInput {
    SubmissionInput::new(
        "python",
        source.to_owned(),
        None,
        RequestedLimits::default(),
    )
    .unwrap()
}

async fn setup_pool() -> sqlx::PgPool {
    let settings = DatabaseSettings::load().expect("set FERRUM_DATABASE_URL for integration tests");
    migrate(&settings).await.expect("apply migrations");
    let pool = connect(&settings).await.expect("connect to PostgreSQL");
    setup_apalis(&pool).await.expect("setup Apalis schema");
    pool
}

#[tokio::test]
#[ignore = "requires PostgreSQL via FERRUM_DATABASE_URL"]
async fn submission_idempotency_tenant_isolation_and_outbox_dispatch() {
    let pool = setup_pool().await;
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let repo_a = PostgresSubmissions::initialize(pool.clone(), tenant_a)
        .await
        .unwrap();
    let repo_b = PostgresSubmissions::initialize(pool.clone(), tenant_b)
        .await
        .unwrap();
    assert!(repo_a.is_ready().await);

    let key = format!("ci-idem-{}", Uuid::new_v4().simple());
    let first = repo_a
        .create(
            tenant_a,
            IdempotencyKey::parse(&key).unwrap(),
            python_input("print(1)"),
        )
        .await
        .unwrap();
    assert_eq!(first.state, ProcessingState::Created);

    let replay = repo_a
        .create(
            tenant_a,
            IdempotencyKey::parse(&key).unwrap(),
            python_input("print(1)"),
        )
        .await
        .unwrap();
    assert_eq!(replay.id, first.id);

    let conflict = repo_a
        .create(
            tenant_a,
            IdempotencyKey::parse(&key).unwrap(),
            python_input("print(2)"),
        )
        .await;
    assert!(matches!(
        conflict,
        Err(SubmissionError::IdempotencyConflict)
    ));

    assert!(repo_b.get(tenant_b, first.id).await.unwrap().is_none());
    assert!(repo_b.cancel(tenant_b, first.id).await.unwrap().is_none());

    let tenant_b_same_key = repo_b
        .create(
            tenant_b,
            IdempotencyKey::parse(&key).unwrap(),
            python_input("print(1)"),
        )
        .await
        .unwrap();
    assert_ne!(tenant_b_same_key.id, first.id);

    let mut dispatched = 0usize;
    while dispatch_execution(&pool).await.unwrap() {
        dispatched += 1;
    }
    assert!(dispatched >= 2);

    let queued = repo_a.get(tenant_a, first.id).await.unwrap().unwrap();
    assert_eq!(queued.state, ProcessingState::Queued);

    let cancelled = repo_a.cancel(tenant_a, first.id).await.unwrap().unwrap();
    assert_eq!(cancelled, SubmissionCancelOutcome::Cancelled);

    let repeat_cancel = repo_a.cancel(tenant_a, first.id).await.unwrap().unwrap();
    assert_eq!(repeat_cancel, SubmissionCancelOutcome::AlreadyTerminal);

    let cancelled_b = repo_b
        .cancel(tenant_b, tenant_b_same_key.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cancelled_b, SubmissionCancelOutcome::Cancelled);

    pool.close().await;
}

#[tokio::test]
#[ignore = "requires PostgreSQL via FERRUM_DATABASE_URL"]
async fn api_keys_webhooks_ssrf_and_reconciliation() {
    let pool = setup_pool().await;
    let tenant = Uuid::new_v4();
    let _repo = PostgresSubmissions::initialize(pool.clone(), tenant)
        .await
        .unwrap();

    let (created_key, raw_key) = create_api_key(
        &pool,
        tenant,
        "ci-integration-key",
        &[ApiKeyScope::SubmissionsRead, ApiKeyScope::SubmissionsWrite],
        None,
    )
    .await
    .unwrap();
    assert!(raw_key.starts_with("ferrum_live_"));

    let auth = authenticate_api_key(&pool, &raw_key)
        .await
        .unwrap()
        .expect("fresh API key must authenticate");
    assert_eq!(auth.organization_id, tenant);
    assert!(auth.has_scope(ApiKeyScope::SubmissionsRead));
    assert!(!auth.has_scope(ApiKeyScope::Admin));

    let keys = list_api_keys(&pool, tenant).await.unwrap();
    assert!(keys.iter().any(|k| k.id == created_key.id));

    assert!(revoke_api_key(&pool, tenant, created_key.id).await.unwrap());
    assert!(
        authenticate_api_key(&pool, &raw_key)
            .await
            .unwrap()
            .is_none()
    );

    for blocked in [
        "http://127.0.0.1:8080/hook",
        "http://169.254.169.254/latest/meta-data/",
        "http://10.0.0.5/internal",
        "http://localhost/admin",
    ] {
        let result =
            create_webhook_endpoint(&pool, tenant, blocked, &["submission.completed".to_owned()])
                .await;
        assert_eq!(result.unwrap_err(), WebhookError::BlockedDestination);
    }

    let invalid_scheme = create_webhook_endpoint(
        &pool,
        tenant,
        "file:///etc/passwd",
        &["submission.completed".to_owned()],
    )
    .await;
    assert_eq!(invalid_scheme.unwrap_err(), WebhookError::InvalidScheme);

    let endpoint = create_webhook_endpoint(
        &pool,
        tenant,
        "https://hooks.example.com/ferrum-ci",
        &["submission.completed".to_owned()],
    )
    .await
    .unwrap();
    let endpoints = list_webhook_endpoints(&pool, tenant).await.unwrap();
    assert!(endpoints.iter().any(|e| e.id == endpoint.id));
    assert!(
        delete_webhook_endpoint(&pool, tenant, endpoint.id)
            .await
            .unwrap()
    );

    let _stale = reconcile_stale_attempts(&pool).await.unwrap();
    let _stuck = reconcile_stuck_outbox(&pool, Duration::from_secs(30))
        .await
        .unwrap();
    let _quarantined = list_quarantined_attempts(&pool).await.unwrap();
    collect_storage_metrics(&pool).await.unwrap();

    pool.close().await;
}
