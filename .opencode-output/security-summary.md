# Security Review Summary

**Date:** 2026-10-21
**Reviewer:** Scheduled opencode security review (glm-5.2)
**Scope:** `creditlint` repository at `ed513f7` (full tree: Rust CLI, npm wrapper, platform packages, GitHub Actions workflows, shell scripts)

## Result

No new validated medium+ vulnerabilities found.

## Coverage

The review examined the following trust boundaries and attack surfaces:

### Native Rust CLI (`src/`)
- **Entry points:** `check --message-file`, `check --stdin`, `check --range`, `audit --all`, `init`, `install-hook`, `github ruleset-pattern`.
- **Shell execution:** `Command::new("git")` is invoked with argument arrays (`args(["log","--format=...", range])`). No `sh -c`, no string interpolation. The `--range` value is passed as a single argv element — argument injection into `git` is not a privilege boundary crossing because the operator already runs the CLI with their own privileges.
- **File access:** `--message-file` reads operator-specified paths; config discovery walks from CWD up to the `.git` root. No symlink hardening is required since the operator controls these paths and the tool only reads them.
- **Commit-msg hook install:** Writes a static `#!/bin/sh` script. Contents contain no user input. `is_managed_hook` guards against overwriting unmanaged hooks. File permissions set to `0o755` (standard for git hooks).
- **Config parsing:** `serde_yaml` + `Regex::new` validation in `config.rs` fails closed on invalid regex/structure; no `eval`, no code execution from config.
- **Commit message handling:** Content is only `lines()`-iterated and regex-matched. No eval, no shell, no file write based on message content.

### npm wrapper (`packages/creditlint/bin/creditlint.js`)
- `spawnSync(resolved.binary.path, process.argv.slice(2), { stdio: "inherit" })` — no shell, arguments passed as array.
- Binary resolution order: `CREDITLINT_BIN` env → platform optional package → `native/` → cargo `target/release` → `target/debug`. No postinstall scripts in any of the 5 platform packages (verified `package.json` for each).
- `require.resolve` is used to locate platform package roots; no path construction from untrusted input.

### GitHub Actions workflows (`.github/workflows/`)
- **`release.yml`**: triggers only on `push: tags: v*` or `workflow_dispatch` (maintainer-gated). Matrix values are static literals; `github.ref` appears only in `if:` conditions and `concurrency.group`, never in a `run:` shell step. npm publish uses `id-token: write` (OIDC trusted publishing) with `NODE_AUTH_TOKEN: ""` — no long-lived token in CI.
- **`ci.yml`**: `pull_request`/`push: branches: [main]` only; permissions scoped to `contents: read`; no `pull_request_target`.
- **`opencode-*.yml`**: scheduled/dispatch only; `persist-credentials: false` on checkouts; outputs confined to `.opencode-output/`.

### Shell scripts (`scripts/`)
- `publish-npm-packages.sh` and `bootstrap-npm-trust-packages.sh`: `set -euo pipefail`, all CLI flags (`--registry`, `--dist-dir`, `--access`) parsed with `case` and passed as separate argv elements to `npm publish`. The embedded Node heredoc receives inputs via `process.argv`, not string interpolation.

## Conclusion

No privilege boundary exists between the input supplier and the operator for any reachable code path. The native CLI is invoked locally by the operator with operator-supplied inputs; the npm wrapper spawns a native binary using arg arrays with no shell; CI release paths are maintainer-gated with static matrix inputs and OIDC-based publishing. No validated medium, high, or critical vulnerability with a real end-to-end attack path was identified.