use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

fn make_repo() -> tempfile::TempDir {
    let temp_dir = tempdir().expect("tempdir");
    fs::create_dir(temp_dir.path().join(".git")).expect("git dir");
    temp_dir
}

fn init_git_repo() -> tempfile::TempDir {
    let temp_dir = tempdir().expect("tempdir");
    run_git(temp_dir.path(), ["init"]);
    run_git(temp_dir.path(), ["config", "user.name", "Creditlint Test"]);
    run_git(
        temp_dir.path(),
        ["config", "user.email", "creditlint@example.com"],
    );
    temp_dir
}

fn run_git<const N: usize>(repo: &std::path::Path, args: [&str; N]) {
    let status = ProcessCommand::new("git")
        .current_dir(repo)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run git");

    assert!(status.success(), "git command should succeed");
}

fn binary_dir() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_creditlint"))
        .parent()
        .expect("binary parent")
        .to_path_buf()
}

fn path_with_creditlint_binary() -> std::ffi::OsString {
    let current_path = std::env::var_os("PATH").unwrap_or_default();
    let paths = std::env::split_paths(&current_path)
        .chain(std::iter::once(binary_dir()))
        .collect::<Vec<_>>();

    std::env::join_paths(paths).expect("join PATH")
}

fn write_and_commit(
    repo: &std::path::Path,
    filename: &str,
    contents: &str,
    subject: &str,
    body: Option<&str>,
) {
    fs::write(repo.join(filename), contents).expect("write file");
    run_git(repo, ["add", filename]);

    let mut command = ProcessCommand::new("git");
    command.current_dir(repo).args(["commit", "-m", subject]);
    if let Some(body) = body {
        command.args(["-m", body]);
    }
    command.stdout(Stdio::null()).stderr(Stdio::null());

    let status = command.status().expect("git commit");
    assert!(status.success(), "git commit should succeed");
}

fn write_and_commit_with_author(
    repo: &std::path::Path,
    filename: &str,
    contents: &str,
    subject: &str,
    author: &str,
) {
    fs::write(repo.join(filename), contents).expect("write file");
    run_git(repo, ["add", filename]);

    let status = ProcessCommand::new("git")
        .current_dir(repo)
        .args(["commit", "-m", subject, "--author", author])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("git commit");

    assert!(status.success(), "git commit should succeed");
}

fn commit_with_hook_path(
    repo: &Path,
    filename: &str,
    contents: &str,
    subject: &str,
    body: Option<&str>,
) -> std::process::Output {
    fs::write(repo.join(filename), contents).expect("write file");
    run_git(repo, ["add", filename]);

    let mut command = ProcessCommand::new("git");
    command.current_dir(repo).args(["commit", "-m", subject]);
    if let Some(body) = body {
        command.args(["-m", body]);
    }
    command.env("PATH", path_with_creditlint_binary());
    command.output().expect("git commit output")
}

fn head_sha(repo: &std::path::Path) -> String {
    let output = ProcessCommand::new("git")
        .current_dir(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse");

    assert!(output.status.success(), "rev-parse should succeed");
    String::from_utf8(output.stdout)
        .expect("sha utf8")
        .trim()
        .to_string()
}

fn has_head(repo: &Path) -> bool {
    ProcessCommand::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--verify", "HEAD"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("rev-parse HEAD")
        .success()
}

#[test]
fn check_message_file_reports_violation_with_exit_code_one() {
    let repo = make_repo();
    let message_path = repo.path().join("message.txt");
    fs::write(&message_path, "Co-authored-by: Codex <codex@example.com>\n").expect("message");

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .args(["check", "--message-file"])
        .arg(&message_path)
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("forbidden-ai-coauthor"),
        "stdout should contain the rule id"
    );
}

