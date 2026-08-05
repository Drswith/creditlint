# Security Review Summary

## No new validated medium+ vulnerabilities found.

**Review scope:** Rust CLI (`src/`), npm wrapper (`packages/creditlint/bin/creditlint.js`),
shell scripts (`scripts/`), and all GitHub Actions workflows (`.github/workflows/`).

**Attack surfaces evaluated:**

- **Git range argument** (`src/git.rs`): Rust's `Command::new("git")` does not invoke a
  shell, so there is no shell injection vector. The `--range` value is supplied by the same
  user who runs the CLI, giving no attacker/victim separation.

- **Config file discovery** (`src/config.rs`): `find_repo_root` walks up the directory tree
  looking for any `.git` entry. A local attacker could theoretically plant a `.git` file in a
  shared ancestor directory to influence config discovery, but this requires local filesystem
  write access on the same system and in the victim's working path. Impact is limited to
  loading a different (possibly weakened) policy config. Rated low, not medium+.

- **Regex patterns from config** (`src/policy/mod.rs`): The Rust `regex` crate guarantees
  linear-time matching and is not vulnerable to classical ReDoS. Patterns originate from the
  repository's own `.creditlint.yml`, not from external attacker input.

- **npm wrapper** (`packages/creditlint/bin/creditlint.js`): `spawnSync` is used without a
  shell. The `CREDITLINT_BIN` environment variable override is documented behavior; anyone who
  can set environment variables already has code execution.

- **GitHub Actions workflows** (`.github/workflows/`): No user-controlled values (PR titles,
  issue bodies, branch names) are interpolated into `run` steps. Permissions follow
  least-privilege. The OpenCode action is pinned to an immutable commit SHA.

- **Shell scripts** (`scripts/`): Proper quoting throughout. No external attacker-controlled
  input is consumed.

**Conclusion:** The codebase is a local-first CLI with no network-facing attack surface, no
SQL, no shell injection, no external callbacks, and no authentication/authorization flows.
No validated medium, high, or critical vulnerabilities with a real end-to-end attack path
were identified in this review.
