# Security Review Summary

**Date:** 2026-07-15
**Scope:** Full repository security review for medium, high, and critical vulnerabilities with real end-to-end attack paths.

## Result

No new validated medium+ vulnerabilities found.

## Scope of Review

The review covered all Rust source (`src/`), the npm wrapper (`packages/creditlint/`), shell/PowerShell scripts (`scripts/`), and GitHub Actions workflows (`.github/workflows/`).

## Key Observations

- **No shell execution:** All subprocess invocations (`Command::new("git")` in Rust, `spawnSync` in the npm wrapper) use argv arrays, not shell interpreters. No shell injection is possible.
- **No network access during policy evaluation:** The tool is local-first. No webhooks, callbacks, or external fetches exist in the policy path.
- **Attacker-controlled inputs are sandboxed:** Contributor-controlled data (commit messages, author/committer identities, PR title/body) only flows through regex matching and JSON serialization. There are no code execution paths from text content.
- **ReDoS not possible:** The Rust `regex` crate uses linear-time finite automata, not backtracking, so catastrophic backtracking cannot be triggered by attacker-controlled commit text.
- **Config is maintainer-controlled:** `.creditlint.yml` is discovered within the repository root. A malicious config requires repo write access, not an external attack.
- **Fail-closed behavior:** Invalid config, unreadable input, and failed Git metadata collection all exit with code 2, not 0.
- **Secrets handled correctly:** Workflow secrets (`GLM_API_KEY`, `CARGO_REGISTRY_TOKEN`) are consumed as environment variables for their intended purposes and are not logged or serialized into output.
- **`--range` argument to git:** The range string is passed to `git log` as a positional argument without a `--` separator. While this is a minor defensive-coding gap, there is no real end-to-end attack path: CI usage always prefixes the range with `origin/` (e.g., `origin/main..HEAD`), and direct CLI usage means the user is supplying their own range. No PR author or external attacker can inject a dangerous git option through this path.
