use std::path::PathBuf;

use regex::Regex;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Policy {
    pub rules: Vec<Rule>,
    pub identity_rules: Vec<IdentityRule>,
    pub allowed_provenance_keys: Vec<String>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            rules: vec![
                Rule {
                    id: "forbidden-ai-coauthor".to_string(),
                    kind: RuleKind::ForbiddenTrailer,
                    field_matcher: FieldMatcher::Exact("Co-authored-by".to_string()),
                    value_matcher: ValueMatcher::Pattern(
                        "(?i)(codex|claude|cursor|copilot|openai|anthropic|gemini|\\bai\\b)"
                            .to_string(),
                    ),
                    message: "AI/tool authorship marker is not allowed".to_string(),
                },
                Rule {
                    id: "forbidden-made-with-marker".to_string(),
                    kind: RuleKind::FreeformMarker,
                    field_matcher: FieldMatcher::Any,
                    value_matcher: ValueMatcher::Pattern("(?i)^made[- ]with\\b.*$".to_string()),
                    message: "Made-with credit marker is not allowed".to_string(),
                },
                Rule {
                    id: "forbidden-made-on-marker".to_string(),
                    kind: RuleKind::FreeformMarker,
                    field_matcher: FieldMatcher::Any,
                    value_matcher: ValueMatcher::Pattern("(?i)^made[- ]on\\b.*$".to_string()),
                    message: "Made-on credit marker is not allowed".to_string(),
                },
                Rule {
                    id: "forbidden-generated-with-marker".to_string(),
                    kind: RuleKind::FreeformMarker,
                    field_matcher: FieldMatcher::Any,
                    value_matcher: ValueMatcher::Pattern(
                        "(?i)^generated[- ]with\\b.*$".to_string(),
                    ),
                    message: "Generated-with credit marker is not allowed".to_string(),
                },
                Rule {
                    id: "forbidden-author-identity-line".to_string(),
                    kind: RuleKind::FreeformMarker,
                    field_matcher: FieldMatcher::Any,
                    value_matcher: ValueMatcher::Pattern(
                        "(?i)^author:\\s*(cursor agent|codex|claude|copilot|openai|anthropic|gemini)\\b.*$"
                            .to_string(),
                    ),
                    message: "AI/tool author identity line is not allowed".to_string(),
                },
            ],
            identity_rules: vec![IdentityRule {
                id: "forbidden-ai-git-identity".to_string(),
                role_matcher: IdentityRoleMatcher::Any,
                name_pattern: Some(
                    "(?i)(cursor agent|codex|claude|copilot|openai|anthropic|gemini)".to_string(),
                ),
                email_pattern: Some(
                    "(?i)(cursoragent@cursor\\.com|codex|claude|copilot|openai|anthropic|gemini)"
                        .to_string(),
                ),
                message: "AI/tool Git author or committer identity is not allowed".to_string(),
            }],
            allowed_provenance_keys: vec![
                "AI-Assisted".to_string(),
                "Tool-Used".to_string(),
                "Generated-by".to_string(),
            ],
        }
    }
}

impl Policy {
    pub fn analyze(&self, source: Source, message: &str) -> Result<Vec<Violation>, AnalysisError> {
        let mut violations = Vec::new();

        for (line_index, raw_line) in message.lines().enumerate() {
            let line_number = line_index + 1;
            let trimmed = raw_line.trim();

            if trimmed.is_empty() {
                continue;
            }

            if let Some((field, value)) = parse_trailer_line(trimmed) {
                let mut matched_forbidden_rule = false;

                for rule in self
                    .rules
                    .iter()
                    .filter(|rule| rule.kind == RuleKind::ForbiddenTrailer)
                {
                    if rule.matches_trailer(field, value)? {
                        matched_forbidden_rule = true;
                        violations.push(Violation {
                            source: source.clone(),
                            rule_id: rule.id.clone(),
                            field: Some(field.to_string()),
                            line: Some(line_number),
                            message: rule.message.clone(),
                        });
                    }
                }

                if self.is_allowed_provenance_key(field) && !matched_forbidden_rule {
                    continue;
                }
            }

            for rule in self
                .rules
                .iter()
                .filter(|rule| rule.kind == RuleKind::FreeformMarker)
            {
                // Free-form rules apply to the trimmed line as a whole so normal
                // prose like "made with care" is not treated as a marker.
                if rule.matches_freeform(trimmed)? {
                    violations.push(Violation {
                        source: source.clone(),
                        rule_id: rule.id.clone(),
                        field: None,
                        line: Some(line_number),
                        message: rule.message.clone(),
                    });
                }
            }
        }

        Ok(violations)
    }

