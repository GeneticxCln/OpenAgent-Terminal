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
        let strip_sensitive = std::env::var("OPENAGENT_AI_STRIP_SENSITIVE")
            .ok()
            .map(|v| v != "0")
            .unwrap_or(true);
        let strip_cwd = std::env::var("OPENAGENT_AI_STRIP_CWD")
            .ok()
            .map(|v| v != "0")
            .unwrap_or(true);
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
            sanitized.working_directory = Some(placeholder.to_string());
        }
    }

    // Redact obvious secrets patterns from scratch text.
    if opts.strip_sensitive {
        let patterns = vec![
            // key=value or key: value with common secret names.
            r"(?i)\b(api[_-]?key|token|secret|password)\s*[:=]\s*([\"']?)[^\s\"']+\2",
        ];
        for pat in patterns {
            if let Ok(re) = Regex::new(pat) {
                sanitized.scratch_text = re.replace_all(&sanitized.scratch_text, |caps: &regex::Captures| {
                    // Keep the key name, redact the value.
                    format!("{}: [REDACTED]", &caps[1])
                }).into_owned();
            }
        }
    }

    // Redact sensitive entries in context
    if opts.strip_sensitive {
        sanitized.context = req.context.iter().map(|(k, v)| {
            if is_sensitive_key(k) {
                (k.clone(), "[REDACTED]".to_string())
            } else {
                (k.clone(), v.clone())
            }
        }).collect();
    }

    sanitized
}

fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    ["key", "token", "secret", "password", "apikey", "api_key"]
        .iter()
        .any(|kw| lower.contains(kw))
}

