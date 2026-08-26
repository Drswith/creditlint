# Security Review Summary

**Date:** 2026-08-26
**Reviewer:** Scheduled application-security review (opencode-security-review workflow)
**Scope:** Full repository review of `creditlint` (Rust CLI + npm wrapper + GitHub Actions)

## Result

No new validated medium+ vulnerabilities found.

## Attack Surfaces Reviewed

### 1. Process/Command Execution (`src/git.rs`)
- `git` is invoked via `Command::new("git").args([...])` (no shell). The user-controlled `--range` value is passed as a discrete argument, not interpolated into a shell string. No shell injection path exists.
- Hook path resolution uses `git rev-parse --git-path` with hardcoded arguments.

### 2. File System Access (`src/cli/mod.rs`, `src/config.rs`)
- `--message-file` reads a caller-specified path; the caller already holds filesystem access, so no privilege escalation.
- Config discovery walks from `start_dir` to repo root. Config files only define regex patterns for matching — no code execution, no arbitrary file writes.
- `install-hook` writes hardcoded hook contents and refuses to overwrite unmanaged hooks.
- `init` refuses to overwrite existing config.

### 3. Config & Regex Handling (`src/config.rs`, `src/policy/mod.rs`)
- YAML is deserialized into typed structs; no arbitrary code paths from config.
- Regex patterns use the Rust `regex` crate (linear-time matching guarantees); invalid patterns fail closed.
- Invalid config, unreadable input, and failed git collection all produce errors (exit code 2), preserving fail-closed behavior.

### 4. npm Wrapper Binary Resolution (`packages/creditlint/bin/creditlint.js`)
- `CREDITLINT_BIN` env override requires environment control (equivalent to existing code execution).
- Platform package binary is resolved before cargo fallbacks; cargo fallbacks require filesystem write access and are intentional for local development.
- `spawnSync` forwards `process.argv.slice(2)` without shell interpretation.

### 5. GitHub Actions Workflows (`.github/workflows/*.yml`)
- All `${{ }}` expressions in `run:` steps use hardcoded `matrix.*` values, not user-controlled input (verified: `release.yml` lines 127, 261, 266).
- `opencode-review.yml` uses `github.event.pull_request.number` only in a `concurrency.group` string (numeric, not in a `run:` step).
- All workflows use `persist-credentials: false` and minimal scoped permissions.
- `opencode-triage.yml` gates on 30-day account age and uses `user.login` in a REST API call, not shell.

### 6. Shell Scripts (`scripts/*.sh`, `scripts/*.ps1`)
- All shell scripts use `set -euo pipefail` and properly quote variable expansions.
- `--registry` and `--dist-dir` arguments are passed as quoted array elements to `npm publish`.
- No `eval`, no unquoted expansions in command positions.

### 7. Network & Secrets
- No HTTP clients, webhooks, SQL databases, or templating engines in the Rust codebase.
- No secrets are logged or exposed; workflow secrets are referenced via `${{ secrets.* }}` only.
