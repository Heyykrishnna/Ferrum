# Phase 5 · Language runtimes

**Prerequisite** · The proven Python pipeline. [Phase 4](04-apalis-worker-pipeline.md)

## Outcome

Support Python, C++, JavaScript through Node.js, C, Java and Rust through one validated language registry. Use C++ first to prove compilation and execution within Isolate.

## Work plan

1. Define a registry entry with a stable language ID, pinned toolchain identity, source and entry-point conventions, compile/run argument vectors, and policy ceilings for each stage. Persist the resolved profile identity so queued submissions keep the accepted runtime contract.
2. Move the original Python profile into the registry and run its existing acceptance cases. Add the remaining profiles in the order below using trusted executables.
3. Separate compilation and execution findings in the result. A compiler error returns bounded diagnostics without attempting to run an artifact. Keep compiler output, generated artifacts, CPU and wall time under policy. Runtime dependency installation and network access stay outside the submitted program's capabilities.
4. After the single-file matrix passes, introduce bounded multi-file inputs with validated normalized paths, duplicate detection, and per-file and total size checks. Validate the full file set before staging and keep all artifacts inside the attempt workspace.

| Order | Profile | Integration focus |
|---|---|---|
| 1 | Python 3 | Preserve the interpreter-only path and stdin/stdout behavior |
| 2 | C++ | Compile and execute the generated binary with separate stage diagnostics |
| 3 | Node.js | Use the trusted entry point and validate interpreted runtime failures |
| 4 | C | Reuse native compile/run orchestration with its own trusted profile |
| 5 | Java | Resolve the class entry point and measure JVM memory behavior on the target host |
| 6 | Rust | Compile a bounded submission with the pinned toolchain and run the resulting binary |

## Acceptance checks

- [ ] Each advertised profile passes execution, stdin/stdout, runtime error and resource-limit tests on supported Linux.
- [ ] Each compiled profile handles compiler failure and timeout and skips run when compilation fails.
- [ ] Executables and arguments resolve from trusted configuration, and compile/run remain inside Isolate.
- [ ] If multi-file support ships, invalid paths and oversized or duplicate files are rejected before staging.

**Handoff** · Expose validated execution facts and language profiles to judging. [Phase 6](06-judging-engine.md)
