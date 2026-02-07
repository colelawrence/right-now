//! Context Resurrection snapshot models (v1 schema)

use serde::{Deserialize, Serialize};

/// Unique identifier for a snapshot
/// Format: "2026-02-06T13:12:33Z_qdz.fix-api-timeout-bug"
pub type SnapshotId = String;

/// Reason why a snapshot was captured
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureReason {
    /// Session transitioned to Stopped state
    SessionStopped,
    /// Session transitioned to Waiting state
    SessionWaiting,
    /// Session transitioned to Running state
    SessionRunning,
    /// Periodic idle timeout triggered capture
    IdleTimeout,
    /// User manually triggered capture (e.g., adding a note)
    Manual,
}

/// Session status at snapshot time
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Running,
    Waiting,
    Stopped,
}

/// Attention state summary captured from session
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttentionSummary {
    /// Type of attention trigger
    #[serde(rename = "attention_type")]
    pub attention_type: AttentionType,
    /// Preview text/snippet
    pub preview: String,
    /// ISO8601 timestamp when attention was triggered
    pub triggered_at: String,
}

/// Type of attention state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionType {
    InputRequired,
    DecisionPoint,
    Completed,
    Error,
}

/// Terminal context captured from a session
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalContext {
    /// Session ID
    pub session_id: u64,
    /// Session status at capture time
    pub status: SessionStatus,
    /// Exit code (if session stopped)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Last attention state (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_attention: Option<AttentionSummary>,
    /// Terminal tail inline (sanitized, for small tails)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail_inline: Option<String>,
    /// Path to terminal tail file (sanitized, for large tails)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tail_path: Option<String>,
}

/// Context snapshot (v1 schema)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSnapshotV1 {
    /// Snapshot identifier
    pub id: SnapshotId,
    /// Schema version (always 1 for v1)
    pub version: u32,

    /// Absolute path to project TODO.md
    pub project_path: String,
    /// Task ID (stable identifier)
    pub task_id: String,
    /// Task title text at time of capture
    pub task_title_at_capture: String,

    /// ISO8601 timestamp when snapshot was captured
    pub captured_at: String,
    /// Reason for capture
    pub capture_reason: CaptureReason,

    /// Terminal context (best effort)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<TerminalContext>,

    /// User note to future self
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_note: Option<String>,

    /// Editor context (reserved for future use)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor: Option<serde_json::Value>,
}

impl ContextSnapshotV1 {
    /// Create a new snapshot with minimal required fields
    pub fn new(
        id: SnapshotId,
        project_path: String,
        task_id: String,
        task_title_at_capture: String,
        captured_at: String,
        capture_reason: CaptureReason,
    ) -> Self {
        Self {
            id,
            version: 1,
            project_path,
            task_id,
            task_title_at_capture,
            captured_at,
            capture_reason,
            terminal: None,
            user_note: None,
            editor: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_reason_serde() {
        // Verify CaptureReason serializes to snake_case strings
        let cases = vec![
            (CaptureReason::SessionStopped, "session_stopped"),
            (CaptureReason::SessionWaiting, "session_waiting"),
            (CaptureReason::SessionRunning, "session_running"),
            (CaptureReason::IdleTimeout, "idle_timeout"),
            (CaptureReason::Manual, "manual"),
        ];

        for (reason, expected_str) in cases {
            let json = serde_json::to_string(&reason).unwrap();
            assert_eq!(json, format!("\"{}\"", expected_str));

            let deserialized: CaptureReason = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, reason);
        }
    }

    #[test]
    fn test_context_snapshot_v1_serde_roundtrip() {
        // Minimal snapshot
        let snapshot = ContextSnapshotV1::new(
            "2026-02-06T13:12:33Z_qdz.fix-api-timeout-bug".to_string(),
            "/Users/test/project/TODO.md".to_string(),
            "qdz.fix-api-timeout-bug".to_string(),
            "Fix API timeout bug".to_string(),
            "2026-02-06T13:12:33Z".to_string(),
            CaptureReason::SessionStopped,
        );

        let json = serde_json::to_string_pretty(&snapshot).unwrap();
        let deserialized: ContextSnapshotV1 = serde_json::from_str(&json).unwrap();

        assert_eq!(snapshot, deserialized);
        assert_eq!(deserialized.version, 1);
        assert_eq!(deserialized.capture_reason, CaptureReason::SessionStopped);
    }

    #[test]
    fn test_context_snapshot_v1_with_terminal() {
        let mut snapshot = ContextSnapshotV1::new(
            "2026-02-06T13:12:33Z_abc.test-task".to_string(),
            "/Users/test/TODO.md".to_string(),
            "abc.test-task".to_string(),
            "Test task".to_string(),
            "2026-02-06T13:12:33Z".to_string(),
            CaptureReason::Manual,
        );

        snapshot.terminal = Some(TerminalContext {
            session_id: 42,
            status: SessionStatus::Stopped,
            exit_code: Some(0),
            last_attention: Some(AttentionSummary {
                attention_type: AttentionType::Completed,
                preview: "Build finished successfully".to_string(),
                triggered_at: "2026-02-06T13:12:00Z".to_string(),
            }),
            tail_inline: Some("$ cargo build\n   Compiling...\n   Finished".to_string()),
            tail_path: None,
        });

        snapshot.user_note = Some("Remember to update docs".to_string());

        let json = serde_json::to_string_pretty(&snapshot).unwrap();
        let deserialized: ContextSnapshotV1 = serde_json::from_str(&json).unwrap();

        assert_eq!(snapshot, deserialized);

        // Verify optional fields are present
        assert!(deserialized.terminal.is_some());
        assert!(deserialized.user_note.is_some());

        let terminal = deserialized.terminal.unwrap();
        assert_eq!(terminal.session_id, 42);
        assert_eq!(terminal.status, SessionStatus::Stopped);
        assert_eq!(terminal.exit_code, Some(0));
        assert!(terminal.last_attention.is_some());

        // Verify JSON structure matches plan expectations
        assert!(json.contains("\"capture_reason\": \"manual\""));
        assert!(json.contains("\"attention_type\": \"completed\""));
        assert!(json.contains("\"version\": 1"));
    }

    #[test]
    fn test_optional_fields_omitted_in_json() {
        let snapshot = ContextSnapshotV1::new(
            "2026-02-06T13:12:33Z_xyz.simple".to_string(),
            "/Users/test/TODO.md".to_string(),
            "xyz.simple".to_string(),
            "Simple task".to_string(),
            "2026-02-06T13:12:33Z".to_string(),
            CaptureReason::IdleTimeout,
        );

        let json = serde_json::to_string(&snapshot).unwrap();

        // Optional fields should not appear in JSON
        assert!(!json.contains("terminal"));
        assert!(!json.contains("user_note"));
        assert!(!json.contains("editor"));
        assert!(!json.contains("exit_code"));
    }
}
