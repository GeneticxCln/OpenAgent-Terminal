use proptest::prelude::*;
use regex::Regex;

// Test the public redaction API from the AI crate
use openagent_terminal_ai::privacy::redact_secrets;

fn extract_password_from_connection(conn_str: &str) -> String {
    if let Some(cap) = Regex::new(r"://[^:]+:([^@]+)@").unwrap().captures(conn_str) {
        cap.get(1).unwrap().as_str().to_string()
    } else {
        String::new()
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

proptest! {
    #[test]
    fn redact_connection_passwords(conn in arb_connection_string()) {
        let redacted = redact_secrets(&conn);
        let pwd = extract_password_from_connection(&conn);
        prop_assert!(!pwd.is_empty());
        prop_assert!(!redacted.contains(&pwd));
        prop_assert!(redacted.contains("[REDACTED]"));
    }
}
