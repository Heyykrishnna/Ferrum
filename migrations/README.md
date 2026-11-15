# Migrations

[Phase 2](../phases/02-database-and-submissions.md) introduces SQLx migrations for submissions and the outbox. Add attempt and result records when the worker needs them, judging records with Phase 6, and key-management or webhook records with Phase 7. Keep Apalis vendor schema changes separate using the selected backend's documented setup.

Use database constraints for tenant-scoped idempotency and attempt ownership. Keep submission and outbox writes atomic. Review indexes against actual queries. Add a new migration to change a schema already in use. Exercise both fresh installation and upgrade on disposable databases.
