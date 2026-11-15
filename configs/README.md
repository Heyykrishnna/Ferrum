# Configuration

Phase 1 introduces typed application and worker settings. Phase 3 adds sandbox policy and a trusted Python profile. Phase 5 expands the language registry.

| Planned path | Scope |
|---|---|
| `application.toml` | HTTP listener, request sizes and database pool thresholds |
| `workers.toml` | Queue polling, leases, graceful drain and admission |
| `sandbox.toml` | Supported host profile, resource limits and cleanup budget |
| `languages/` | Stable IDs, pinned toolchains, fixed executables and argument vectors |

Apply defaults, then files, then environment overrides. Validate at startup and reject unknown security settings. Keep secrets out of logs, language profiles and child environments. The root [.env.example](../.env.example) defines only the local database settings.
