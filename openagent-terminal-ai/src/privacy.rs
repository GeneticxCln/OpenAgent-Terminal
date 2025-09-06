use crate::AiRequest;
use regex::Regex;

/// Options controlling prompt/data sanitization before sending to providers.
#[derive(Debug, Clone, Copy)]
pub struct AiPrivacyOptions {
    pub strip_sensitive: bool,
    pub strip_cwd: bool,
}

impl Default for AiPrivacyOptions {
    fn default() -> Self {
        Self { strip_sensitive: true, strip_cwd: true }
    }
}

impl AiPrivacyOptions {
    /// Read options from environment variables.
    /// OPENAGENT_AI_STRIP_SENSITIVE: default "1"
    /// OPENAGENT_AI_STRIP_CWD: default "1"
    pub fn from_env() -> Self {
        let strip_sensitive =
            std::env::var("OPENAGENT_AI_STRIP_SENSITIVE").ok().map(|v| v != "0").unwrap_or(true);
        let strip_cwd =
            std::env::var("OPENAGENT_AI_STRIP_CWD").ok().map(|v| v != "0").unwrap_or(true);
        Self { strip_sensitive, strip_cwd }
    }
}

/// Sanitize an AI request by redacting paths and sensitive values.
pub fn sanitize_request(req: &AiRequest, opts: AiPrivacyOptions) -> AiRequest {
    let mut sanitized = req.clone();

    // Redact working directory from scratch text and context.
    if let Some(dir) = &req.working_directory {
        if opts.strip_cwd {
            let placeholder = "[REDACTED_PATH]";
            sanitized.scratch_text = sanitized.scratch_text.replace(dir, placeholder);
            // Replace common home path in text if present.
            if let Ok(home) = std::env::var("HOME") {
                if !home.is_empty() {
                    sanitized.scratch_text = sanitized.scratch_text.replace(&home, placeholder);
                }
            }
            // Replace the precise working directory value with a generic redaction marker
            sanitized.working_directory = Some("[REDACTED]".to_string());
        }
    }

    // Redact obvious secrets patterns from scratch text.
    if opts.strip_sensitive {
        sanitized.scratch_text = redact_secrets(&sanitized.scratch_text);
    }

    // Redact sensitive entries in context
    if opts.strip_sensitive {
        sanitized.context = req
            .context
            .iter()
            .map(|(k, v)| {
                if is_sensitive_key(k) {
                    (k.clone(), "[REDACTED]".to_string())
                } else {
                    (k.clone(), v.clone())
                }
            })
            .collect();
    }

    sanitized
}

fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    ["key", "token", "secret", "password", "apikey", "api_key", "auth", "credential"]
        .iter()
        .any(|kw| lower.contains(kw))
}

