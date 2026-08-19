# Security Review Summary

No new validated medium+ vulnerabilities found.

## Scope

Reviewed the `creditlint` repository (Rust-native CLI for Git credit/authorship
metadata policy), including:

- Rust source under `src/` (CLI, policy engine, config loader, git metadata
  collector, reporter, GitHub ruleset exporter).
- npm wrapper under `packages/creditlint/bin/creditlint.js` and platform
  package skeletons under `packages/creditlint-*`.
- Shell scripts under `scripts/`.
- GitHub Actions workflows under `.github/workflows/`.
- OpenSpec change artifacts under `openspec/changes/`.

## Attack surfaces examined

- **Rust CLI input paths**: `--range`, `--message-file`, `--stdin`, config
  discovery, hook installation, `init` config writing. All user-supplied inputs
  are controlled by the user running the CLI (self-attack only). `git log` is
  invoked via `Command::new("git").args([...])` with no shell, so the `--range`
  value is passed as a single positional argument and cannot inject git
  options or shell metacharacters.
- **Config loading**: `.creditlint.yml` is discovered only within the bounds of
  the repository root (`.git` directory). Regex patterns are validated at load
  time and fail closed. ReDoS would require repo write access to plant a
  malicious config.
- **Hook installation**: `commit_msg_hook_path` runs `git rev-parse --git-path
  hooks/commit-msg`, which honors `core.hooksPath`. Redirecting the written
  hook to an arbitrary path would require the attacker to first set
  `core.hooksPath` in the local git config (self-attack only). The written
  content is a fixed, benign shell script.
- **npm wrapper**: Uses `spawnSync(path, args, { stdio: "inherit" })` with
  `args = process.argv.slice(2)`. No shell, no string interpolation, so no
  command injection. `CREDITLINT_BIN` is a documented override for the user's
  own environment. Platform package resolution uses standard
  `require.resolve`; no path traversal from untrusted input.
- **CI workflows**: `ci.yml` runs on PRs with `contents: read` only. Fork PRs
  do not receive repository secrets by GitHub default. `release.yml` is
  triggered only by maintainer tags or `workflow_dispatch`; permissions are
  job-scoped and secrets come from `${{ secrets.* }}` (no hardcoded
  credentials).
- **OpenCode workflows**: Pinned to an immutable commit SHA
  (`anomalyco/opencode/github@77fc88c...`). The security-review and
  critical-bug-scan workflows are triggered only by schedule or maintainer
  manual dispatch (no external trigger). The issue-triage workflow is
  triggered by external issue authors but gates on a 30-day account-age check
  and only grants `issues: write`; prompt-injection risk is inherent to
  LLM-based automation and is bounded to comments on the attacker's own
  issue, not a code-level vulnerability with a meaningful security boundary
  crossing.
- **Secrets handling**: All workflow secrets are referenced via
  `${{ secrets.* }}`. `NODE_AUTH_TOKEN` is intentionally empty in the
  `publish-npm` job because npm trusted publishing uses GitHub OIDC
  (`id-token: write`). `CARGO_REGISTRY_TOKEN` is checked for emptiness before
  `cargo publish`.

## Conclusion

No validated medium, high, or critical vulnerabilities with a real
end-to-end attack path were identified in this review. Candidate findings
were either self-attack only (user controls the input that reaches the
vulnerable code), required prior local write access, or were inherent
LLM-automation behavior risks rather than code vulnerabilities.
