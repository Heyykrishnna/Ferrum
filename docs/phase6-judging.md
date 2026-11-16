# Phase 6 · Judging Engine

## Overview

The judging engine evaluates contestant submissions against a defined set of test cases, computes partial or full scores, determines individual and aggregate verdicts, and safeguards private test case data.

## Architecture

The judging capability is implemented as a standalone crate `crates/judge` and integrated across the Ferrum execution pipeline:

```text
apps/api -> crates/domain -> crates/infrastructure -> apps/worker -> crates/sandbox -> crates/judge
```

### 1. Comparison Modes

The judging engine provides three comparison modes:

- **Exact (`exact`)**: Byte-for-byte identity check between the program's standard output and expected output.
- **Whitespace-tolerant (`whitespace`)**:
  - Normalizes line endings (`\r\n` to `\n`).
  - Strips trailing whitespace characters from each line.
  - Strips trailing empty lines while preserving non-empty line content and internal whitespace.
- **Token-based (`tokens`)**:
  - Normalizes arbitrary whitespace sequences (spaces, tabs, newlines) into individual tokens, verifying matching token sequences regardless of formatting variations.

### 2. Verdict Precedence

Individual test case verdicts are aggregated into a single submission verdict using strict precedence rules:

1. `CompilationError`: Highest priority. If compilation fails, the code never executes.
2. `InternalError`: Infrastructure, sandbox, or custom checker failures.
3. First non-`Accepted` test failure in test suite order:
   - `TimeLimitExceeded`
   - `MemoryLimitExceeded`
   - `OutputLimitExceeded`
   - `RuntimeError`
   - `WrongAnswer`
4. `Accepted`: Returned only when all test cases succeed with matching outputs.

### 3. Scoring Engine

- Each test case defines an integer `weight` (defaults to 1).
- Total score is the sum of weights of passing test cases.
- Maximum score is the sum of weights of all test cases in the suite.
- Both `score` and `max_score` are persisted and exposed in API responses.

### 4. Privacy and Hidden Cases

- A test case flagged with `hidden: true` protects sensitive grading inputs and outputs from contestants.
- For hidden cases, `CaseFinding::hidden` strips:
  - `input`
  - `expected_output`
  - `stdout`
  - `stderr`
  - `exit_code`
  - `signal`
- The response preserves only non-sensitive grading data: `index`, `verdict`, `score`, `max_score`, `cpu_time_ms`, `wall_time_ms`, and `peak_memory_kib`.

### 5. Custom Checker Abstraction

- Built-in `Checker` trait allows pluggable evaluation logic.
- Differentiates program correctness from checker failures:
  - Checker crash or non-zero exit maps to platform `InternalError`.
  - Clean checker rejection maps to contestant `WrongAnswer`.

### 6. Sandbox Sequential Execution and Artifact Reuse

- For compiled languages (C, C++, Rust, Java):
  - Compilation occurs once inside `/box`.
  - The compiled binary remains in `/box`.
  - For each test case in sequence, only the input file is refreshed, executing the binary without recompilation.
  - If compilation fails, execution stages are skipped, and all test cases record `CompilationError`.

## Database Schema (Migration `0004_judging_results.sql`)

```sql
CREATE TABLE submission_test_cases (
    organization_id uuid NOT NULL,
    submission_id uuid NOT NULL,
    case_index integer NOT NULL,
    input text NOT NULL,
    expected_output text NOT NULL,
    weight integer NOT NULL DEFAULT 1,
    hidden boolean NOT NULL DEFAULT false,
    comparison_mode text NOT NULL DEFAULT 'exact',
    PRIMARY KEY (organization_id, submission_id, case_index),
    FOREIGN KEY (organization_id, submission_id) REFERENCES submissions (organization_id, id)
);

CREATE TABLE submission_case_results (
    attempt_id uuid NOT NULL REFERENCES execution_attempts (id),
    case_index integer NOT NULL,
    verdict text NOT NULL,
    score integer NOT NULL DEFAULT 0,
    max_score integer NOT NULL DEFAULT 1,
    stdout bytea NOT NULL CHECK (octet_length(stdout) <= 65536),
    stderr bytea NOT NULL CHECK (octet_length(stderr) <= 65536),
    exit_code smallint CHECK (exit_code BETWEEN 0 AND 255),
    signal smallint CHECK (signal BETWEEN 0 AND 255),
    cpu_time_ms bigint CHECK (cpu_time_ms >= 0),
    wall_time_ms bigint CHECK (wall_time_ms >= 0),
    peak_memory_kib bigint CHECK (peak_memory_kib >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (attempt_id, case_index)
);

ALTER TABLE execution_results ADD COLUMN score integer;
ALTER TABLE execution_results ADD COLUMN max_score integer;
```

## API Contract

Submissions accept either single-case or multi-case formats:

### Single Expected Output
```json
{
  "language": "python",
  "source": "print(int(input()) * 2)",
  "stdin": "21\n",
  "expected_output": "42\n",
  "comparison_mode": "whitespace"
}
```

### Multi-Case Suite
```json
{
  "language": "cpp",
  "source": "#include <iostream>\nint main() { int x; std::cin >> x; std::cout << x * 2 << std::endl; }",
  "comparison_mode": "exact",
  "tests": [
    {
      "input": "5\n",
      "expected_output": "10\n",
      "weight": 50,
      "hidden": false
    },
    {
      "input": "100\n",
      "expected_output": "200\n",
      "weight": 50,
      "hidden": true
    }
  ]
}
```

### Result Response
```json
{
  "id": "c1f7a07c-9b87-43f1-b997-8c3dc523363f",
  "state": "completed",
  "verdict": "accepted",
  "result": {
    "outcome": "success",
    "stdout": "10\n",
    "stderr": "",
    "score": 100,
    "max_score": 100,
    "cases": [
      {
        "index": 1,
        "verdict": "accepted",
        "score": 50,
        "max_score": 50,
        "input": "5\n",
        "expected_output": "10\n",
        "stdout": "10\n",
        "stderr": "",
        "exit_code": 0,
        "cpu_time_ms": 12,
        "wall_time_ms": 18,
        "peak_memory_kib": 1024
      },
      {
        "index": 2,
        "verdict": "accepted",
        "score": 50,
        "max_score": 50,
        "cpu_time_ms": 11,
        "wall_time_ms": 17,
        "peak_memory_kib": 1024
      }
    ]
  }
}
```

## Verification

### Unit & Workspace Tests
```bash
cargo test --workspace
cargo clippy --all-targets --workspace -- -D warnings
cargo fmt --all --check
```

### Smoke Tests with Database Double
```bash
python3 tests/phase6/judging-smoke.py
```

### Live Isolate Verification on Debian 13 amd64 Host
```bash
python3 tests/phase6/judging-smoke.py --isolate
```
