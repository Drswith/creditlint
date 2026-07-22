# Security Review Summary

**Date:** 2026-07-22
**Scope:** Full repository review for validated medium, high, or critical vulnerabilities with a real end-to-end attack path.

## Result

No new validated medium+ vulnerabilities found.

## Review Coverage

### Trust Boundaries
- **External input:** Git commit message text and author/committer identity (parsed but never executed, evaluated only by the Rust `regex` crate which provides linear-time guarantees).
- **Trusted input:** Repo-local `.creditlint.yml` policy config, CLI flags from the developer/CI operator, and `CREDITLINT_BIN` env override.
- **No network surface:** The CLI is local-first with no HTTP/RPC/webhook handlers.

### Attack Surfaces Examined
1. **Shell execution (Rust):** `src/git.rs` invokes `git` via `Command::new("git").args([...])` using arg arrays, not a shell. The user-supplied `--range` is passed as a single revision arg to `git log` — no shell injection, and the supplier is the CLI operator (not an external attacker).
2. **Process spawning (npm wrapper):** `packages/creditlint/bin/creditlint.js` uses `spawnSync(path, argvArray, {stdio:"inherit"})` with no shell. Binary path is resolved through a fixed candidate list (`CREDITLINT_BIN`, platform package, `native/`, `target/{release,debug}/`); no attacker-controllable path component.
3. **File access:** All paths in `src/cli/mod.rs` and `src/config.rs` are derived from `env::current_dir()`, repo-root walking (`.git` detection), `git rev-parse --git-path`, or an explicit user-supplied `--message-file` flag. No path traversal from commit/PR content.
4. **Config parsing:** `.creditlint.yml` is repo-local and trusted. `serde_yaml` is safe from deserialization RCE. Regex patterns are validated at load time; raw content is never echoed back on parse errors.
5. **Policy engine:** `src/policy/mod.rs` only string-matches commit text against compiled regexes. Default patterns are simple alternations/anchored literals with no catastrophic backtracking. The Rust `regex` crate enforces linear time regardless.
6. **GitHub Actions:** No `github.event.*` interpolation in any `run:` step. The only `${{ }}` interpolations in `run:` blocks reference hardcoded `matrix.*` values. `github.ref` appears only in `concurrency.group` and `if:` conditionals.
7. **Hook installation:** `install-hook` writes a static, quoted shell snippet to the path returned by `git rev-parse --git-path hooks/commit-msg`. The managed-hook overwrite check requires both a marker and version string, and overwrites with safe static content.
8. **Maintainer scripts:** `scripts/*.sh` use `set -euo pipefail` and pass user-supplied `--registry`/`--dist-dir` values as separate array elements to `npm`/`pnpm` — no shell injection.

### Dependencies Reviewed
- `clap`, `regex`, `serde`, `serde_json`, `serde_yaml`, `thiserror` — no known RCE advisories matching an end-to-end attack path. `serde_yaml` is unmaintained (RUSTSEC-2024-0320) but this is a low-signal maintenance note, not an exploitable vulnerability, and is out of scope per the reporting bar.

## Artifacts

No `security-findings.json` was written because no validated findings met the medium+ severity bar with a real end-to-end attack path.
