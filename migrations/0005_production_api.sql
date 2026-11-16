CREATE TABLE api_keys (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organizations (id),
    name text NOT NULL CHECK (name <> ''),
    key_prefix text NOT NULL CHECK (octet_length(key_prefix) BETWEEN 8 AND 32),
    key_hash text NOT NULL UNIQUE CHECK (octet_length(key_hash) = 64),
    scopes text[] NOT NULL DEFAULT '{"submissions:read", "submissions:write"}',
    expires_at timestamptz,
    revoked_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX api_keys_organization_idx ON api_keys (organization_id);
CREATE INDEX api_keys_hash_active_idx ON api_keys (key_hash) WHERE revoked_at IS NULL;

CREATE TABLE webhook_endpoints (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL REFERENCES organizations (id),
    url text NOT NULL CHECK (url ~ '^https?://'),
    secret text NOT NULL CHECK (octet_length(secret) >= 16),
    events text[] NOT NULL DEFAULT '{"submission.completed", "submission.failed", "submission.cancelled"}',
    active boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX webhook_endpoints_organization_idx ON webhook_endpoints (organization_id);

CREATE TABLE webhook_deliveries (
    id uuid PRIMARY KEY,
    organization_id uuid NOT NULL,
    endpoint_id uuid NOT NULL REFERENCES webhook_endpoints (id) ON DELETE CASCADE,
    event_id uuid NOT NULL,
    event_type text NOT NULL,
    status_code smallint,
    success boolean NOT NULL,
    delivered_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX webhook_deliveries_org_endpoint_idx ON webhook_deliveries (organization_id, endpoint_id);

ALTER TABLE submissions ADD COLUMN cancel_requested boolean NOT NULL DEFAULT false;
