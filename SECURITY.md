# Security Policy

## Scope

Ferrum executes untrusted user-submitted code. Security reports affecting any of the following boundaries are treated as high priority:

- Sandbox escape, privilege escalation, cgroup/namespace bypass, or residual process/filesystem leaks in `crates/sandbox`
- Compiler/runtime argument injection, path traversal, or symlink exploitation
- Tenant isolation bypass, API key verification flaws, or credential/secret disclosure
- Webhook SSRF bypasses or unsafe egress handling
- Supply-chain vulnerabilities in Rust (`Cargo.lock`) or web (`web/package-lock.json`) dependencies

## Reporting a Vulnerability

Do not open public GitHub issues for suspected security vulnerabilities.

Use GitHub Security Advisories (Private Vulnerability Reporting) on this repository to submit:

1. Affected component, commit SHA or release tag, and host environment (kernel, Isolate version, architecture)
2. Minimal reproduction steps or proof-of-concept payload
3. Expected versus observed isolation or authorization behavior

## Security Verification

Automated checks run continuously in GitHub Actions:

- Static analysis via CodeQL (`rust`, `javascript-typescript`)
- Rust dependency advisories, license, ban, and source verification via `cargo-deny` and RustSec (`cargo-audit`)
- Web dependency auditing via `npm audit`
- Containerized Linux sandbox and toolchain checks plus dedicated Debian 13 Isolate 2.7 host verification
- SLSA build provenance and CycloneDX/SPDX SBOM attestations on release artifacts