#[test]
fn install_hook_creates_managed_commit_msg_hook() {
    let repo = init_git_repo();

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .arg("install-hook")
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(0));

    let hook_path = repo.path().join(".git/hooks/commit-msg");
    let hook = fs::read_to_string(&hook_path).expect("hook file");
    assert!(hook.contains("creditlint managed hook"));
    assert!(hook.contains("version: 1"));
    assert!(hook.contains("creditlint check --message-file \"$1\""));

    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&hook_path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
}

#[test]
fn install_hook_refuses_to_overwrite_unmanaged_hook() {
    let repo = init_git_repo();
    let hook_path = repo.path().join(".git/hooks/commit-msg");
    fs::write(&hook_path, "#!/bin/sh\necho custom hook\n").expect("hook");

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .arg("install-hook")
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("existing unmanaged commit-msg hook"),
        "stderr should explain the unmanaged hook"
    );
    assert_eq!(
        fs::read_to_string(&hook_path).expect("hook file"),
        "#!/bin/sh\necho custom hook\n"
    );
}

#[test]
fn install_hook_replaces_existing_managed_hook() {
    let repo = init_git_repo();
    let hook_path = repo.path().join(".git/hooks/commit-msg");
    fs::write(
        &hook_path,
        "#!/bin/sh\n# creditlint managed hook\n# version: 1\n\necho old hook\n",
    )
    .expect("hook");

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .arg("install-hook")
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(0));
    let hook = fs::read_to_string(&hook_path).expect("hook file");
    assert!(hook.contains("creditlint check --message-file \"$1\""));
    assert!(!hook.contains("echo old hook"));
}

#[test]
fn managed_hook_blocks_violating_commit() {
    let repo = init_git_repo();

    let install = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .arg("install-hook")
        .output()
        .expect("run command");
    assert_eq!(install.status.code(), Some(0));

    let output = commit_with_hook_path(
        repo.path(),
        "blocked.txt",
        "blocked\n",
        "blocked commit",
        Some("Co-authored-by: Codex <codex@example.com>"),
    );

    assert!(
        !output.status.success(),
        "commit should be rejected by hook"
    );
    assert!(
        !has_head(repo.path()),
        "rejected commit should not create HEAD"
    );
}

#[test]
fn managed_hook_allows_clean_commit() {
    let repo = init_git_repo();

    let install = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .arg("install-hook")
        .output()
        .expect("run command");
    assert_eq!(install.status.code(), Some(0));

    let output = commit_with_hook_path(
        repo.path(),
        "clean.txt",
        "clean\n",
        "clean commit",
        Some("Reviewed-by: Jane Doe <jane@example.com>"),
    );

    assert!(output.status.success(), "clean commit should pass hook");
    assert!(
        has_head(repo.path()),
        "successful commit should create HEAD"
    );
}

#[test]
fn init_writes_default_config_file() {
    let repo = make_repo();

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .arg("init")
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(0));

    let config_path = repo.path().join(".creditlint.yml");
    let config = fs::read_to_string(&config_path).expect("config file");
    assert!(config.contains("version: 1"));
    assert!(config.contains("forbidden_identities"));
    assert!(config.contains("key: Co-authored-by"));
    assert!(config.contains("allowed_provenance_trailers"));
}

#[test]
fn init_refuses_to_overwrite_existing_config() {
    let repo = make_repo();
    let config_path = repo.path().join(".creditlint.yml");
    fs::write(
        &config_path,
        "version: 1\nrules:\n  forbidden_trailers: []\n",
    )
    .expect("config");

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .arg("init")
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("config file already exists"),
        "stderr should explain the existing config"
    );
    assert_eq!(
        fs::read_to_string(&config_path).expect("config file"),
        "version: 1\nrules:\n  forbidden_trailers: []\n"
    );
}

#[test]
fn check_stdin_supports_json_output() {
    let repo = make_repo();

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .args(["check", "--stdin", "--format", "json"])
        .write_stdin("Made with Cursor\n")
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(1));

    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["ok"], Value::Bool(false));
    assert_eq!(
        json["violations"][0]["rule_id"],
        "forbidden-made-with-marker"
    );
    assert_eq!(json["violations"][0]["source"]["kind"], "Stdin");
}

