# Phase 5 language runtimes

The single-file registry is compiled into the API and worker. The API exposes the configured entries at authenticated `GET /v1/languages` and persists the accepted profile revision with the input and runtime limits. The worker reconstructs the same revision and revalidates its limits before staging. Toolchain patches within the pinned Debian version families are supported. The worker verifies their versions before accepting queue work.

| ID | Language slug | Profile | Source | Toolchain family |
|---|---|---|---|---|
| 1 | `python` | `python3.13-v1` | `main.py` | Debian Python 3.13 |
| 2 | `cpp` | `gcc14-cpp20-v1` | `main.cpp` | GCC 14.2, C++20 |
| 3 | `javascript` | `node20.19-v1` | `main.cjs` | Node.js 20.19 |
| 4 | `c` | `gcc14-c17-v1` | `main.c` | GCC 14.2, C17 |
| 5 | `java` | `openjdk21-v1` | `Main.java` | OpenJDK 21, class `Main` |
| 6 | `rust` | `rust1.85-v1` | `main.rs` | Debian rustc 1.85, Rust 2024 |

Isolate compiles and runs submitted code with fixed executable paths and argument vectors. A compiler gets its own CPU, wall, memory, file and process limits. The compiled artifact stays in the box and is checked before execution. The native runtime uses a read-only executable `/box` mount. Interpreted runtimes and compilers use a writable `noexec` workspace. Compiler and runtime output are each bounded to 64 KiB per stream. User stdin is staged after compilation. The registry supports standard libraries and single-file source.

| Stage | CPU / wall ceiling | Memory ceiling |
|---|---|---:|
| Python, C, C++ and Rust runtime | 2 s / 6 s | 128 MiB |
| Node.js and JVM runtime | 3 s / 6 s | 256 MiB |
| C, C++ and Java compilation | 8 s / 12 s | 512 MiB |
| Rust compilation | 8 s / 12 s | 768 MiB |

The compiler may write files up to 8 MiB each within a 32 MiB workspace quota. The worker holds one slot throughout compilation and execution, including cleanup. Successful compilation is followed by a fresh Isolate invocation in the same box after the cgroup is empty. Isolate 2.7 reports cumulative cgroup memory peak across invocations, so Ferrum reports the compiler peak and leaves the runtime peak null for compiled submissions. Interpreted profiles still report their runtime peak. [Isolate manual](https://github.com/ioi/isolate/blob/v2.7/isolate.1.txt)

## Verify in Docker

From the Ferrum checkout on macOS or Linux:

```sh
docker build --platform linux/amd64 --target language-toolchains \
  -f deployment/phase3.Dockerfile -t ferrum-phase5-checks .
docker run --rm --platform linux/amd64 --network none --cap-drop ALL \
  --security-opt no-new-privileges ferrum-phase5-checks
python3 -I -u tests/phase4/worker-smoke.py
```

The image checks fixed toolchain commands with trusted fixtures. The PostgreSQL suite checks durable compiler diagnostics, verdicts and profile revalidation. Docker Desktop uses its own kernel. Isolate acceptance uses the dedicated worker profile below.

## Qualify the Linux worker

Start with a [Phase 3-qualified Debian 13 amd64 host](phase3-isolate.md#qualify-the-adapter). Install the toolchain packages from Debian 13:

```sh
sudo apt-get update
sudo apt-get install --no-install-recommends python3 gcc-14 g++-14 nodejs openjdk-21-jdk-headless rustc
```

Use the ordinary worker account and keep box 0 reserved during verification. With Docker Compose available for the disposable PostgreSQL smoke test, run from the Ferrum checkout:

```sh
cargo test --locked -p ferrum-sandbox --test language_host -- --ignored --nocapture
cargo test --locked -p ferrum-sandbox --lib compiler_deadlines_skip_run_for_every_compiled_profile -- --ignored --nocapture
python3 -I -u tests/phase4/worker-smoke.py --isolate
```

The first command exercises all six profiles through Isolate. The second uses short compiler deadlines on trusted fixtures and checks that the execution stage is skipped. The final command checks the API, PostgreSQL, Apalis and real Isolate together for Python and C++, including durable compiler diagnostics and results after API restart. Record the revision and outcomes in the [Phase 5 checklist](../phases/05-language-runtimes.md#acceptance-checks).

For an existing Ferrum database, apply `cargo run --locked -p ferrum-api -- migrate` with its usual `FERRUM_DATABASE_URL` before starting the updated API or worker. Upgrade API and workers together.
