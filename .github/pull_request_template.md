## Summary

- What changed and why:

## Security and Reliability Checklist

- [ ] No secrets, tokens, or credentials committed or logged
- [ ] Untrusted code boundaries (sandbox, process invocation, path handling, network isolation) remain fail-closed
- [ ] Database schema changes use monotonic migrations under `migrations/` without mutating previously applied migrations
- [ ] API/OpenAPI contract changes are synchronized between `docs/openapi.yaml` and `docs/openapi.json`
- [ ] New or changed behavior includes focused tests

## Verification

List exact commands executed and outcomes:

```sh
```

