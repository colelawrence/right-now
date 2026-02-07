//! Capture routines for building snapshots from runtime state
//!
//! Defines the SessionProvider trait contract that session module implements,
//! inverting the dependency to avoid coupling CR to session internals.

use crate::context_resurrection::models::{AttentionSummary, SessionStatus};

/// Snapshot of session state provided by session module
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    /// Session status at snapshot time
    pub status: SessionStatus,
    /// Exit code (if session stopped)
    pub exit_code: Option<i32>,
    /// Last attention state (if any)
    pub last_attention: Option<AttentionSummary>,
    /// Unsanitized terminal tail (capture.rs sanitizes before storing)
    pub tail: String,
}

/// Trait implemented by session module to provide snapshot data
///
/// This inverts the dependency: CR depends on an abstraction, not on session internals.
pub trait SessionProvider: Send + Sync {
    /// Get snapshot of session state
    fn get_session_state(&self, session_id: u64) -> Option<SessionSnapshot>;
}

/// Sanitize terminal output by redacting common secrets (best-effort)
///
/// Patterns redacted:
/// - API_KEY=... / TOKEN=... / SECRET=... (environment variable assignments)
/// - password: ... (case-insensitive, colon-separated)
/// - Authorization: Bearer ...
/// - PEM private keys (-----BEGIN ... PRIVATE KEY-----)
/// - AWS access keys (AKIA...)
///
/// Returns sanitized string with secrets replaced by `[REDACTED]`.
pub fn sanitize_terminal_output(input: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    // Pattern definitions (data-driven for easy extension)
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

    let patterns = PATTERNS.get_or_init(|| {
        vec![
            // API_KEY=value, TOKEN=value, SECRET=value, etc.
            // Matches: API_KEY=abc123, export TOKEN="xyz", SECRET='foo', etc.
            // Match full variable name (including prefixes like GITHUB_TOKEN, AUTH_SECRET)
            Regex::new(r"(?i)\b\w*(API_?KEY|TOKEN|SECRET|PASSWORD|AUTH_?KEY)\s*=\s*\S+").unwrap(),
            // password: value (case-insensitive, colon-separated)
            // Matches: password: secret123, Password: "foo", etc.
            Regex::new(r"(?i)password\s*:\s*\S+").unwrap(),
            // Authorization: Bearer <token>
            // Matches: Authorization: Bearer eyJhbGc..., etc.
            Regex::new(r"(?i)authorization\s*:\s*bearer\s+\S+").unwrap(),
            // PEM private keys (any type: RSA, EC, OPENSSH, etc.)
            // Matches entire key block including header and footer
            Regex::new(r"-----BEGIN[^\n]*PRIVATE KEY-----[\s\S]*?-----END[^\n]*PRIVATE KEY-----")
                .unwrap(),
            // AWS access keys (AKIA followed by 16 alphanumeric chars)
            Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
        ]
    });

    let mut sanitized = input.to_string();
    for pattern in patterns {
        sanitized = pattern.replace_all(&sanitized, "[REDACTED]").to_string();
    }

    sanitized
}

// CaptureService stub (implementation comes in later phases per plan)
// Will implement:
// - capture_now() using SessionProvider trait
// - per-task capture lock (flock) per §1.3.3
// - deduplication window (5s same task_id+reason) per §1.3.3
// - rate limit (1 capture per task per 2s) per §1.3.3

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_api_key_assignments() {
        // Environment variable style assignments
        let input = "export API_KEY=sk_live_abc123xyz";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "export [REDACTED]");

        let input = "API_KEY=\"sk_test_secret_value\"";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "[REDACTED]");

        let input = "APIKEY=my_secret_key";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "[REDACTED]");
    }

    #[test]
    fn test_sanitize_token_assignments() {
        let input = "TOKEN=ghp_abcd1234xyz";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "[REDACTED]");

        let input = "export GITHUB_TOKEN='ghp_secret'";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "export [REDACTED]");
    }

    #[test]
    fn test_sanitize_secret_assignments() {
        let input = "SECRET=my-super-secret";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "[REDACTED]");

        let input = "AUTH_SECRET=\"xyz123\"";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "[REDACTED]");
    }

    #[test]
    fn test_sanitize_password_colon_format() {
        // Case-insensitive password: value
        let input = "password: secret123";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "[REDACTED]");

        let input = "Password: \"my_pass\"";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "[REDACTED]");

        let input = "PASSWORD: admin123";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "[REDACTED]");
    }

    #[test]
    fn test_sanitize_authorization_bearer() {
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "[REDACTED]");

        let input = "authorization: bearer sk_test_123abc";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "[REDACTED]");
    }

    #[test]
    fn test_sanitize_pem_private_keys() {
        let rsa_key = r#"-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEA1234567890abcdef
... more lines ...
-----END RSA PRIVATE KEY-----"#;
        let output = sanitize_terminal_output(rsa_key);
        assert_eq!(output, "[REDACTED]");

        let ec_key = r#"-----BEGIN EC PRIVATE KEY-----
MHcCAQEEIAbcdef1234567890
-----END EC PRIVATE KEY-----"#;
        let output = sanitize_terminal_output(ec_key);
        assert_eq!(output, "[REDACTED]");

        let openssh_key = r#"-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmU=
-----END OPENSSH PRIVATE KEY-----"#;
        let output = sanitize_terminal_output(openssh_key);
        assert_eq!(output, "[REDACTED]");
    }

    #[test]
    fn test_sanitize_aws_access_keys() {
        let input = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "AWS_ACCESS_KEY_ID=[REDACTED]");

        let input = "Found key: AKIA1234567890ABCDEF in logs";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "Found key: [REDACTED] in logs");
    }

    #[test]
    fn test_sanitize_multiple_secrets_in_one_input() {
        let input = r#"
export API_KEY=sk_live_abc123
password: my_secret_pass
Authorization: Bearer eyJhbGc...
AWS key: AKIA1234567890ABCDEF
"#;
        let output = sanitize_terminal_output(input);

        // All secrets should be redacted
        assert!(output.contains("[REDACTED]"));
        assert!(!output.contains("sk_live_abc123"));
        assert!(!output.contains("my_secret_pass"));
        assert!(!output.contains("eyJhbGc"));
        assert!(!output.contains("AKIA1234567890ABCDEF"));
    }

    #[test]
    fn test_sanitize_no_redaction_safe_content() {
        // Input with no secrets should remain unchanged
        let input = "$ cargo build\n   Compiling project v0.1.0\n   Finished dev [unoptimized] target(s) in 2.5s";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, input);

        let input = "Running tests...\ntest result: ok. 42 passed; 0 failed";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, input);

        let input = "API documentation: https://api.example.com/docs";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_sanitize_edge_cases() {
        // Empty input
        assert_eq!(sanitize_terminal_output(""), "");

        // Whitespace only
        assert_eq!(sanitize_terminal_output("   \n\t  "), "   \n\t  ");

        // Mixed safe and unsafe on same line
        let input = "Debug: API_KEY=secret123 and some normal text";
        let output = sanitize_terminal_output(input);
        assert!(output.contains("[REDACTED]"));
        assert!(output.contains("Debug:"));
        assert!(output.contains("and some normal text"));
        assert!(!output.contains("secret123"));
    }

    #[test]
    fn test_sanitize_case_insensitivity() {
        // Verify patterns are case-insensitive where appropriate
        let input = "api_key=secret";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "[REDACTED]");

        let input = "Api_Key=secret";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "[REDACTED]");

        let input = "PASSWORD=secret";
        let output = sanitize_terminal_output(input);
        assert_eq!(output, "[REDACTED]");
    }
}