    pub fn analyze_identity(
        &self,
        source: Source,
        identity: &Identity,
    ) -> Result<Vec<Violation>, AnalysisError> {
        let mut violations = Vec::new();

        for rule in &self.identity_rules {
            if let Some(field) = rule.matches_identity(identity)? {
                violations.push(Violation {
                    source: source.clone(),
                    rule_id: rule.id.clone(),
                    field: Some(field),
                    line: None,
                    message: rule.message.clone(),
                });
            }
        }

        Ok(violations)
    }

    fn is_allowed_provenance_key(&self, field: &str) -> bool {
        self.allowed_provenance_keys.iter().any(|key| key == field)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rule {
    pub id: String,
    pub kind: RuleKind,
    pub field_matcher: FieldMatcher,
    pub value_matcher: ValueMatcher,
    pub message: String,
}

impl Rule {
    fn matches_trailer(&self, field: &str, value: &str) -> Result<bool, AnalysisError> {
        Ok(self.field_matcher.matches(field, &self.id)?
            && self.value_matcher.matches(value, &self.id)?)
    }

    fn matches_freeform(&self, line: &str) -> Result<bool, AnalysisError> {
        match (&self.field_matcher, &self.value_matcher) {
            (FieldMatcher::Any, matcher) => matcher.matches(line, &self.id),
            (matcher, ValueMatcher::Any) => matcher.matches(line, &self.id),
            (field_matcher, value_matcher) => {
                Ok(field_matcher.matches(line, &self.id)?
                    && value_matcher.matches(line, &self.id)?)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IdentityRule {
    pub id: String,
    pub role_matcher: IdentityRoleMatcher,
    pub name_pattern: Option<String>,
    pub email_pattern: Option<String>,
    pub message: String,
}

impl IdentityRule {
    fn matches_identity(&self, identity: &Identity) -> Result<Option<String>, AnalysisError> {
        if !self.role_matcher.matches(identity.role) {
            return Ok(None);
        }

        let role = identity.role.field_prefix();

        if let Some(pattern) = &self.name_pattern
            && matches_pattern(pattern, &identity.name, &self.id)?
        {
            return Ok(Some(format!("{role}.name")));
        }

        if let Some(pattern) = &self.email_pattern
            && matches_pattern(pattern, &identity.email, &self.id)?
        {
            return Ok(Some(format!("{role}.email")));
        }

        Ok(None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum IdentityRoleMatcher {
    Any,
    Role(IdentityRole),
}

impl IdentityRoleMatcher {
    fn matches(self, role: IdentityRole) -> bool {
        match self {
            IdentityRoleMatcher::Any => true,
            IdentityRoleMatcher::Role(expected) => expected == role,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Identity {
    pub role: IdentityRole,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum IdentityRole {
    Author,
    Committer,
}

impl IdentityRole {
    fn field_prefix(self) -> &'static str {
        match self {
            IdentityRole::Author => "author",
            IdentityRole::Committer => "committer",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RuleKind {
    ForbiddenTrailer,
    FreeformMarker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FieldMatcher {
    Any,
    Exact(String),
    Pattern(String),
}

impl FieldMatcher {
    fn matches(&self, value: &str, rule_id: &str) -> Result<bool, AnalysisError> {
        match self {
            FieldMatcher::Any => Ok(true),
            FieldMatcher::Exact(expected) => Ok(expected == value),
            FieldMatcher::Pattern(pattern) => matches_pattern(pattern, value, rule_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ValueMatcher {
    Any,
    Exact(String),
    Pattern(String),
}

impl ValueMatcher {
    fn matches(&self, value: &str, rule_id: &str) -> Result<bool, AnalysisError> {
        match self {
            ValueMatcher::Any => Ok(true),
            ValueMatcher::Exact(expected) => Ok(expected == value),
            ValueMatcher::Pattern(pattern) => matches_pattern(pattern, value, rule_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Source {
    pub kind: SourceKind,
    pub path: Option<PathBuf>,
    pub commit_sha: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SourceKind {
    MessageFile,
    Stdin,
    Commit,
    Audit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Violation {
    pub source: Source,
    pub rule_id: String,
    pub field: Option<String>,
    pub line: Option<usize>,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("invalid regex pattern in rule {rule_id}: {pattern}")]
    InvalidPattern {
        rule_id: String,
        pattern: String,
        #[source]
        source: regex::Error,
    },
}

fn parse_trailer_line(line: &str) -> Option<(&str, &str)> {
    let (field, value) = line.split_once(':')?;
    let field = field.trim();
    let value = value.trim();

    if field.is_empty() || value.is_empty() {
        return None;
    }

    Some((field, value))
}

fn matches_pattern(pattern: &str, value: &str, rule_id: &str) -> Result<bool, AnalysisError> {
    let regex = Regex::new(pattern).map_err(|source| AnalysisError::InvalidPattern {
        rule_id: rule_id.to_string(),
        pattern: pattern.to_string(),
        source,
    })?;

    Ok(regex.is_match(value))
}

#[cfg(test)]
mod tests {
    use super::{
        FieldMatcher, Identity, IdentityRole, IdentityRoleMatcher, IdentityRule, Policy, Rule,
        RuleKind, Source, SourceKind, ValueMatcher,
    };

    fn test_source() -> Source {
        Source {
            kind: SourceKind::MessageFile,
            path: None,
            commit_sha: Some("abc123".to_string()),
        }
    }

    #[test]
    fn default_policy_rejects_ai_coauthor_trailer() {
        let policy = Policy::default();
        let violations = policy
            .analyze(test_source(), "Co-authored-by: Codex <codex@example.com>")
            .expect("analysis should succeed");

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "forbidden-ai-coauthor");
    }

    #[test]
    fn default_policy_rejects_made_with_marker() {
        let policy = Policy::default();
        let violations = policy
            .analyze(test_source(), "Made with Cursor")
            .expect("analysis should succeed");

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "forbidden-made-with-marker");
    }

    #[test]
    fn default_policy_rejects_cursor_agent_author_line() {
        let policy = Policy::default();
        let violations = policy
            .analyze(
                test_source(),
                "Author: Cursor Agent <cursoragent@cursor.com>",
            )
            .expect("analysis should succeed");

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "forbidden-author-identity-line");
    }

    #[test]
    fn default_policy_allows_human_coauthor() {
        let policy = Policy::default();
        let violations = policy
            .analyze(test_source(), "Co-authored-by: Jane Doe <jane@example.com>")
            .expect("analysis should succeed");

        assert!(violations.is_empty());
    }

    #[test]
    fn default_policy_allows_coauthor_with_ai_substring_in_name_or_email() {
        let policy = Policy::default();
        let messages = [
            "Co-authored-by: Jane Doe <jane@gmail.com>",
            "Co-authored-by: Kai Smith <kai@example.com>",
            "Co-authored-by: Claire Jones <claire@example.com>",
            "Co-authored-by: John Doe <john@mail.example.com>",
        ];

        for message in messages {
            let violations = policy
                .analyze(test_source(), message)
                .expect("analysis should succeed");
            assert!(
                violations.is_empty(),
                "expected no violations for: {message}"
            );
        }
    }

    #[test]
    fn default_policy_rejects_standalone_ai_coauthor() {
        let policy = Policy::default();
        let messages = [
            "Co-authored-by: AI <ai@example.com>",
            "Co-authored-by: ai-bot <ai@example.com>",
            "Co-authored-by: AI Assistant <ai@example.com>",
        ];

        for message in messages {
            let violations = policy
                .analyze(test_source(), message)
                .expect("analysis should succeed");
            assert_eq!(violations.len(), 1, "expected one violation for: {message}");
            assert_eq!(violations[0].rule_id, "forbidden-ai-coauthor");
        }
    }

    #[test]
    fn default_policy_allows_allowed_provenance_marker() {
        let policy = Policy::default();
        let violations = policy
            .analyze(test_source(), "AI-Assisted: true")
            .expect("analysis should succeed");

        assert!(violations.is_empty());
    }

    #[test]
    fn freeform_matching_does_not_flag_normal_prose() {
        let policy = Policy::default();
        let violations = policy
            .analyze(test_source(), "The fix was made with care.")
            .expect("analysis should succeed");

        assert!(violations.is_empty());
    }

    #[test]
    fn structured_violation_contains_source_line_and_commit_sha() {
        let policy = Policy::default();
        let source = test_source();
        let violations = policy
            .analyze(
                source,
                "subject line\nCo-authored-by: Codex <codex@example.com>",
            )
            .expect("analysis should succeed");

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].line, Some(2));
        assert_eq!(violations[0].source.kind, SourceKind::MessageFile);
        assert_eq!(violations[0].source.commit_sha.as_deref(), Some("abc123"));
        assert_eq!(violations[0].field.as_deref(), Some("Co-authored-by"));
    }

    #[test]
    fn forbidden_rule_wins_over_allowed_provenance_key() {
        let policy = Policy {
            rules: vec![Rule {
                id: "forbidden-generated-by-codex".to_string(),
                kind: RuleKind::ForbiddenTrailer,
                field_matcher: FieldMatcher::Exact("Generated-by".to_string()),
                value_matcher: ValueMatcher::Pattern("(?i)codex".to_string()),
                message: "Generated-by Codex is not allowed".to_string(),
            }],
            identity_rules: vec![],
            allowed_provenance_keys: vec!["Generated-by".to_string()],
        };

        let violations = policy
            .analyze(test_source(), "Generated-by: Codex")
            .expect("analysis should succeed");

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "forbidden-generated-by-codex");
    }

    #[test]
    fn allowed_provenance_key_passes_without_forbidden_match() {
        let policy = Policy {
            rules: vec![Rule {
                id: "forbidden-generated-by-codex".to_string(),
                kind: RuleKind::ForbiddenTrailer,
                field_matcher: FieldMatcher::Exact("Generated-by".to_string()),
                value_matcher: ValueMatcher::Pattern("(?i)codex".to_string()),
                message: "Generated-by Codex is not allowed".to_string(),
            }],
            identity_rules: vec![],
            allowed_provenance_keys: vec!["Generated-by".to_string()],
        };

        let violations = policy
            .analyze(test_source(), "Generated-by: internal-build-script")
            .expect("analysis should succeed");

        assert!(violations.is_empty());
    }

    #[test]
    fn default_policy_rejects_cursor_agent_author_identity() {
        let policy = Policy::default();
        let violations = policy
            .analyze_identity(
                test_source(),
                &Identity {
                    role: IdentityRole::Author,
                    name: "Cursor Agent".to_string(),
                    email: "cursoragent@cursor.com".to_string(),
                },
            )
            .expect("identity analysis should succeed");

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_id, "forbidden-ai-git-identity");
        assert_eq!(violations[0].field.as_deref(), Some("author.name"));
    }

    #[test]
    fn default_policy_rejects_ai_committer_identity() {
        let policy = Policy::default();
        let violations = policy
            .analyze_identity(
                test_source(),
                &Identity {
                    role: IdentityRole::Committer,
                    name: "Release Bot".to_string(),
                    email: "codex@example.com".to_string(),
                },
            )
            .expect("identity analysis should succeed");

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].field.as_deref(), Some("committer.email"));
    }

    #[test]
    fn default_policy_allows_human_author_identity() {
        let policy = Policy::default();
        let violations = policy
            .analyze_identity(
                test_source(),
                &Identity {
                    role: IdentityRole::Author,
                    name: "Jane Doe".to_string(),
                    email: "jane@example.com".to_string(),
                },
            )
            .expect("identity analysis should succeed");

        assert!(violations.is_empty());
    }

    #[test]
    fn identity_rule_can_target_only_author_role() {
        let policy = Policy {
            rules: vec![],
            identity_rules: vec![IdentityRule {
                id: "forbidden-author-agent".to_string(),
                role_matcher: IdentityRoleMatcher::Role(IdentityRole::Author),
                name_pattern: Some("(?i)agent".to_string()),
                email_pattern: None,
                message: "author agent is not allowed".to_string(),
            }],
            allowed_provenance_keys: vec![],
        };

        let committer_violations = policy
            .analyze_identity(
                test_source(),
                &Identity {
                    role: IdentityRole::Committer,
                    name: "Agent Smith".to_string(),
                    email: "agent@example.com".to_string(),
                },
            )
            .expect("identity analysis should succeed");
        let author_violations = policy
            .analyze_identity(
                test_source(),
                &Identity {
                    role: IdentityRole::Author,
                    name: "Agent Smith".to_string(),
                    email: "agent@example.com".to_string(),
                },
            )
            .expect("identity analysis should succeed");

        assert!(committer_violations.is_empty());
        assert_eq!(author_violations.len(), 1);
    }
}
