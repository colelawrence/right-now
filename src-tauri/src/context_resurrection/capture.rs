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

// CaptureService stub (implementation comes in later phases per plan)
// Will implement:
// - capture_now() using SessionProvider trait
// - sanitize_terminal_output() per §0.13.1
// - per-task capture lock (flock) per §1.3.3
// - deduplication window (5s same task_id+reason) per §1.3.3
// - rate limit (1 capture per task per 2s) per §1.3.3
