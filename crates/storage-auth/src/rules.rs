use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::claims::Claims;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecurityRules {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub path: String,
    pub read: RuleCondition,
    pub write: RuleCondition,
    pub delete: RuleCondition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuleCondition {
    Bool(bool),
    Expression(String),
}

impl Default for RuleCondition {
    fn default() -> Self {
        RuleCondition::Bool(false)
    }
}

impl Rule {
    pub fn allow_all(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            read: RuleCondition::Bool(true),
            write: RuleCondition::Bool(true),
            delete: RuleCondition::Bool(true),
        }
    }

    pub fn authenticated_only(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            read: RuleCondition::Expression("request.auth != null".to_string()),
            write: RuleCondition::Expression("request.auth != null".to_string()),
            delete: RuleCondition::Expression("request.auth.admin == true".to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AccessRequest {
    pub operation: Operation,
    pub bucket: String,
    pub object_path: String,
    pub claims: Option<Claims>,
    pub custom_metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Read,
    Write,
    Delete,
}

pub struct SecurityRulesEngine {
    rules: SecurityRules,
}

impl SecurityRulesEngine {
    pub fn new(rules: SecurityRules) -> Self {
        Self { rules }
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let rules: SecurityRules = serde_json::from_str(json)?;
        Ok(Self::new(rules))
    }

    pub fn default_rules() -> Self {
        Self::new(SecurityRules {
            rules: vec![
                Rule::allow_all("public/**"),
                Rule::authenticated_only("private/**"),
            ],
        })
    }

    pub fn evaluate(&self, request: &AccessRequest) -> bool {
        if let Some(claims) = &request.claims {
            if claims.is_admin() {
                return true;
            }
        }

        let object_path = &request.object_path;

        for rule in &self.rules.rules {
            if Self::path_matches(&rule.path, object_path) {
                return Self::evaluate_condition(
                    match request.operation {
                        Operation::Read => &rule.read,
                        Operation::Write => &rule.write,
                        Operation::Delete => &rule.delete,
                    },
                    request,
                );
            }
        }

        false
    }

    fn path_matches(pattern: &str, path: &str) -> bool {
        if pattern == "**" || pattern == "*" {
            return true;
        }

        if pattern.ends_with("/**") {
            let prefix = &pattern[..pattern.len() - 3];
            return path.starts_with(prefix);
        }

        if pattern.contains('*') {
            let parts: Vec<&str> = pattern.split('*').collect();
            if parts.len() == 2 {
                return path.starts_with(parts[0]) && path.ends_with(parts[1]);
            }
        }

        pattern == path
    }

    fn evaluate_condition(condition: &RuleCondition, request: &AccessRequest) -> bool {
        match condition {
            RuleCondition::Bool(b) => *b,
            RuleCondition::Expression(expr) => Self::evaluate_expression(expr, request),
        }
    }

    fn evaluate_expression(expr: &str, request: &AccessRequest) -> bool {
        let expr = expr.trim();

        if expr == "request.auth != null" {
            return request.claims.is_some();
        }

        if expr == "request.auth.admin == true" {
            return request
                .claims
                .as_ref()
                .map(|c| c.admin)
                .unwrap_or(false);
        }

        if expr.starts_with("request.auth.uid == '") && expr.ends_with('\'') {
            let uid = &expr[20..expr.len() - 1];
            return request
                .claims
                .as_ref()
                .map(|c| c.uid == uid)
                .unwrap_or(false);
        }

        if expr.contains("resource.metadata") {
            if let Some((key, expected)) = expr
                .split_once("==")
                .map(|(k, v)| (k.trim(), v.trim().trim_matches('\'')))
            {
                let meta_key = key
                    .trim()
                    .strip_prefix("resource.metadata.")
                    .unwrap_or(key);
                return request
                    .custom_metadata
                    .get(meta_key)
                    .map(|v| v == expected)
                    .unwrap_or(false);
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_matching() {
        assert!(SecurityRulesEngine::path_matches("public/**", "public/images/photo.jpg"));
        assert!(SecurityRulesEngine::path_matches("public/**", "public/file.txt"));
        assert!(!SecurityRulesEngine::path_matches("public/**", "private/file.txt"));
    }

    #[test]
    fn test_public_read() {
        let engine = SecurityRulesEngine::default_rules();
        let req = AccessRequest {
            operation: Operation::Read,
            bucket: "my-bucket".to_string(),
            object_path: "public/photo.jpg".to_string(),
            claims: None,
            custom_metadata: HashMap::new(),
        };
        assert!(engine.evaluate(&req));
    }

    #[test]
    fn test_private_requires_auth() {
        let engine = SecurityRulesEngine::default_rules();
        let req = AccessRequest {
            operation: Operation::Write,
            bucket: "my-bucket".to_string(),
            object_path: "private/doc.pdf".to_string(),
            claims: None,
            custom_metadata: HashMap::new(),
        };
        assert!(!engine.evaluate(&req));

        let req_auth = AccessRequest {
            claims: Some(Claims::new("user1", Some("user@example.com".to_string()), false)),
            ..req
        };
        assert!(engine.evaluate(&req_auth));
    }
}
