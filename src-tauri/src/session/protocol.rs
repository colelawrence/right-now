// Session protocol - shared structs for daemon <-> CLI communication
// Uses framed JSON messages over Unix sockets

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Session status reflecting the current state of a terminal session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum SessionStatus {
    /// Session is actively running a command
    Running,
    /// Session has paused or is waiting for input
    Waiting,
    /// Session has stopped (exit code received or manually stopped)
    Stopped,
}

impl std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionStatus::Running => write!(f, "Running"),
            SessionStatus::Waiting => write!(f, "Waiting"),
            SessionStatus::Stopped => write!(f, "Stopped"),
        }
    }
}

impl std::str::FromStr for SessionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Running" => Ok(SessionStatus::Running),
            "Waiting" => Ok(SessionStatus::Waiting),
            "Stopped" => Ok(SessionStatus::Stopped),
            _ => Err(format!("Unknown session status: {}", s)),
        }
    }
}

/// Unique session identifier
pub type SessionId = u64;

/// Types of attention events detected in output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionType {
    InputRequired,
    DecisionPoint,
    Completed,
    Error,
}

impl std::fmt::Display for AttentionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttentionType::InputRequired => write!(f, "Input required"),
            AttentionType::DecisionPoint => write!(f, "Decision point"),
            AttentionType::Completed => write!(f, "Completed"),
            AttentionType::Error => write!(f, "Error"),
        }
    }
}

/// Summary of an attention event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionSummary {
    pub profile: String,
    pub attention_type: AttentionType,
    pub preview: String,
    pub triggered_at: DateTime<Utc>,
}

/// Session metadata stored in persistence and exchanged via protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session ID (used in deep links: todos://session/<id>)
    pub id: SessionId,
    /// Task key/name from the TODO file that this session is associated with
    pub task_key: String,
    /// Stable task ID from TODO.md (e.g., "abc.derived-label") if available
    /// Allows CR to join session snapshots even after task name changes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Absolute path to the TODO.md file
    pub project_path: String,
    /// Current status of the session
    pub status: SessionStatus,
    /// PID of the PTY child process (if running)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pty_pid: Option<u32>,
    /// Shell command being executed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_command: Option<Vec<String>>,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the session was last updated
    pub updated_at: DateTime<Utc>,
    /// Exit code if session has stopped
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Last detected attention event, if any
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_attention: Option<AttentionSummary>,
}

impl Session {
    pub fn new(
        id: SessionId,
        task_key: String,
        task_id: Option<String>,
        project_path: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            task_key,
            task_id,
            project_path,
            status: SessionStatus::Running,
            pty_pid: None,
            shell_command: None,
            created_at: now,
            updated_at: now,
            exit_code: None,
            last_attention: None,
        }
    }

    pub fn deep_link(&self) -> String {
        format!("todos://session/{}", self.id)
    }
}

// ============================================================================
// Client -> Daemon requests
// ============================================================================

/// Request message from CLI/UI to daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonRequest {
    /// Start a new session for a task
    Start {
        /// Task name/key to match in the TODO file
        task_key: String,
        /// Stable task ID (e.g., "abc.derived-label") if available from TODO.md
        #[serde(default, skip_serializing_if = "Option::is_none")]
        task_id: Option<String>,
        /// Path to the TODO.md file
        project_path: String,
        /// Optional shell command to run (defaults to $SHELL)
        #[serde(skip_serializing_if = "Option::is_none")]
        shell: Option<Vec<String>>,
    },
    /// Continue/attach to an existing session
    Continue {
        /// Session ID to continue
        session_id: SessionId,
        /// Optional number of bytes of recent output to include
        #[serde(skip_serializing_if = "Option::is_none")]
        tail_bytes: Option<usize>,
    },
    /// List all sessions, optionally filtered by project
    List {
        /// Optional filter by project path
        #[serde(skip_serializing_if = "Option::is_none")]
        project_path: Option<String>,
    },
    /// Attach to PTY output for a running session
    Attach {
        /// Session ID to attach
        session_id: SessionId,
        /// Number of bytes of recent output to replay before streaming live
        #[serde(skip_serializing_if = "Option::is_none")]
        tail_bytes: Option<usize>,
    },
    /// Resize a running session's PTY
    Resize {
        /// Session ID to resize
        session_id: SessionId,
        /// Columns of the terminal
        cols: u16,
        /// Rows of the terminal
        rows: u16,
    },
    /// Stop a running session
    Stop {
        /// Session ID to stop
        session_id: SessionId,
    },
    /// Fetch recent output for a running session
    Tail {
        /// Session ID to fetch output for
        session_id: SessionId,
        /// Maximum bytes to return from the ring buffer
        #[serde(skip_serializing_if = "Option::is_none")]
        bytes: Option<usize>,
    },
    /// Get status of a specific session
    Status {
        /// Session ID to query
        session_id: SessionId,
    },
    /// Ping to check if daemon is alive
    Ping,
    /// Request daemon to shut down gracefully
    Shutdown,
}

