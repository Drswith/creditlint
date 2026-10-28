# Security Review Summary

**Date:** 2026-10-28
**Scope:** Full repository review of `creditlint` (Rust CLI + npm wrapper + shell scripts + GitHub Actions workflows)

## Result

No new validated medium+ vulnerabilities found.

## Attack Surfaces Reviewed

1. **Git command execution** (`src/git.rs`) — `Command::new("git").args(...)` uses argument passing, not a shell, so no command injection. The `--range` argument is user-controlled via CLI flag, not from untrusted external input.

2. **Regex policy matching** (`src/policy/mod.rs`) — Uses the Rust `regex` crate, which is a linear-time NFA engine immune to catastrophic backtracking (ReDoS). Patterns are config-trusted; commit message content is only matched via `is_match`, never executed.

3. **Config loading** (`src/config.rs`) — YAML parsing fails closed on errors with exit code 2. Regex patterns are validated at load time. Config discovery only walks up to the repo root (found via `.git`).

4. **npm wrapper** (`packages/creditlint/bin/creditlint.js`) — `CREDITLINT_BIN` env override is a documented, user-controlled feature. Binary is spawned via `spawnSync` (no shell). Module resolution follows standard Node.js patterns with platform-determined package names.

5. **Hook installation** (`src/cli/mod.rs`) — Hook path comes from `git rev-parse --git-path`. Hook contents are hardcoded. Unmanaged existing hooks are refused overwrite.

6. **GitHub ruleset pattern export** (`src/github.rs`) — Constructs a regex string from config-supplied patterns. Output is printed to stdout for manual use. `normalize_pattern` rejects inline regex flags beyond a leading `(?i)`. Config is trusted.

7. **Shell scripts** (`scripts/`) — Use `set -euo pipefail` and proper quoting. The inline Node script in `bootstrap-npm-trust-packages.sh` sources `repoRoot` from `BASH_SOURCE` (trusted path).

8. **GitHub Actions workflows** — Checkouts use `persist-credentials: false`. No `pull_request_target` with write permissions. The issue-triage workflow gates on account age (>= 30 days). The security-review and bug-scan workflows run on schedule/dispatch only.

## Why No Findings

The codebase is a local-first CLI tool with no network services, no SQL, no auth flows, no webhooks, and no templating. The only external command (`git`) is invoked safely via `Command::new` argument passing. Commit message content never influences control flow beyond regex `is_match` calls. Config input fails closed on invalid YAML or regex. The npm wrapper delegates to a native binary via `spawnSync` without shell interpolation.