/// Comprehensive secret redaction function
fn redact_secrets(text: &str) -> String {
    let patterns = vec![
        // Command-line flags with secrets (match these first to avoid generic kv rewriting)
        (
            r#"(?i)--(password|token|key|secret|auth|api[_-]?key)\s*=?\s*(?:[\"']?)([^\s\"']+)(?:[\"']?)"#,
            "--$1=[REDACTED]",
        ),
        (r#"(?i)-(p|P)\s+([^\s]+)"#, "-$1 [REDACTED]"), // Common password flag
        // Specific tokens should be redacted before generic ones
        // GitHub tokens
        (r#"\b(ghp_[a-zA-Z0-9]{36})\b"#, "[REDACTED_GITHUB_TOKEN]"),
        (r#"\b(gho_[a-zA-Z0-9]{36})\b"#, "[REDACTED_GITHUB_OAUTH]"),
        (r#"\b(ghu_[a-zA-Z0-9]{36})\b"#, "[REDACTED_GITHUB_USER]"),
        (r#"\b(ghs_[a-zA-Z0-9]{36})\b"#, "[REDACTED_GITHUB_SERVER]"),
        (r#"\b(ghr_[a-zA-Z0-9]{36})\b"#, "[REDACTED_GITHUB_REFRESH]"),
        // Env-var style API keys (e.g., OPENAI_API_KEY=...)
        (r#"(?i)\b([A-Z_]*api[_-]?key)\s*[:=]\s*\S+"#, "$1: [REDACTED]"),
        // API keys and tokens (generic) — only when preceded by start or whitespace
        (
            r#"(?i)(^|\s)(api[_-]?key|token|secret|password|auth|credential)\s*[:=]\s*(?:[\"']?)[^\s\"'\[][^\s\"']*(?:[\"']?)"#,
            "$1$2: [REDACTED]",
        ),
        // JWT tokens (xxx.yyy.zzz format)
        (r#"\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b"#, "[REDACTED_JWT]"),
        // AWS credentials
        (r#"\b(AKIA[0-9A-Z]{16})\b"#, "[REDACTED_AWS_ACCESS_KEY]"),
        (r#"\b([A-Za-z0-9/+=]{40})\b"#, "[REDACTED_AWS_SECRET_KEY]"),
        (r#"aws_access_key_id\s*=\s*([^\s]+)"#, "aws_access_key_id = [REDACTED]"),
        (r#"aws_secret_access_key\s*=\s*([^\s]+)"#, "aws_secret_access_key = [REDACTED]"),
        // GitHub tokens
        (r#"\b(ghp_[a-zA-Z0-9]{36})\b"#, "[REDACTED_GITHUB_TOKEN]"),
        (r#"\b(gho_[a-zA-Z0-9]{36})\b"#, "[REDACTED_GITHUB_OAUTH]"),
        (r#"\b(ghu_[a-zA-Z0-9]{36})\b"#, "[REDACTED_GITHUB_USER]"),
        (r#"\b(ghs_[a-zA-Z0-9]{36})\b"#, "[REDACTED_GITHUB_SERVER]"),
        (r#"\b(ghr_[a-zA-Z0-9]{36})\b"#, "[REDACTED_GITHUB_REFRESH]"),
        // GCP Service Account keys (base64 encoded JSON)
        (
            r#"\"private_key\"\s*:\s*\"-----BEGIN [^\"]+-----[^\"]+-----END [^\"]+-----\""#,
            r#""private_key": "[REDACTED_PRIVATE_KEY]""#,
        ),
        (
            r#"\"client_email\"\s*:\s*\"[^@]+@[^.]+\.iam\.gserviceaccount\.com\""#,
            r#""client_email": "[REDACTED_SERVICE_ACCOUNT]""#,
        ),
        // SSH private keys
        (
            r#"-----BEGIN (RSA |DSA |EC |OPENSSH )?PRIVATE KEY-----[\s\S]+?-----END (RSA |DSA |EC |OPENSSH )?PRIVATE KEY-----"#,
            "[REDACTED_SSH_PRIVATE_KEY]",
        ),
        // Bearer tokens in headers
        (r#"(?i)authorization:\s*bearer\s+([^\s]+)"#, "Authorization: Bearer [REDACTED]"),
        (r#"(?i)x-api-key:\s*([^\s]+)"#, "X-API-Key: [REDACTED]"),
        // Database connection strings
        (
            r#"(mongodb|postgres|postgresql|mysql|redis|amqp)://[^:]+:([^@]+)@"#,
            "$1://[USER]:[REDACTED]@",
        ),
        // Environment variable exports
        (
            r#"(?i)export\s+(.*(?:KEY|TOKEN|SECRET|PASSWORD|AUTH).*)=([^\s]+)"#,
            "export $1=[REDACTED]",
        ),
        // Slack tokens
        (r#"xox[baprs]-[0-9]{10,}-[0-9]{10,}-[a-zA-Z0-9]{24,}"#, "[REDACTED_SLACK_TOKEN]"),
        // Generic UUIDs that might be sensitive
        (
            r#"(?i)(session[_-]?id|csrf[_-]?token)\s*[:=]\s*(?:[\"']?)[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}(?:[\"']?)"#,
            "$1: [REDACTED_UUID]",
        ),
    ];

    let mut result = text.to_string();

    for (pattern, replacement) in patterns {
        if let Ok(re) = Regex::new(pattern) {
            result = re.replace_all(&result, replacement).into_owned();
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_jwt() {
        let text = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.\
                    eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.\
                    SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let redacted = redact_secrets(text);
        assert!(redacted.contains("[REDACTED_JWT]"));
    }

    #[test]
    fn test_redact_aws_key() {
        let text = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE";
        let redacted = redact_secrets(text);
        assert!(redacted.contains("[REDACTED_AWS_ACCESS_KEY]"));
    }

    #[test]
    fn test_redact_github_token() {
        let text = "token: ghp_abcdefghijklmnopqrstuvwxyz0123456789";
        let redacted = redact_secrets(text);
        assert!(redacted.contains("[REDACTED_GITHUB_TOKEN]"));
    }

    #[test]
    fn test_redact_command_flags() {
        let text = "mysql -u admin --password=secret123 -h localhost";
        let redacted = redact_secrets(text);
        assert!(redacted.contains("--password=[REDACTED]"));
    }

    #[test]
    fn test_redact_connection_string() {
        let text = "mongodb://user:password123@localhost:27017/db";
        let redacted = redact_secrets(text);
        assert!(redacted.contains("[REDACTED]"));
        assert!(!redacted.contains("password123"));
    }

    #[test]
    fn test_multiline_and_shell_flags() {
        let text = r#"pg_dump -U user -h host -p 5432 --password secret
curl -H "Authorization: Bearer abc123" https://example.com
export OPENAI_API_KEY=sk-abcdef
mysql -u root -p hunter2
"#;
        let redacted = redact_secrets(text);
        // --password redacted
        assert!(redacted.contains("--password=[REDACTED]"));
        // Header redacted
        assert!(redacted.contains("Authorization: Bearer [REDACTED]"));
        // Env-style API key redacted
        assert!(
            redacted.contains("OPENAI_API_KEY: [REDACTED]")
                || redacted.contains("OPENAI_API_KEY=[REDACTED]")
        );
        // Short -p flag with value redacted
        assert!(redacted.contains("-p [REDACTED]"));
    }

    #[test]
    fn test_sanitize_request_redacts_working_dir_and_home() {
        let req = AiRequest {
            scratch_text: "/home/alice/projects/demo: run --password=foo".to_string(),
            working_directory: Some("/home/alice/projects/demo".to_string()),
            shell_kind: Some("bash".to_string()),
            context: vec![("OPENAI_API_KEY".into(), "sk-abc".into())],
        };
        // Set HOME for test
        std::env::set_var("HOME", "/home/alice");
        let out = sanitize_request(&req, AiPrivacyOptions::default());
        // Working directory should be redacted
        assert!(out.scratch_text.contains("[REDACTED_PATH]"));
        // HOME path in scratch should be redacted as path
        assert!(!out.scratch_text.contains("/home/alice"));
        // Working directory field should be redacted
        assert_eq!(out.working_directory.as_deref(), Some("[REDACTED]"));
        // Context secret redacted
        assert!(out.context.iter().any(|(k, v)| k == "OPENAI_API_KEY" && v == "[REDACTED]"));
    }

    #[test]
    fn test_multiline_quotes_and_mixed_flags() {
        let text = r#"psql --host=localhost --username=alice --password='s3cr3t'
mysqldump -p hunter2 \
  --result-file=/tmp/backup.sql
"#;
        let redacted = redact_secrets(text);
        // Both styles should be redacted
        assert!(redacted.contains("--password=[REDACTED]"));
        assert!(redacted.contains("-p [REDACTED]"));
        // Ensure the backup path remains
        assert!(redacted.contains("--result-file=/tmp/backup.sql"));
    }
}
