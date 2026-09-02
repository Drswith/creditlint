# Security Review Summary

**Date:** 2026-09-02
**Scope:** `creditlint` Rust CLI, npm wrapper, GitHub Actions workflows, and helper scripts.

## Result

**1 validated medium-severity finding.**

## Validated Finding

### Git log record separator injection bypasses creditlint policy enforcement — Medium

**Location:** `src/git.rs`

**Root cause:** `collect_range_messages` and `collect_all_messages` invoke
`git log --format=%H%x1f%an%x1f%ae%x1f%cn%x1f%ce%x1f%B%x1e` and parse the output
in `parse_git_log_stream` by splitting on ASCII `\x1e` (Record Separator) and
`\x1f` (Field Separator). Both characters are legal inside git commit message
bodies — git only forbids NUL (`\x00`). An attacker-controlled commit message
that contains `\x1e` therefore injects a fake record boundary.

**Attack path:**

1. Attacker is any PR contributor on a repo that runs `creditlint check --range`
   (or `creditlint audit --all`) as a CI check.
2. They craft a commit message such as:
   `clean subject\n\nbody\x1eCo-authored-by: Codex <codex@example.com>\n`.
3. The injected `\x1e` splits the commit's git-log record into two chunks. The
   first chunk's message field is truncated to `body`. The text after the
   separator (`Co-authored-by: Codex …`) is treated as a new record but has no
   `\x1f` field separators, so `splitn(6, …)` yields only one field;
   `fields.next()?` returns `None` and `filter_map` silently drops the chunk.
4. The forbidden trailer never reaches `Policy::analyze`. creditlint exits 0.

**Verified end-to-end:**

| Commit message | creditlint exit | violations |
|---|---|---|
| `clean subject\n\nbody\x1eCo-authored-by: Codex <codex@…>` | **0** | `{"ok":true,"violations":[]}` |
| `clean subject\n\nbody\nCo-authored-by: Codex <codex@…>` (control) | 1 | `forbidden-ai-coauthor` |

The same technique hides any freeform marker (`Made with Cursor`, `Generated with
Claude`, `Author: Cursor Agent`). The `--message-file` and `--stdin` paths are
unaffected (they feed text directly to `Policy::analyze`); only `--range` and
`audit --all` — the CI/audit modes — are vulnerable.

**Impact:** A PR contributor can sneak any AI/tool authorship marker past
creditlint's automated CI gate, allowing it to land permanently in protected
branch history. The injected `\x1e` is a non-printable control character that
renders as nothing or an inconspicuous symbol in git-log UIs and GitHub diffs,
so human review provides weak detection.

**Remediation:** Switch to NUL-separated output (`%x00` in the `--format`
string, or `git log -z`), since git commit objects cannot contain NUL. As
defense-in-depth, validate that each parsed record's first field matches a real
git SHA before accepting the record.

## Areas Reviewed — No Validated Findings

- **CLI input handling** (`src/cli/mod.rs`): `--message-file` uses
  `fs::read_to_string`; `--range` is passed as a single argv element to
  `Command::new("git")` (no shell). No path traversal or command injection.
- **Config loading** (`src/config.rs`): per-repo `.creditlint.yml` is loaded
  from the working directory. A PR that edits the config to disable rules is a
  deployment/trust-boundary concern (config change is visible in the PR diff)
  rather than a code vulnerability; not reported as it does not clear the
  medium+ bar.
- **Regex evaluation** (`src/policy/mod.rs`): uses the Rust `regex` crate which
  has linear-time guarantees — no ReDoS.
- **Ruleset export** (`src/github.rs`): pure string construction, no eval.
- **npm wrapper** (`packages/creditlint/bin/creditlint.js`): `CREDITLINT_BIN`
  override and platform-package resolution use `spawnSync` without `shell: true`
  — no shell injection. Environment-controlled binary execution is intended
  behavior for the wrapper.
- **GitHub Actions workflows** (`.github/workflows/`): no `github.event.*`
  interpolations in `run:` steps; `permissions:` are least-privilege; secrets
  are referenced via `${{ secrets.* }}`.
- **Shell scripts** (`scripts/*.sh`): `set -euo pipefail`, proper quoting, no
  eval of attacker input.
- **Identity-field `\x1f` injection**: tested — git strips control characters
  from author/committer name/email fields, so this variant is not exploitable.