// ============================================================================
// Daemon -> Client responses
// ============================================================================

/// Response message from daemon to CLI/UI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonResponse {
    /// Session was started successfully
    SessionStarted { session: Session },
    /// Session was continued (attached)
    SessionContinued {
        session: Session,
        #[serde(skip_serializing_if = "Option::is_none")]
        tail: Option<Vec<u8>>,
    },
    /// Session was stopped
    SessionStopped { session: Session },
    /// List of sessions matching the query
    SessionList { sessions: Vec<Session> },
    /// Status of a single session
    SessionStatus { session: Session },
    /// Attach socket is ready for streaming
    AttachReady {
        session: Session,
        #[serde(skip_serializing_if = "Option::is_none")]
        tail: Option<Vec<u8>>,
        socket_path: String,
    },
    /// Session PTY was resized
    SessionResized {
        session_id: SessionId,
        cols: u16,
        rows: u16,
    },
    /// Recent output tail for a session
    SessionTail {
        session_id: SessionId,
        /// UTF-8 bytes from the PTY ring buffer (may be partial UTF-8)
        data: Vec<u8>,
    },
    /// Pong response
    Pong,
    /// Shutdown acknowledged
    ShuttingDown,
    /// Error response
    Error { message: String },
}

// ============================================================================
// Daemon -> Client push notifications (broadcast)
// ============================================================================

/// Push notification from daemon to subscribed clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DaemonNotification {
    /// A session's status changed
    SessionUpdated { session: Session },
    /// A session was removed/cleaned up
    SessionRemoved { session_id: SessionId },
    /// Session output triggered attention
    Attention {
        session_id: SessionId,
        profile: String,
        attention_type: AttentionType,
        preview: String,
        triggered_at: DateTime<Utc>,
    },
}

// ============================================================================
// Helpers for message framing
// ============================================================================

/// Serialize a message to JSON bytes with newline delimiter
pub fn serialize_message<T: Serialize>(msg: &T) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec(msg)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Deserialize a message from JSON bytes (strips trailing newline)
pub fn deserialize_message<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
) -> Result<T, serde_json::Error> {
    let trimmed = if bytes.last() == Some(&b'\n') {
        &bytes[..bytes.len() - 1]
    } else {
        bytes
    };
    serde_json::from_slice(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_status_roundtrip() {
        for status in [
            SessionStatus::Running,
            SessionStatus::Waiting,
            SessionStatus::Stopped,
        ] {
            let s = status.to_string();
            let parsed: SessionStatus = s.parse().unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn test_request_serialization() {
        let req = DaemonRequest::Start {
            task_key: "Implement reports".to_string(),
            task_id: Some("abc.implement-reports".to_string()),
            project_path: "/path/TODO.md".to_string(),
            shell: Some(vec![
                "/bin/zsh".to_string(),
                "-lc".to_string(),
                "npm run dev".to_string(),
            ]),
        };

        let bytes = serialize_message(&req).unwrap();
        let parsed: DaemonRequest = deserialize_message(&bytes).unwrap();

        if let DaemonRequest::Start {
            task_key,
            task_id,
            project_path,
            shell,
        } = parsed
        {
            assert_eq!(task_key, "Implement reports");
            assert_eq!(task_id, Some("abc.implement-reports".to_string()));
            assert_eq!(project_path, "/path/TODO.md");
            assert_eq!(
                shell,
                Some(vec![
                    "/bin/zsh".to_string(),
                    "-lc".to_string(),
                    "npm run dev".to_string()
                ])
            );
        } else {
            panic!("Wrong variant");
        }
    }

    #[test]
    fn test_session_deep_link() {
        let session = Session::new(
            42,
            "Test task".to_string(),
            Some("abc.test-task".to_string()),
            "/test/TODO.md".to_string(),
        );
        assert_eq!(session.deep_link(), "todos://session/42");
        assert_eq!(session.task_id, Some("abc.test-task".to_string()));
    }
}
