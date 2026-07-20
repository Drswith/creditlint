use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::Deserialize;
use thiserror::Error;

use crate::policy::{
    FieldMatcher, IdentityRole, IdentityRoleMatcher, IdentityRule, Policy, Rule, RuleKind,
    ValueMatcher,
};

pub const CONFIG_FILE_NAME: &str = ".creditlint.yml";
const DEFAULT_CONFIG_CONTENTS: &str = r#"version: 1
rules:
  forbidden_identities:
    - name_pattern: "(?i)(cursor agent|codex|claude|copilot|openai|anthropic|gemini)"
      email_pattern: "(?i)(cursoragent@cursor\\.com|codex|claude|copilot|openai|anthropic|gemini)"
  forbidden_trailers:
    - key: Co-authored-by
      value_pattern: "(?i)(codex|claude|cursor|copilot|openai|anthropic|gemini|\\bai\\b)"
    - key_pattern: "(?i)^made[- ]with\\b.*$"
    - key_pattern: "(?i)^made[- ]on\\b.*$"
    - key_pattern: "(?i)^generated[- ]with\\b.*$"
  allowed_provenance_trailers:
    - AI-Assisted
    - Tool-Used
    - Generated-by
"#;

pub fn load_policy_from_current_dir() -> Result<Policy, ConfigError> {
    let current_dir = env::current_dir().map_err(ConfigError::CurrentDir)?;
    load_policy(&current_dir)
}

pub fn default_config_contents() -> &'static str {
    DEFAULT_CONFIG_CONTENTS
}

pub fn init_config_path_from_current_dir() -> Result<PathBuf, ConfigError> {
    let current_dir = env::current_dir().map_err(ConfigError::CurrentDir)?;
    init_config_path(&current_dir)
}

pub fn init_config_path(start_dir: &Path) -> Result<PathBuf, ConfigError> {
    Ok(find_repo_root(start_dir)?.join(CONFIG_FILE_NAME))
}

pub fn load_policy(start_dir: &Path) -> Result<Policy, ConfigError> {
    let config_path = discover_config(start_dir)?;

    match config_path {
        Some(path) => {
            let content = fs::read_to_string(&path).map_err(|source| ConfigError::Read {
                path: path.clone(),
                source,
            })?;
            let raw = serde_yaml::from_str::<RawConfig>(&content).map_err(|source| {
                ConfigError::Parse {
                    path: path.clone(),
                    source,
                }
            })?;

            raw.into_policy()
        }
        None => Ok(Policy::default()),
    }
}

fn discover_config(start_dir: &Path) -> Result<Option<PathBuf>, ConfigError> {
    let repo_root = find_repo_root(start_dir)?;
    let mut current = Some(start_dir);

    while let Some(dir) = current {
        let candidate = dir.join(CONFIG_FILE_NAME);
        if candidate.is_file() {
            return Ok(Some(candidate));
        }

        if dir == repo_root {
            break;
        }

        current = dir.parent();
    }

    Ok(None)
}

