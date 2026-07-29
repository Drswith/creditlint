# Security Review Summary

**Date:** 2026-07-29
**Scope:** Full repository review of `creditlint` (Rust CLI + npm wrapper)

## Findings

### 1. Medium — Record separator injection bypasses policy enforcement in git log parser

**Location:** `src/git.rs`

**Description:**

`creditlint audit --all` and `creditlint check --range` collect commit metadata via
`git log --format=%H%x1f%an%x1f%ae%x1f%cn%x1f%ce%x1f%B%x1e`. The `%B` placeholder
emits the raw commit message body, and git permits arbitrary bytes in commit messages
—including the 0x1e (Record Separator) byte used as the record delimiter.

An attacker who can push commits to a repository where creditlint runs in CI can embed
a 0x1e byte in the commit message before a forbidden trailer (e.g.
`Co-authored-by: Codex <codex@example.com>`). The parser in `parse_git_log_stream`
splits the git output on 0x1e, producing a fragment containing only the forbidden
trailer. `parse_git_log_records` then calls `splitn(6, 0x1f)` on that fragment; it
has fewer than 6 fields, so `fields.next()?` returns `None` and `filter_map`
silently discards it. The trailer is never analyzed by the policy engine.

**Proof of concept verified:**

| Scenario | Command | Exit Code | Violations |
|---|---|---|---|
| Normal violating commit | `creditlint audit --all` | 1 | 1 (detected) |
| Same commit with `\x1e` before trailer | `creditlint audit --all` | 0 | 0 (**bypassed**) |
| Same commit with `\x1e` before trailer | `creditlint check --range <sha>..HEAD` | 0 | 0 (**bypassed**) |
| Same commit via stdin | `creditlint check --stdin` | 1 | 1 (detected) |

The `--message-file` and `--stdin` paths are unaffected because they pass content
directly to `policy.analyze` without the 0x1e-delimited parsing step.

**Remediation:**

Use NUL (0x00) as the record/field separator, since git commit messages cannot
contain NUL bytes. Alternatively, retrieve each commit's message individually via
`git show -s --format=%B <sha>`. As defense-in-depth, fail closed if a parsed
record does not contain the expected number of fields rather than silently dropping
it.

## Areas Reviewed (no findings)

- **Shell/command injection:** `Command::new("git")` calls pass arguments as a
  vector, not through a shell. No injection vector found.
- **Config/YAML parsing:** `serde_yaml` deserialization of `.creditlint.yml` fails
  closed on invalid input. Regex patterns from config are compiled with Rust's
  `regex` crate (linear-time, not susceptible to ReDoS).
- **npm wrapper binary resolution:** `CREDITLINT_BIN` env override and
  `require.resolve` platform-package lookup follow standard npm conventions.
  `spawnSync` passes arguments without a shell.
- **File access:** Config discovery walks up from the current directory and is
  bounded by the repo root (`.git` detection). No path traversal.
- **Output encoding:** JSON output uses `serde_json::to_string_pretty` (proper
  escaping). Human output is plain text.
- **CI/release workflows:** Permissions are scoped (`contents: read` by default).
  No untrusted-input injection in workflow steps.
