# Security Review Summary

**Date:** 2026-11-18
**Scope:** Full repository review of `creditlint` (Rust CLI + npm wrapper + GitHub workflows + shell scripts)

## Findings

### 1. Commit message record-separator injection bypasses audit and range checks — Medium

**Location:** `src/git.rs`

**What:** `collect_all_messages()` and `collect_range_messages()` invoke `git log` with a custom `--format` that uses `0x1e` (RECORD SEPARATOR) between commits and `0x1f` (FIELD SEPARATOR) between fields. The `%B` placeholder emits the raw commit body, including any control bytes the contributor embedded. `parse_git_log_stream()` splits the stdout on `0x1e`, so a contributor who places a literal `0x1e` byte in their commit message before a forbidden trailer (e.g. `Co-authored-by: Codex`) causes the record to split early. The first chunk becomes a valid-looking record with a truncated clean message; the second chunk (containing the forbidden trailer) is discarded because it has fewer than 6 fields. The result: `creditlint audit --all` and `creditlint check --range` report `ok: true` with zero violations for that commit.

**Who / how:** Any contributor with push or PR access controls the raw bytes of their commit message. They reach the vulnerable code when CI, a scheduled audit, or merge-bot validation runs `creditlint audit --all` or `creditlint check --range <range>`.

**Verified PoC:**
- Commit message `feat: add feature\x1e\n\nCo-authored-by: Codex <codex@example.com>` → `creditlint audit --all` returns `{"ok": true, "violations": []}` (exit 0).
- Same message without the `\x1e` → correctly flagged with 1 violation (exit 1).
- `--stdin` and `--message-file` paths are NOT affected (they read the message directly without record-separator splitting).

**Impact:** The tool's core detection capability is silently bypassed on the primary CI/audit paths. Unauthorized authorship and credit markers hidden after a `0x1e` byte enter the repository undetected.

**Remediation:** Use `git log -z` with NUL-byte separators (NUL cannot appear in git object content) instead of `0x1f`/`0x1e`. Alternatively, reject records whose message field contains `0x1e`/`0x1f` bytes or fail-closed on field-count mismatch.

## Surfaces reviewed with no validated findings

- **npm wrapper** (`packages/creditlint/bin/creditlint.js`): `CREDITLINT_BIN` is a local environment variable; `spawnSync` passes `process.argv.slice(2)` without a shell — no injection.
- **Shell scripts** (`scripts/*.sh`): local dev/CI scripts with proper quoting; no untrusted input reaches command construction.
- **GitHub workflows** (`.github/workflows/*.yml`): matrix values used in `run:` steps are hardcoded in the workflow file, not derived from PR/issue content; `persist-credentials: false` is set on checkouts.
- **Config parsing** (`src/config.rs`): regex patterns are owner-controlled (repo `.creditlint.yml`); fail-closed on invalid regex/parse. Default policy patterns are simple alternations without ReDoS risk.
- **GitHub ruleset export** (`src/github.rs`): output is regex-escaped via `regex::escape`; inline flag injection is rejected.