fn find_repo_root(start_dir: &Path) -> Result<&Path, ConfigError> {
    let mut current = Some(start_dir);

    while let Some(dir) = current {
        let git_entry = dir.join(".git");
        if git_entry.exists() {
            return Ok(dir);
        }

        current = dir.parent();
    }

    Err(ConfigError::RepoRootNotFound {
        start_dir: start_dir.to_path_buf(),
    })
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to determine current working directory")]
    CurrentDir(#[source] std::io::Error),
    #[error("failed to read config file at {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config file at {path}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("repository root could not be determined from {start_dir}")]
    RepoRootNotFound { start_dir: PathBuf },
    #[error("config validation failed: {0}")]
    Validation(String),
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    version: u32,
    rules: RawRules,
}

impl RawConfig {
    fn into_policy(self) -> Result<Policy, ConfigError> {
        if self.version != 1 {
            return Err(ConfigError::Validation(format!(
                "unsupported config version {}",
                self.version
            )));
        }

        let mut rules = Vec::with_capacity(self.rules.forbidden_trailers.len());
        let mut identity_rules = Vec::with_capacity(self.rules.forbidden_identities.len());

        for (index, raw_rule) in self.rules.forbidden_identities.into_iter().enumerate() {
            identity_rules.push(raw_rule.into_identity_rule(index)?);
        }

        for (index, raw_rule) in self.rules.forbidden_trailers.into_iter().enumerate() {
            rules.push(raw_rule.into_rule(index)?);
        }

        Ok(Policy {
            rules,
            identity_rules,
            allowed_provenance_keys: self.rules.allowed_provenance_trailers,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawRules {
    #[serde(default)]
    forbidden_identities: Vec<RawForbiddenIdentityRule>,
    #[serde(default)]
    forbidden_trailers: Vec<RawForbiddenRule>,
    #[serde(default)]
    allowed_provenance_trailers: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawForbiddenIdentityRule {
    role: Option<RawIdentityRole>,
    name_pattern: Option<String>,
    email_pattern: Option<String>,
}

impl RawForbiddenIdentityRule {
    fn into_identity_rule(self, index: usize) -> Result<IdentityRule, ConfigError> {
        if self.name_pattern.is_none() && self.email_pattern.is_none() {
            return Err(ConfigError::Validation(format!(
                "forbidden_identities[{index}] must define name_pattern or email_pattern"
            )));
        }

        if let Some(pattern) = &self.name_pattern {
            validate_identity_pattern(pattern, index, "name")?;
        }

        if let Some(pattern) = &self.email_pattern {
            validate_identity_pattern(pattern, index, "email")?;
        }

        Ok(IdentityRule {
            id: format!("config-forbidden-identity-{index}"),
            role_matcher: self
                .role
                .map(|role| IdentityRoleMatcher::Role(role.into()))
                .unwrap_or(IdentityRoleMatcher::Any),
            name_pattern: self.name_pattern,
            email_pattern: self.email_pattern,
            message: "Config-defined Git identity is not allowed".to_string(),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawIdentityRole {
    Author,
    Committer,
}

impl From<RawIdentityRole> for IdentityRole {
    fn from(value: RawIdentityRole) -> Self {
        match value {
            RawIdentityRole::Author => IdentityRole::Author,
            RawIdentityRole::Committer => IdentityRole::Committer,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawForbiddenRule {
    key: Option<String>,
    key_pattern: Option<String>,
    value_pattern: Option<String>,
}

impl RawForbiddenRule {
    fn into_rule(self, index: usize) -> Result<Rule, ConfigError> {
        let field_matcher = match (self.key, self.key_pattern) {
            (Some(key), None) => FieldMatcher::Exact(key),
            (None, Some(pattern)) => FieldMatcher::Pattern(pattern),
            (Some(_), Some(_)) => {
                return Err(ConfigError::Validation(format!(
                    "forbidden_trailers[{index}] cannot define both key and key_pattern"
                )));
            }
            (None, None) => {
                return Err(ConfigError::Validation(format!(
                    "forbidden_trailers[{index}] must define key or key_pattern"
                )));
            }
        };

        let kind = match (&field_matcher, self.value_pattern.as_ref()) {
            (FieldMatcher::Exact(_), _) => RuleKind::ForbiddenTrailer,
            (FieldMatcher::Pattern(_), Some(_)) => RuleKind::ForbiddenTrailer,
            (FieldMatcher::Pattern(_), None) => RuleKind::FreeformMarker,
            (FieldMatcher::Any, _) => RuleKind::FreeformMarker,
        };

        let value_matcher = match self.value_pattern {
            Some(pattern) => ValueMatcher::Pattern(pattern),
            None => ValueMatcher::Any,
        };

        validate_matcher(&field_matcher, index, "field")?;
        validate_matcher_value(&value_matcher, index, "value")?;

        Ok(Rule {
            id: format!("config-forbidden-rule-{index}"),
            kind,
            field_matcher,
            value_matcher,
            message: "Config-defined credit marker is not allowed".to_string(),
        })
    }
}

fn validate_matcher(
    matcher: &FieldMatcher,
    index: usize,
    matcher_name: &str,
) -> Result<(), ConfigError> {
    match matcher {
        FieldMatcher::Pattern(pattern) => validate_pattern(pattern, index, matcher_name),
        FieldMatcher::Any | FieldMatcher::Exact(_) => Ok(()),
    }
}

fn validate_matcher_value(
    matcher: &ValueMatcher,
    index: usize,
    matcher_name: &str,
) -> Result<(), ConfigError> {
    match matcher {
        ValueMatcher::Pattern(pattern) => validate_pattern(pattern, index, matcher_name),
        ValueMatcher::Any | ValueMatcher::Exact(_) => Ok(()),
    }
}

fn validate_pattern(pattern: &str, index: usize, matcher_name: &str) -> Result<(), ConfigError> {
    Regex::new(pattern).map_err(|source| {
        ConfigError::Validation(format!(
            "forbidden_trailers[{index}] has invalid {matcher_name}_pattern `{pattern}`: {source}"
        ))
    })?;

    Ok(())
}

fn validate_identity_pattern(
    pattern: &str,
    index: usize,
    matcher_name: &str,
) -> Result<(), ConfigError> {
    Regex::new(pattern).map_err(|source| {
        ConfigError::Validation(format!(
            "forbidden_identities[{index}] has invalid {matcher_name}_pattern `{pattern}`: {source}"
        ))
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{ConfigError, load_policy};
    use crate::policy::{FieldMatcher, Identity, IdentityRole, RuleKind, Source, SourceKind};

    #[test]
    fn loads_custom_rule_from_config_file() {
        let temp_dir = tempdir().expect("tempdir");
        let repo_root = temp_dir.path();
        fs::create_dir(repo_root.join(".git")).expect("git dir");
        let nested = repo_root.join("nested");
        fs::create_dir(&nested).expect("nested dir");

        fs::write(
            repo_root.join(".creditlint.yml"),
            r#"
version: 1
rules:
  forbidden_identities:
    - role: author
      name_pattern: "(?i)agent"
  forbidden_trailers:
    - key: X-Custom-Attribution
      value_pattern: "(?i)agent"
  allowed_provenance_trailers:
    - Tool-Used
"#,
        )
        .expect("config file");

        let policy = load_policy(&nested).expect("load policy");

        assert_eq!(
            policy.allowed_provenance_keys,
            vec!["Tool-Used".to_string()]
        );
        assert_eq!(policy.rules.len(), 1);
        assert_eq!(policy.identity_rules.len(), 1);
        assert_eq!(policy.rules[0].kind, RuleKind::ForbiddenTrailer);
        assert_eq!(
            policy.rules[0].field_matcher,
            FieldMatcher::Exact("X-Custom-Attribution".to_string())
        );

        let violations = policy
            .analyze_identity(
                Source {
                    kind: SourceKind::Commit,
                    path: None,
                    commit_sha: None,
                },
                &Identity {
                    role: IdentityRole::Author,
                    name: "Agent Smith".to_string(),
                    email: "agent@example.com".to_string(),
                },
            )
            .expect("identity analysis");
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "config-forbidden-identity-0");
    }

    #[test]
    fn invalid_config_fails_closed() {
        let temp_dir = tempdir().expect("tempdir");
        let repo_root = temp_dir.path();
        fs::create_dir(repo_root.join(".git")).expect("git dir");

        fs::write(
            repo_root.join(".creditlint.yml"),
            "version: 1\nrules:\n  forbidden_trailers:\n    - key: [\n",
        )
        .expect("config file");

        let error = load_policy(repo_root).expect_err("invalid config should fail");
        assert!(matches!(error, ConfigError::Parse { .. }));
    }

    #[test]
    fn invalid_regex_pattern_fails_closed() {
        let temp_dir = tempdir().expect("tempdir");
        let repo_root = temp_dir.path();
        fs::create_dir(repo_root.join(".git")).expect("git dir");

        fs::write(
            repo_root.join(".creditlint.yml"),
            r#"
version: 1
rules:
  forbidden_identities:
    - name_pattern: "["
  forbidden_trailers:
    - key: X-Bad
      value_pattern: "["
"#,
        )
        .expect("config file");

        let error = load_policy(repo_root).expect_err("invalid regex should fail");
        assert!(matches!(error, ConfigError::Validation(_)));
    }

    #[test]
    fn missing_config_uses_default_policy() {
        let temp_dir = tempdir().expect("tempdir");
        let repo_root = temp_dir.path();
        fs::create_dir(repo_root.join(".git")).expect("git dir");

        let policy = load_policy(repo_root).expect("default policy");
        let source = Source {
            kind: SourceKind::MessageFile,
            path: None,
            commit_sha: None,
        };
        let violations = policy
            .analyze(source, "Co-authored-by: Codex <codex@example.com>")
            .expect("analysis should succeed");

        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn config_can_define_only_identity_rules() {
        let temp_dir = tempdir().expect("tempdir");
        let repo_root = temp_dir.path();
        fs::create_dir(repo_root.join(".git")).expect("git dir");

        fs::write(
            repo_root.join(".creditlint.yml"),
            r#"
version: 1
rules:
  forbidden_identities:
    - email_pattern: "(?i)cursoragent@cursor\\.com"
"#,
        )
        .expect("config file");

        let policy = load_policy(repo_root).expect("load policy");
        assert!(policy.rules.is_empty());
        assert_eq!(policy.identity_rules.len(), 1);
    }
}
