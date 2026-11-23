use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use thiserror::Error;

/// NUL byte used by `git log -z` to separate commit records.
///
/// Git object content (commit messages, author names, author emails) can
/// never contain a NUL byte, so a contributor cannot forge this separator
/// by embedding control bytes in their commit message — unlike the previous
/// `\x1e`/`\x1f` separators which could appear in commit bodies and silently
/// split records to bypass policy checks.
const RECORD_SEPARATOR: u8 = 0;
const FIELDS_PER_RECORD: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRecord {
    pub sha: String,
    pub author_name: String,
    pub author_email: String,
    pub committer_name: String,
    pub committer_email: String,
    pub message: String,
}

pub fn collect_range_messages(range: &str) -> Result<Vec<CommitRecord>, GitError> {
    collect_git_messages(
        [
            "log",
            "-z",
            "--format=%H%n%an%n%ae%n%cn%n%ce%n%B",
            range,
        ],
        range,
    )
}

pub fn collect_all_messages() -> Result<Vec<CommitRecord>, GitError> {
    collect_git_messages(
        [
            "log",
            "-z",
            "--format=%H%n%an%n%ae%n%cn%n%ce%n%B",
            "--all",
        ],
        "--all",
    )
}

pub fn commit_msg_hook_path() -> Result<PathBuf, GitError> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-path", "hooks/commit-msg"])
        .output()
        .map_err(GitError::Spawn)?;

    if !output.status.success() {
        return Err(GitError::CommandFailed {
            range: "hooks/commit-msg".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    Ok(PathBuf::from(
        String::from_utf8(output.stdout)
            .map_err(GitError::InvalidUtf8)?
            .trim(),
    ))
}

fn collect_git_messages<const N: usize>(
    args: [&str; N],
    scope: &str,
) -> Result<Vec<CommitRecord>, GitError> {
    let mut child = Command::new("git")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(GitError::Spawn)?;

    let stdout = child.stdout.take().ok_or(GitError::MissingStdout)?;
    let messages = parse_git_log_stream(BufReader::new(stdout))?;
    let status = child.wait().map_err(GitError::Wait)?;

    let mut stderr = String::new();
    if let Some(mut stderr_pipe) = child.stderr.take() {
        stderr_pipe
            .read_to_string(&mut stderr)
            .map_err(GitError::ReadStderr)?;
    }

    if !status.success() {
        return Err(GitError::CommandFailed {
            range: scope.to_string(),
            stderr: stderr.trim().to_string(),
        });
    }

    Ok(messages)
}

#[cfg(test)]
fn parse_git_log_output(stdout: &str) -> Result<Vec<CommitRecord>, GitError> {
    parse_git_log_bytes(stdout.as_bytes())
}

fn parse_git_log_stream<R: Read>(reader: R) -> Result<Vec<CommitRecord>, GitError> {
    let mut bytes = Vec::new();
    BufReader::new(reader)
        .read_to_end(&mut bytes)
        .map_err(GitError::ReadStdout)?;

    parse_git_log_bytes(&bytes)
}

fn parse_git_log_bytes(bytes: &[u8]) -> Result<Vec<CommitRecord>, GitError> {
    let mut records = Vec::new();

    for record_bytes in bytes.split(|&b| b == RECORD_SEPARATOR) {
        if record_bytes.is_empty() {
            continue;
        }

        let record =
            String::from_utf8(record_bytes.to_vec()).map_err(GitError::InvalidUtf8)?;
        let mut fields = record.splitn(FIELDS_PER_RECORD, '\n');
        let sha = fields.next().ok_or(GitError::MalformedOutput {
            detail: "missing commit sha field".to_string(),
        })?;
        let author_name = fields.next().ok_or(GitError::MalformedOutput {
            detail: "missing author name field".to_string(),
        })?;
        let author_email = fields.next().ok_or(GitError::MalformedOutput {
            detail: "missing author email field".to_string(),
        })?;
        let committer_name = fields.next().ok_or(GitError::MalformedOutput {
            detail: "missing committer name field".to_string(),
        })?;
        let committer_email = fields.next().ok_or(GitError::MalformedOutput {
            detail: "missing committer email field".to_string(),
        })?;
        let message = fields.next().ok_or(GitError::MalformedOutput {
            detail: "missing commit message field".to_string(),
        })?;

        records.push(CommitRecord {
            sha: sha.to_string(),
            author_name: author_name.to_string(),
            author_email: author_email.to_string(),
            committer_name: committer_name.to_string(),
            committer_email: committer_email.to_string(),
            message: message.trim_end_matches('\n').to_string(),
        });
    }

    Ok(records)
}

#[derive(Debug, Error)]
pub enum GitError {
    #[error("failed to execute git")]
    Spawn(#[source] std::io::Error),
    #[error("failed while waiting for git to finish")]
    Wait(#[source] std::io::Error),
    #[error("git process did not expose stdout")]
    MissingStdout,
    #[error("git log failed for range `{range}`: {stderr}")]
    CommandFailed { range: String, stderr: String },
    #[error("failed to read git stdout")]
    ReadStdout(#[source] std::io::Error),
    #[error("failed to read git stderr")]
    ReadStderr(#[source] std::io::Error),
    #[error("git output was not valid UTF-8")]
    InvalidUtf8(#[source] std::string::FromUtf8Error),
    #[error("git log output was malformed: {detail}")]
    MalformedOutput { detail: String },
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{parse_git_log_output, parse_git_log_stream};

    #[test]
    fn parses_git_log_output_with_nul_record_separator() {
        let parsed = parse_git_log_output(
            "abc123\nJane Doe\njane@example.com\nCreditlint Test\ncreditlint@example.com\nsubject line\nbody line\u{0000}def456\nJohn Doe\njohn@example.com\nCreditlint Test\ncreditlint@example.com\nsecond subject\u{0000}",
        )
        .expect("parse output");

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].sha, "abc123");
        assert_eq!(parsed[0].author_name, "Jane Doe");
        assert_eq!(parsed[0].author_email, "jane@example.com");
        assert_eq!(parsed[0].committer_name, "Creditlint Test");
        assert_eq!(parsed[0].committer_email, "creditlint@example.com");
        assert_eq!(parsed[0].message, "subject line\nbody line");
        assert_eq!(parsed[1].sha, "def456");
        assert_eq!(parsed[1].message, "second subject");
    }

    #[test]
    fn parses_git_log_stream_incrementally() {
        let parsed = parse_git_log_stream(Cursor::new(
            b"abc123\nJane Doe\njane@example.com\nCreditlint Test\ncreditlint@example.com\nsubject line\nbody line\x00def456\nJohn Doe\njohn@example.com\nCreditlint Test\ncreditlint@example.com\nsecond subject\x00".to_vec(),
        ))
        .expect("parse stream");

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].sha, "abc123");
        assert_eq!(parsed[1].sha, "def456");
    }

    #[test]
    fn embedded_record_separator_byte_in_message_does_not_split_record() {
        let body_with_separator = "feat: add feature\u{001e}\n\nCo-authored-by: Codex <codex@example.com>";
        let raw = format!(
            "abc123\nJane Doe\njane@example.com\nCreditlint Test\ncreditlint@example.com\n{body_with_separator}\u{0000}",
        );

        let parsed = parse_git_log_output(&raw).expect("parse output");

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].sha, "abc123");
        assert!(parsed[0].message.contains('\u{001e}'));
        assert!(parsed[0]
            .message
            .contains("Co-authored-by: Codex <codex@example.com>"));
    }

    #[test]
    fn embedded_field_separator_byte_in_message_does_not_split_record() {
        let body_with_separator = "feat: add feature\u{001f}Co-authored-by: Codex <codex@example.com>";
        let raw = format!(
            "abc123\nJane Doe\njane@example.com\nCreditlint Test\ncreditlint@example.com\n{body_with_separator}\u{0000}",
        );

        let parsed = parse_git_log_output(&raw).expect("parse output");

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].sha, "abc123");
        assert!(parsed[0]
            .message
            .contains("Co-authored-by: Codex <codex@example.com>"));
    }

    #[test]
    fn empty_commit_body_is_preserved() {
        let parsed = parse_git_log_output(
            "abc123\nJane Doe\njane@example.com\nCreditlint Test\ncreditlint@example.com\n\u{0000}",
        )
        .expect("parse output");

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].sha, "abc123");
        assert_eq!(parsed[0].message, "");
    }

    #[test]
    fn malformed_record_with_too_few_fields_fails_closed() {
        let result = parse_git_log_output("abc123\nJane Doe\njane@example.com\u{0000}");

        assert!(result.is_err());
    }
}
