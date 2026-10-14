# Security Review Summary

No new validated medium+ vulnerabilities found.

## Scope

Reviewed all Rust source (`src/`), npm wrapper (`packages/creditlint/bin/creditlint.js`),
shell scripts (`scripts/`), and GitHub Actions workflows (`.github/workflows/`).

## Key Attack Surfaces Assessed

- **Git command execution** (`src/git.rs`): Uses `Command::new("git")` with array
  arguments. No shell is invoked, so the `--range` CLI input cannot cause command
  injection. Not exploitable.

- **Config-supplied regex** (`src/config.rs`, `src/policy/mod.rs`): Patterns from
  `.creditlint.yml` are compiled with the Rust `regex` crate, which provides
  linear-time matching guarantees. No ReDoS / catastrophic backtracking possible.
  Invalid patterns fail closed at config load time.

- **npm wrapper binary resolution** (`packages/creditlint/bin/creditlint.js`):
  `spawnSync` is used without `shell: true`. The `CREDITLINT_BIN` env var is a
  developer override (analogous to `RUSTC`/`CARGO`); an attacker who can set
  environment variables already has code execution. Not a vulnerability.

- **GitHub Actions workflows**: No `pull_request_target` trigger is used anywhere,
  so secrets are never exposed to untrusted PR code. Permissions are scoped to
  `contents: read` by default; `contents: write` and `pull-requests: write` only
  appear on scheduled/manual-dispatch workflows. `persist-credentials: false` is
  set on checkout steps.

- **Shell scripts**: All variables are properly quoted; `npm publish` invocations
  use array-based arguments. No `eval` or unsafe unquoted expansion found.

- **Dependencies**: Pinned versions (`clap 4.5.53`, `regex 1.12.2`, `serde 1.0.228`,
  `serde_json 1.0.149`, `serde_yaml 0.9.34`, `thiserror 2.0.17`) have no known
  CVEs exploitable via the attack paths present in this codebase.