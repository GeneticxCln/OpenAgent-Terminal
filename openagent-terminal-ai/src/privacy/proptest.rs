use super::*;
use proptest::prelude::*;
use regex::Regex;

// Generate realistic test data for property-based testing
prop_compose! {
    fn arb_api_key()(
        prefix in "(?:api[_-]?key|token|secret|password)",
        separator in r"[:=]\s*",
        quotes in prop::option::of(r#"[\"\']"#),
        key in r"[A-Za-z0-9_\-\.]+{16,64}"
    ) -> String {
        match quotes {
            Some(q) => format!("{}{}{}{}{}", prefix, separator, q, key, q),
            None => format!("{}{}{}", prefix, separator, key)
        }
    }
}

prop_compose! {
    fn arb_github_token()(
        prefix in "gh[pours]_",
        token in r"[a-zA-Z0-9]{36}"
    ) -> String {
        format!("{}{}", prefix, token)
    }
}

prop_compose! {
    fn arb_jwt_token()(
        header in "eyJ[A-Za-z0-9_-]+",
        payload in "eyJ[A-Za-z0-9_-]+",
        signature in "[A-Za-z0-9_-]+"
    ) -> String {
        format!("{}.{}.{}", header, payload, signature)
    }
}

prop_compose! {
    fn arb_aws_access_key()(
        key in "AKIA[0-9A-Z]{16}"
    ) -> String {
        key
    }
}

prop_compose! {
    fn arb_connection_string()(
        protocol in "(?:mongodb|postgres|postgresql|mysql|redis|amqp)",
        username in r"[a-zA-Z0-9_]{3,16}",
        password in r"[a-zA-Z0-9_@#$%^&*!]{8,32}",
        host in r"[a-zA-Z0-9\.\-]{3,64}",
        port in 1024u16..65535u16
    ) -> String {
        format!("{}://{}:{}@{}:{}/database", protocol, username, password, host, port)
    }
}

prop_compose! {
    fn arb_command_with_password()(
        command in r"(?:mysql|psql|pg_dump|redis-cli)",
        username in r"[a-zA-Z0-9_]{3,16}",
        password in r"[a-zA-Z0-9_@#$%^&*!]{8,32}",
        flag_style in prop::bool::ANY
    ) -> String {
        if flag_style {
            format!("{} -u {} --password={}", command, username, password)
        } else {
            format!("{} -u {} -p {}", command, username, password)
        }
    }
}

proptest! {
    #[test]
    fn test_api_keys_always_redacted(key in arb_api_key()) {
        let original = format!("Configuration: {}", key);
        let redacted = redact_secrets(&original);
        
        // Should not contain the original key
        prop_assert!(!redacted.contains(&extract_key_from_pattern(&key)));
        
        // Should contain some form of redaction marker
        prop_assert!(redacted.contains("[REDACTED]"));
    }
    
    #[test]
    fn test_github_tokens_always_redacted(token in arb_github_token()) {
        let original = format!("export GITHUB_TOKEN={}", token);
        let redacted = redact_secrets(&original);
        
        // Should not contain the original token
        prop_assert!(!redacted.contains(&token));
        
        // Should contain GitHub-specific redaction
        prop_assert!(redacted.contains("[REDACTED_GITHUB"));
    }
    
    #[test]
    fn test_jwt_tokens_always_redacted(jwt in arb_jwt_token()) {
        let original = format!("Authorization: Bearer {}", jwt);
        let redacted = redact_secrets(&original);
        
        // Should not contain the original JWT
        prop_assert!(!redacted.contains(&jwt));
        
        // Should contain JWT-specific redaction
        prop_assert!(redacted.contains("[REDACTED_JWT]") || redacted.contains("[REDACTED]"));
    }
    
    #[test]
    fn test_aws_keys_always_redacted(aws_key in arb_aws_access_key()) {
        let original = format!("AWS_ACCESS_KEY_ID={}", aws_key);
        let redacted = redact_secrets(&original);
        
        // Should not contain the original key
        prop_assert!(!redacted.contains(&aws_key));
        
        // Should contain AWS-specific redaction
        prop_assert!(redacted.contains("[REDACTED_AWS"));
    }
    
    #[test]
    fn test_connection_strings_always_redacted(conn_str in arb_connection_string()) {
        let original = format!("Database connection: {}", conn_str);
        let redacted = redact_secrets(&original);
        
        // Should not contain the original password (extract from connection string)
        let password = extract_password_from_connection(&conn_str);
        prop_assert!(!redacted.contains(&password));
        
        // Should contain some redaction marker
        prop_assert!(redacted.contains("[REDACTED]") || redacted.contains("[USER]"));
    }
    
    #[test]
    fn test_command_passwords_always_redacted(cmd in arb_command_with_password()) {
        let original = format!("Running: {}", cmd);
        let redacted = redact_secrets(&original);
        
        // Should not contain the original password
        let password = extract_password_from_command(&cmd);
        prop_assert!(!redacted.contains(&password));
        
        // Should contain redaction marker
        prop_assert!(redacted.contains("[REDACTED]"));
    }
    
    #[test]
    fn test_redaction_preserves_non_secrets(
        text in r"[a-zA-Z0-9 \.\-_/]{10,100}",
        non_secret_key in r"(?:name|host|port|database|table|user)",
        non_secret_value in r"[a-zA-Z0-9\.\-_]{3,20}"
    ) {
        // Create text that looks like config but isn't secret
        let original = format!("{} {}={} more text", text, non_secret_key, non_secret_value);
        let redacted = redact_secrets(&original);
        
        // Non-secret parts should be preserved
        prop_assert!(redacted.contains(&text));
        prop_assert!(redacted.contains(&non_secret_key));
        prop_assert!(redacted.contains(&non_secret_value));
    }
    
    #[test]
    fn test_mixed_content_redaction(
        secret in arb_api_key(),
        safe_text in r"[a-zA-Z0-9 \.\-_]{20,50}",
        safe_config in r"(?:host|port|database)=[a-zA-Z0-9\.\-_]{3,20}"
    ) {
        let original = format!("{} {} {}", safe_text, secret, safe_config);
        let redacted = redact_secrets(&original);
        
        // Safe parts should be preserved
        prop_assert!(redacted.contains(&safe_text));
        
        // Secret should be redacted
        let key = extract_key_from_pattern(&secret);
        prop_assert!(!redacted.contains(&key));
        prop_assert!(redacted.contains("[REDACTED]"));
    }
}

// Helper functions to extract secrets from generated patterns
fn extract_key_from_pattern(pattern: &str) -> String {
    if let Some(cap) = Regex::new(r"[:=]\s*[\"']?([^\"'\s]+)[\"']?").unwrap().captures(pattern) {
        cap.get(1).unwrap().as_str().to_string()
    } else {
        pattern.to_string()
    }
}

fn extract_password_from_connection(conn_str: &str) -> String {
    if let Some(cap) = Regex::new(r"://[^:]+:([^@]+)@").unwrap().captures(conn_str) {
        cap.get(1).unwrap().as_str().to_string()
    } else {
        String::new()
    }
}

fn extract_password_from_command(cmd: &str) -> String {
    // Try to extract password from various command formats
    for pattern in [
        r"--password=([^\s]+)",
        r"--password\s+([^\s]+)", 
        r"-p\s+([^\s]+)",
    ] {
        if let Some(cap) = Regex::new(pattern).unwrap().captures(cmd) {
            return cap.get(1).unwrap().as_str().to_string();
        }
    }
    String::new()
}

#[cfg(test)]
mod fuzzing_tests {
    use super::*;
    
    proptest! {
        #[test]
        fn test_redaction_never_panics(
            input in r"[\s\S]{0,1000}"
        ) {
            // Redaction should never panic, even on malformed input
            let _result = redact_secrets(&input);
        }
        
        #[test] 
        fn test_redaction_doesnt_expand_input_significantly(
            input in r"[\s\S]{0,500}"
        ) {
            let redacted = redact_secrets(&input);
            // Redacted text shouldn't be more than 3x original length
            // This prevents redaction from causing memory issues
            prop_assert!(redacted.len() <= input.len() * 3);
        }
        
        #[test]
        fn test_double_redaction_is_idempotent(
            input in r"[\s\S]{0,200}"
        ) {
            let once = redact_secrets(&input);
            let twice = redact_secrets(&once);
            // Redacting twice should not change anything
            prop_assert_eq!(once, twice);
        }
    }
}

// Additional targeted tests for edge cases
#[cfg(test)]
mod edge_case_tests {
    use super::*;
    
    proptest! {
        #[test]
        fn test_empty_and_whitespace_handling(
            whitespace in r"[\s]{0,20}"
        ) {
            let redacted = redact_secrets(&whitespace);
            // Whitespace-only input should remain unchanged
            prop_assert_eq!(redacted, whitespace);
        }
        
        #[test]
        fn test_very_long_keys_redacted(
            prefix in "api[_-]?key[:=]\\s*",
            key in r"[A-Za-z0-9_\-\.]{64,200}" // Very long key
        ) {
            let input = format!("{}{}", prefix, key);
            let redacted = redact_secrets(&input);
            prop_assert!(!redacted.contains(&key));
        }
        
        #[test]
        fn test_unicode_in_non_secret_content(
            unicode_text in r"[α-ωΑ-Ω가-힣一-龯]{5,20}",
            separator in r"[\s\-_\.]{1,3}",
            normal_text in r"[a-zA-Z0-9]{5,20}"
        ) {
            let input = format!("{}{}{}", unicode_text, separator, normal_text);
            let redacted = redact_secrets(&input);
            // Unicode content should be preserved if not part of secrets
            prop_assert!(redacted.contains(&unicode_text));
            prop_assert!(redacted.contains(&normal_text));
        }
    }
}