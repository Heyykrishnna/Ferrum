# Phase 6 · Judging engine

**Prerequisite** · The verified language matrix and bounded execution results. [Phase 5](05-language-runtimes.md)

## Outcome

Score a submitted program against a versioned set of tests. Keep per-case findings, final program verdict and platform processing state independently readable.

## Work plan

1. Add test-suite data with stable case order, input, expected output, visibility, weight and effective limits. Persist the suite version used by the submission and the per-case findings that produced its outcome.
2. Implement exact byte comparison first. Define whitespace handling with examples for line endings, trailing space and final newlines before enabling a second mode. Capture raw execution facts within policy for diagnosis.
3. Orchestrate cases sequentially initially. For compiled profiles, define how a built artifact is safely reused across cases within the attempt. Keep case inputs, output and mutable workspace state separate so one case cannot change another's result.
4. Define aggregate verdict precedence, partial scoring, skipped cases, and cancellation. Compare output only after a valid run. Map wrong answer and program errors to program outcomes and checker or infrastructure faults to their platform category. [State and verdict](submission-lifecycle.md)
5. Limit response views and telemetry so hidden inputs, expected output and answer-revealing diagnostics remain private. Test visibility through the public result mapping.
6. Add custom checkers behind their own sandbox policy after deterministic built-in comparison is proven. Give the checker access only to the files it needs and report checker failure separately from a contestant's wrong answer.

## Acceptance checks

- [ ] Fixed fixtures reproduce exact-match and whitespace-mode behavior as documented.
- [ ] Correct, incorrect, compilation-failing, runtime-failing and resource-limited submissions produce the expected findings.
- [ ] A mixed test suite returns repeatable case ordering, verdict and score under the same policy.
- [ ] Cancellation stops remaining work and partial results follow the documented contract.
- [ ] Hidden cases stay private. Any enabled custom checker passes its own sandbox and failure tests.

**Handoff** · Publish safe polling contracts and event types for external integrations. [Phase 7](07-production-api.md)