#[test]
fn clean_input_returns_exit_code_zero() {
    let repo = make_repo();

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .args(["check", "--stdin"])
        .write_stdin("Co-authored-by: Jane Doe <jane@example.com>\n")
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn invalid_config_returns_exit_code_two() {
    let repo = make_repo();
    let message_path = repo.path().join("message.txt");
    fs::write(
        &message_path,
        "Co-authored-by: Jane Doe <jane@example.com>\n",
    )
    .expect("message");
    fs::write(
        repo.path().join(".creditlint.yml"),
        "version: 1\nrules:\n  forbidden_trailers:\n    - key: [\n",
    )
    .expect("config");

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .args(["check", "--message-file"])
        .arg(&message_path)
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("failed to load policy"),
        "stderr should explain the config failure"
    );
}

#[test]
fn check_range_clean_commits_returns_zero() {
    let repo = init_git_repo();
    write_and_commit(repo.path(), "first.txt", "first\n", "first commit", None);
    write_and_commit(repo.path(), "second.txt", "second\n", "second commit", None);

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .args(["check", "--range", "HEAD~1..HEAD"])
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn check_range_violating_commit_returns_one_and_includes_sha() {
    let repo = init_git_repo();
    write_and_commit(repo.path(), "first.txt", "first\n", "first commit", None);
    write_and_commit(
        repo.path(),
        "second.txt",
        "second\n",
        "second commit",
        Some("Co-authored-by: Codex <codex@example.com>"),
    );
    let violating_sha = head_sha(repo.path());

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .args(["check", "--range", "HEAD~1..HEAD"])
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("forbidden-ai-coauthor"));
    assert!(stdout.contains(&violating_sha));
}

#[test]
fn check_range_rejects_cursor_agent_git_author_identity() {
    let repo = init_git_repo();
    write_and_commit(repo.path(), "first.txt", "first\n", "first commit", None);
    write_and_commit_with_author(
        repo.path(),
        "second.txt",
        "second\n",
        "second commit",
        "Cursor Agent <cursoragent@cursor.com>",
    );
    let violating_sha = head_sha(repo.path());

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .args(["check", "--range", "HEAD~1..HEAD"])
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("forbidden-ai-git-identity"));
    assert!(stdout.contains("author.name"));
    assert!(stdout.contains(&violating_sha));
}

#[test]
fn check_range_invalid_range_returns_two() {
    let repo = init_git_repo();
    write_and_commit(repo.path(), "first.txt", "first\n", "first commit", None);

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .args(["check", "--range", "missing..HEAD"])
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("failed to collect commit messages from git"),
        "stderr should explain the git range failure"
    );
}

#[test]
fn audit_all_reports_violations() {
    let repo = init_git_repo();
    write_and_commit(repo.path(), "first.txt", "first\n", "first commit", None);
    write_and_commit(
        repo.path(),
        "second.txt",
        "second\n",
        "second commit",
        Some("Made with Cursor"),
    );

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .args(["audit", "--all", "--format", "json"])
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(1));
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json["ok"], Value::Bool(false));
    assert_eq!(
        json["violations"][0]["rule_id"],
        "forbidden-made-with-marker"
    );
}

