# Security Review Summary

**Date:** 2026-09-23

## Findings

Found **1** validated medium+ vulnerability.

### 1. Record separator injection in git log parser enables complete policy bypass

- **Severity:** medium
- **Status:** active
- **Location:** `src/git.rs`

**Attack path:**

An attacker with push access to a branch (e.g., an external contributor opening a pull request) creates a git commit whose author name, committer name, author email, or committer email contains a raw 0x1e byte (ASCII Record Separator). Git stores arbitrary bytes in commit identity fields and git-log --format=%an (or %ae, %cn, %ce) emits them verbatim. creditlint's collect_range_messages and collect_all_messages invoke 'git log --format=%H%x1f%an%x1f%ae%x1f%cn%x1f%ce%x1f%B%x1e' and then parse_git_log_stream splits the output on 0x1e to delimit commit records. The injected 0x1e inside an identity field fragments the record into pieces that each have fewer than 6 fields; parse_git_log_records uses splitn(6, 0x1f) and returns None from fields.next()? for any fragment with fewer than 6 fields, silently dropping it via filter_map. The entire commit record vanishes, so neither the identity rules (forbidden-ai-git-identity) nor the trailer rules (forbidden-ai-coauthor, made-with, etc.) are evaluated against it. Verified: a commit authored by 'Codex' with email 'codex@evil.com' and a 'Co-authored-by: Codex' trailer passes 'creditlint audit --all' with exit code 0 and zero violations when a single 0x1e byte is embedded in any identity field (attacker name, email, committer name, or committer email). The same commit without the injected byte is correctly flagged. Both 'audit --all' and 'check --range HEAD~1..HEAD' are affected.

**Impact:**

Complete bypass of creditlint's core authorship policy enforcement. An attacker can submit AI-generated commits (with forbidden AI tool identities and attribution trailers) that pass creditlint checks undetected in CI, pre-commit hooks, or merge-bot validation. The control character is invisible in most terminals and editors, making the attack stealthy. This defeats the sole purpose of the tool.

**Remediation:**

Sanitize or reject control characters in commit identity fields before parsing. Options: (1) use a delimiter that cannot appear in git identity fields, e.g. NUL-terminated records with git-log -z; (2) reject commits whose author/committer name or email contains 0x1e or 0x1f bytes and report them as violations (fail-closed); (3) pass the format separators through a git sanitize option. At minimum, validate that each parsed record has exactly 6 fields and treat records that fragment due to embedded separators as violations rather than silently dropping them.

## Areas Reviewed

- Rust source: CLI, config parser, policy engine, git integration, reporter, github ruleset export
- npm wrapper package: binary resolution, spawn execution, no lifecycle scripts
- Shell scripts: publish/bootstrap/smoke-test scripts
- GitHub Actions workflows: ci.yml, release.yml, opencode-* workflows
- Checked for: shell injection, script injection in workflows, unsafe Rust, path traversal, ReDoS, secrets handling, network fetches, supply chain issues

No other validated medium+ vulnerabilities found.
