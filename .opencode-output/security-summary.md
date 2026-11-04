# Security Review Summary

No new validated medium+ vulnerabilities found.

## Scope

Reviewed the `creditlint` Rust CLI, optional npm wrapper, maintainer scripts,
and GitHub Actions workflows.

## Attack surfaces examined

- **Git range argument** (`src/git.rs`): `--range` is passed as a single argv
  element to `git log`. No shell injection. A `--`-prefixed range can only
  inject one inert option at a time; dangerous git options like `--exec`
  require companion flags (`--bisect`) that cannot be supplied in a single
  argv element. In CI, the range derives from `github.base_ref`, which must
  be an existing branch (requires push access).
- **Config regex patterns** (`src/config.rs`): Rust's `regex` crate uses a
  finite-automaton engine with no backtracking, so ReDoS is not possible.
  Config is repo-trusted.
- **npm wrapper binary resolution** (`packages/creditlint/bin/creditlint.js`):
  Uses standard `require.resolve` and `CREDITLINT_BIN` env override. No
  creditlint-specific attack path beyond generic npm supply-chain concerns.
- **Install-hook write** (`src/cli/mod.rs`): Writes a static hook script to
  the path from `git rev-parse --git-path hooks/commit-msg`. Even if
  `core.hooksPath` redirects the path, the written contents are static and
  do not execute attacker-controlled commands.
- **README CI example**: Uses `${{ github.base_ref }}` in a shell `run:`
  block, but `github.base_ref` must be an existing branch requiring
  collaborator push access. Not an external-attacker privilege escalation.
- **GitHub Actions workflows**: Secrets passed via environment variables.
  No untrusted input (PR content, issue content, branch names) interpolated
  into shell commands.
- **Maintainer scripts** (`scripts/`): Not exposed to untrusted input.
