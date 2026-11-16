ALTER TABLE submissions ADD COLUMN IF NOT EXISTS traceparent text;
ALTER TABLE outbox_events ADD COLUMN IF NOT EXISTS traceparent text;