#[test]
fn audit_and_range_detect_trailer_after_record_separator_byte() {
    // Regression: a 0x1e byte (the legacy git log record separator) inside a
    // commit message must not split the record so that a forbidden trailer is
    // silently dropped from audit/range analysis.
    let repo = init_git_repo();
    write_and_commit(repo.path(), "first.txt", "first\n", "first commit", None);

    let message_path = repo.path().join("commit-message.txt");
    let mut message = String::from("legit subject");
    message.push('\u{001e}');
    message.push('\n');
    message.push_str("Co-authored-by: Codex <codex@example.com>\n");
    fs::write(&message_path, message).expect("write message file");

    fs::write(repo.path().join("second.txt"), "second\n").expect("write file");
    run_git(repo.path(), ["add", "second.txt"]);
    let status = ProcessCommand::new("git")
        .current_dir(repo.path())
        .args(["commit", "-F", message_path.to_str().unwrap()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("git commit");
    assert!(status.success(), "git commit should succeed");

    let audit = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .args(["audit", "--all", "--format", "json"])
        .output()
        .expect("run audit");
    assert_eq!(audit.status.code(), Some(1), "audit should detect the trailer");
    let audit_json: Value = serde_json::from_slice(&audit.stdout).expect("audit json");
    assert_eq!(audit_json["ok"], Value::Bool(false));
    assert!(
        audit_json["violations"]
            .as_array()
            .map(|v| !v.is_empty())
            .unwrap_or(false),
        "audit should report at least one violation"
    );

    let check = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .args(["check", "--range", "HEAD~1..HEAD", "--format", "json"])
        .output()
        .expect("run check range");
    assert_eq!(
        check.status.code(),
        Some(1),
        "check --range should detect the trailer"
    );
    let check_json: Value = serde_json::from_slice(&check.stdout).expect("check json");
    assert_eq!(check_json["ok"], Value::Bool(false));
}

#[test]
fn github_ruleset_pattern_exports_default_policy() {
    let repo = make_repo();

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .args(["github", "ruleset-pattern"])
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("Co\\-authored\\-by"));
    assert!(stdout.contains("made[- ]with"));
}

#[test]
fn github_ruleset_pattern_fails_closed_for_unsupported_policy() {
    let repo = make_repo();
    fs::write(
        repo.path().join(".creditlint.yml"),
        r#"version: 1
rules:
  forbidden_trailers:
    - key: Generated-by
  allowed_provenance_trailers:
    - Generated-by
"#,
    )
    .expect("config");

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .args(["github", "ruleset-pattern"])
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("cannot be represented safely"),
        "stderr should explain the fail-closed export"
    );
}

#[test]
fn nested_directory_check_uses_repo_root_config() {
    let repo = init_git_repo();
    let nested_dir = repo.path().join("nested/deeper");
    fs::create_dir_all(&nested_dir).expect("nested dir");
    fs::write(
        repo.path().join(".creditlint.yml"),
        r#"version: 1
rules:
  forbidden_trailers:
    - key: X-Custom-Attribution
      value_pattern: "(?i)agent"
"#,
    )
    .expect("config");

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(&nested_dir)
        .args(["check", "--stdin"])
        .write_stdin("X-Custom-Attribution: agent\n")
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("config-forbidden-rule-0"),
        "stdout should reflect the repo-root policy"
    );
}

#[test]
fn init_outside_git_repo_fails_closed() {
    let temp_dir = tempdir().expect("tempdir");

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(temp_dir.path())
        .arg("init")
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("failed to load policy"),
        "stderr should explain the init failure"
    );
}

#[test]
fn install_hook_outside_git_repo_fails_closed() {
    let temp_dir = tempdir().expect("tempdir");

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(temp_dir.path())
        .arg("install-hook")
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        !output.stderr.is_empty(),
        "stderr should be non-empty for missing git metadata"
    );
}

#[test]
fn github_ruleset_pattern_fails_closed_for_regex_trailer_keys() {
    let repo = make_repo();
    fs::write(
        repo.path().join(".creditlint.yml"),
        r#"version: 1
rules:
  forbidden_trailers:
    - key_pattern: "(?i)x-.*"
      value_pattern: "agent"
"#,
    )
    .expect("config");

    let output = Command::cargo_bin("creditlint")
        .expect("binary")
        .current_dir(repo.path())
        .args(["github", "ruleset-pattern"])
        .output()
        .expect("run command");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("requires an exact trailer key"),
        "stderr should explain the unsupported regex trailer key"
    );
}
