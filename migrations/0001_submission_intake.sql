CREATE TABLE organizations (
    id uuid PRIMARY KEY,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE submissions (
    organization_id uuid NOT NULL REFERENCES organizations (id),
    id uuid NOT NULL,
    idempotency_key text COLLATE "C" NOT NULL
        CHECK (idempotency_key ~ '^[A-Za-z0-9_.:-]{1,128}$'),
    language text NOT NULL CHECK (language <> ''),
    runtime_profile text NOT NULL CHECK (runtime_profile <> ''),
    source text NOT NULL CHECK (octet_length(source) BETWEEN 1 AND 65536),
    stdin text NOT NULL CHECK (octet_length(stdin) <= 65536),
    limits jsonb NOT NULL CHECK (jsonb_typeof(limits) = 'object'),
    state text NOT NULL DEFAULT 'created'
        CHECK (state IN ('created', 'queued', 'processing', 'completed', 'failed', 'cancelled')),
    verdict text
        CHECK (verdict IN ('accepted', 'wrong_answer', 'compilation_error', 'runtime_error',
            'time_limit_exceeded', 'memory_limit_exceeded', 'output_limit_exceeded', 'internal_error')),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (organization_id, id),
    UNIQUE (organization_id, idempotency_key)
);

CREATE TABLE outbox_events (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    submission_id uuid NOT NULL,
    event_type text NOT NULL CHECK (event_type <> ''),
    event_version smallint NOT NULL CHECK (event_version > 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    published_at timestamptz,
    FOREIGN KEY (organization_id, submission_id) REFERENCES submissions (organization_id, id)
);

CREATE UNIQUE INDEX outbox_submission_created_once
    ON outbox_events (organization_id, submission_id)
    WHERE event_type = 'submission.created';

CREATE INDEX outbox_unpublished_order
    ON outbox_events (created_at, id)
    WHERE published_at IS NULL;
