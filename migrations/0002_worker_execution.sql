CREATE TABLE execution_attempts (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    submission_id uuid NOT NULL,
    attempt_number integer NOT NULL CHECK (attempt_number BETWEEN 1 AND 3),
    state text NOT NULL CHECK (state IN ('queued', 'running', 'completed', 'failed', 'cancelled')),
    host_id uuid,
    worker_id uuid,
    lease_expires_at timestamptz,
    cleanup_disposition text CHECK (cleanup_disposition IN ('not_needed', 'verified', 'unresolved')),
    cleanup_error text,
    cleanup_recovered_at timestamptz,
    failure_reason text,
    started_at timestamptz,
    completed_at timestamptz,
    FOREIGN KEY (organization_id, submission_id) REFERENCES submissions (organization_id, id),
    UNIQUE (organization_id, submission_id, attempt_number),
    UNIQUE (organization_id, submission_id, id)
);

CREATE UNIQUE INDEX execution_attempts_one_active
    ON execution_attempts (organization_id, submission_id)
    WHERE state IN ('queued', 'running');

CREATE INDEX execution_attempts_host_recovery
    ON execution_attempts (host_id, id)
    WHERE state = 'running' OR (cleanup_disposition = 'unresolved' AND cleanup_recovered_at IS NULL);

CREATE TABLE execution_results (
    organization_id uuid NOT NULL,
    submission_id uuid NOT NULL,
    attempt_id uuid NOT NULL UNIQUE,
    outcome text NOT NULL CHECK (outcome IN (
        'success', 'runtime_error', 'cpu_time_limit', 'wall_time_limit', 'memory_limit',
        'output_limit_stdout', 'output_limit_stderr', 'cancelled'
    )),
    stdout bytea NOT NULL CHECK (octet_length(stdout) <= 65536),
    stderr bytea NOT NULL CHECK (octet_length(stderr) <= 65536),
    exit_code smallint CHECK (exit_code BETWEEN 0 AND 255),
    signal smallint CHECK (signal BETWEEN 0 AND 255),
    cpu_time_ms bigint CHECK (cpu_time_ms >= 0),
    wall_time_ms bigint CHECK (wall_time_ms >= 0),
    peak_memory_kib bigint CHECK (peak_memory_kib >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (organization_id, submission_id),
    FOREIGN KEY (organization_id, submission_id, attempt_id)
        REFERENCES execution_attempts (organization_id, submission_id, id)
);

CREATE UNIQUE INDEX outbox_submission_completed_once
    ON outbox_events (organization_id, submission_id)
    WHERE event_type = 'submission.completed';
