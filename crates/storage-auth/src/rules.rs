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
            if let Some(path_params) = Self::path_matches(&rule.path, object_path) {
                return Self::evaluate_condition(
                    match request.operation {
                        Operation::Read => &rule.read,
                        Operation::Write => &rule.write,
                        Operation::Delete => &rule.delete,
                    },
                    request,
                    &path_params,
                );
            }
        }

        false
    }

    /// Compara o padrão ao caminho do objeto, capturando parâmetros `{nome}`.
    fn path_matches(pattern: &str, path: &str) -> Option<HashMap<String, String>> {
        if pattern == "**" || pattern == "*" {
            return Some(HashMap::new());
        }

        let (pattern_base, allow_rest) = if let Some(base) = pattern.strip_suffix("/**") {
            (base, true)
        } else {
            (pattern, false)
        };

        Self::match_segments(pattern_base, path, allow_rest)
    }

    fn match_segments(
        pattern: &str,
        path: &str,
        allow_rest: bool,
    ) -> Option<HashMap<String, String>> {
        let pattern_parts: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
        let path_parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        if allow_rest {
            if path_parts.len() < pattern_parts.len() {
                return None;
            }
        } else if pattern_parts.len() != path_parts.len() {
            return None;
        }

        let mut params = HashMap::new();

        for (i, pat) in pattern_parts.iter().enumerate() {
            let seg = path_parts.get(i)?;

            if let Some(key) = pat.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                params.insert(key.to_string(), (*seg).to_string());
            } else if *pat != *seg {
                return None;
            }
        }

        Some(params)
    }

    fn evaluate_condition(
        condition: &RuleCondition,
        request: &AccessRequest,
        path_params: &HashMap<String, String>,
    ) -> bool {
        match condition {
            RuleCondition::Bool(b) => *b,
            RuleCondition::Expression(expr) => {
                Self::evaluate_expression(expr, request, path_params)
            }
        }
    }

    fn substitute_params(expr: &str, path_params: &HashMap<String, String>) -> String {
        let mut result = expr.to_string();
        for (key, value) in path_params {
            result = result.replace(&format!("{{{key}}}"), value);
        }
        result
    }

    fn evaluate_expression(
        expr: &str,
        request: &AccessRequest,
        path_params: &HashMap<String, String>,
    ) -> bool {
        let expr = Self::substitute_params(expr.trim(), path_params);

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

        if let Some(rest) = expr.strip_prefix("request.auth.uid == '") {
            if let Some(uid) = rest.strip_suffix('\'') {
                return request
                    .claims
                    .as_ref()
                    .map(|c| c.uid == uid)
                    .unwrap_or(false);
            }
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

    fn engine_from_config() -> SecurityRulesEngine {
        SecurityRulesEngine::from_json(include_str!(
            "../../../config/security_rules.json"
        ))
        .expect("security_rules.json válido")
    }

    #[test]
    fn test_path_matching_public() {
        assert!(SecurityRulesEngine::path_matches("public/**", "public/images/photo.jpg").is_some());
        assert!(SecurityRulesEngine::path_matches("public/**", "public/file.txt").is_some());
        assert!(SecurityRulesEngine::path_matches("public/**", "private/file.txt").is_none());
    }

    #[test]
    fn test_path_matching_users_wildcard() {
        let params =
            SecurityRulesEngine::path_matches("users/{userId}/**", "users/abc123/avatar.jpg")
                .expect("deve casar");
        assert_eq!(params.get("userId").map(String::as_str), Some("abc123"));
    }

    #[test]
    fn test_users_upload_owner_allowed() {
        let engine = engine_from_config();
        let req = AccessRequest {
            operation: Operation::Write,
            bucket: "b".to_string(),
            object_path: "users/user-1/photo.jpg".to_string(),
            claims: Some(Claims::from_firebase("user-1", None, false)),
            custom_metadata: HashMap::new(),
        };
        assert!(engine.evaluate(&req));
    }

    #[test]
    fn test_users_upload_other_denied() {
        let engine = engine_from_config();
        let req = AccessRequest {
            operation: Operation::Write,
            bucket: "b".to_string(),
            object_path: "users/other-user/photo.jpg".to_string(),
            claims: Some(Claims::from_firebase("user-1", None, false)),
            custom_metadata: HashMap::new(),
        };
        assert!(!engine.evaluate(&req));
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
