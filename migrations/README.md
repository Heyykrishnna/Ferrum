# Migrations

[Phase 2](../phases/02-database-and-submissions.md) introduces SQLx migrations for submissions and the outbox. Add attempt and result records when the worker needs them, judging records with Phase 6, and key-management or webhook records with Phase 7. Keep Apalis vendor schema changes separate using the selected backend's documented setup.

Use database constraints for tenant-scoped idempotency and attempt ownership. Keep submission and outbox writes atomic. Review indexes against actual queries. Add a new migration to change a schema already in use. Exercise both fresh installation and upgrade on disposable databases.

Run `cargo run --locked -p ferrum-api -- migrate` from the workspace root with `FERRUM_DATABASE_URL` set. The API embeds these SQLx migrations. Use a database role with schema privileges for the migration command. The serving role needs access to read and insert into the Phase 2 tables as required for intake. Use a disposable database for testing.

`0001_submission_intake.sql` creates the minimal tenant table, submissions, and the outbox. The outbox stores only identifiers, event metadata, and timestamps. Phase 4 can scan unpublished events by creation time and ID and build the queue message from these fields.
