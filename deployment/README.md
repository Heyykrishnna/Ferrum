# Deployment

## Local PostgreSQL

Run from the Ferrum root with Docker Compose and an accessible Docker daemon. Copy the template and set a unique local password in `.env`. PostgreSQL listens on loopback port 54329 by default; set `FERRUM_POSTGRES_PORT` if the port is in use.

```sh
cp -n .env.example .env
```

After editing `.env`, run:

```sh
docker compose config --quiet
docker compose up -d --wait postgres
docker compose exec -T postgres sh -c 'psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -c "SELECT 1;"'
docker compose down
```

The named data volume survives `docker compose down`. PostgreSQL uses initialization settings when the volume is first created. Changing `.env` does not rotate an existing database password.

## Worker and production environments

Use a supported, disposable Linux host for [Isolate validation](../phases/00-architecture-and-environment.md). This Compose file supplies PostgreSQL only. Production runs separate API and worker processes, with dedicated Linux worker hosts, private PostgreSQL and telemetry, TLS and injected secrets. Select the hosting and secrets providers during [Phase 10](../phases/10-production-release.md).
