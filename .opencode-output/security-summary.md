# Security Review Summary

No new validated medium+ vulnerabilities found.

## Scope

Scheduled application-security review of the `creditlint` repository. Reviewed
the Rust native CLI (`src/`), the optional npm wrapper
(`packages/creditlint/`), release/publish bash scripts (`scripts/`), and all
GitHub Actions workflows (`.github/workflows/`).

## Surfaces traced and cleared

- **Git subprocess invocation (`src/git.rs`)**: `Command::new("git").args([...])`
  passes argv items directly without a shell. The `--range` value is a single
  argv item following `--format=...`, so neither shell injection nor flag
  injection is reachable. The documented CI usage prefixes the range with
  `origin/`, blocking leading-dash option injection, and GitHub branch-name
  charset restrictions prevent metacharacter injection through shell
  substitution. The invoking user is the trust principal.
- **Config deserialization (`src/config.rs`)**: `serde_yaml` parses
  `.creditlint.yml`; all regex patterns are validated with `Regex::new` and
  fail closed on error. The Rust `regex` crate is linear-time (no ReDoS).
- **Policy engine and ruleset export (`src/policy/mod.rs`, `src/github.rs`)**:
  pure data processing and string manipulation; no execution paths.
- **Hook installer (`src/cli/mod.rs`)**: writes a static hook string, refuses
  to overwrite unmanaged hooks, verifies the managed marker.
- **npm wrapper (`packages/creditlint/bin/creditlint.js`)**: `spawnSync` of a
  resolved binary path. `CREDITLINT_BIN` is an intended env override; local
  fallback paths live inside the install root, so planting there requires
  pre-existing write access of equivalent privilege.
- **GitHub Actions workflows**: all use `persist-credentials: false`; no
  `pull_request_target`; PR-triggered workflow uses `pull_request` with
  `contents: read`; job-scoped least-privilege permissions; no dangerous
  `${{ github.event.* }}` interpolation into shell commands.
- **Bash scripts (`scripts/*.sh`)**: `set -euo pipefail`, separate argv items
  passed to `npm publish`, no `eval`, no unquoted expansion into shell.

## Outcome

No findings written to `security-findings.json` because no validated medium,
high, or critical vulnerability with a real end-to-end attack path was
identified.
