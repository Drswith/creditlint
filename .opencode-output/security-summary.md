# Security Review Summary

**Date:** 2026-08-12
**Reviewer:** opencode scheduled application-security reviewer (glm/glm-5.2)
**Scope:** Full repository — Rust CLI, npm wrapper, scripts, GitHub Actions workflows

## Outcome

No new validated medium+ vulnerabilities found.

## What was reviewed

- **Rust CLI (`src/`)**: `cli/mod.rs`, `git.rs`, `config.rs`, `policy/mod.rs`, `github.rs`, `reporter.rs`, `lib.rs`, `main.rs`
- **npm wrapper**: `packages/creditlint/bin/creditlint.js` and its test suite
- **Maintainer scripts**: `scripts/bootstrap-npm-trust-packages.sh`, `scripts/publish-npm-packages.sh`, `scripts/smoke-release-binary.{sh,ps1}`
- **GitHub Actions**: `ci.yml`, `release.yml`, and the opencode review/security/triage workflows

## Attack surfaces examined

1. **Git process invocation (`src/git.rs`)** — `Command::new("git")` with discrete arg arrays; no shell interpolation. The `--range` value reaches `git log` as a single positional/option arg and is never user-controlled in any committed workflow.
2. **Policy regex matching (`src/policy/mod.rs`)** — Uses the Rust `regex` crate, which is linear-time and not vulnerable to catastrophic backtracking (ReDoS). Config-supplied patterns are validated at load time and fail closed.
3. **Config parsing (`src/config.rs`)** — `serde_yaml` deserialization with explicit version check and regex validation; invalid config/regex returns exit code 2.
4. **npm binary resolution (`packages/creditlint/bin/creditlint.js`)** — `CREDITLINT_BIN` and `target/{release,debug}` fallbacks require pre-existing local write access to the victim filesystem, which is outside the trust boundary for a local-first CLI. `spawnSync` uses arg arrays.
5. **Hook installation (`src/cli/mod.rs`)** — Managed-hook overwrite guard uses content marker matching; no TOCTOU-relevant privileged write.
6. **Output rendering (`src/reporter.rs`)** — `serde_json::to_string_pretty` and plain-text formatting; no eval, templating, or shell sinks.
7. **Maintainer scripts and release workflow** — Paths are quoted; GitHub Actions use `persist-credentials: false`, scoped permissions, and no `github.event.*` interpolation into shell commands.

## Why nothing met the reporting bar

Every candidate concern collapsed into one of the excluded categories defined by the reporting instructions:

- **Self-attack / pre-existing privilege**: npm wrapper fallback paths (`CREDITLINT_BIN`, `target/release`, `target/debug`) and config-discovery walk-up require the attacker to already hold local write access to the user's filesystem or git config.
- **Isolated unsafe-looking API without a real attack path**: `Command::new("git")` arg passing and `spawnSync` with arg arrays are not shell-injectable.
- **Low-signal best-practice notes**: e.g., optional-dependency resolution ordering, hook marker string matching — no concrete end-to-end impact.

No `unsafe`, `panic!`, `todo!`, or `unimplemented!` appears in production code paths; all `.expect()` calls are confined to tests.

## Artifacts

- No `.opencode-output/security-findings.json` was written because no validated findings exist.
- This summary is the only artifact produced.
