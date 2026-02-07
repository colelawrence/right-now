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

use crate::context_resurrection::models::{CaptureReason, ContextSnapshotV1, TerminalContext};
use crate::context_resurrection::store::SnapshotStore;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

/// Clock trait for testable time
pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
    fn now_utc(&self) -> SystemTime;
}

/// Real system clock
#[derive(Debug, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
    fn now_utc(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// Capture coordination state (dedup + rate limit)
#[derive(Debug)]
struct CaptureState {
    /// Last capture timestamp per (task_id, reason) for 5s dedup
    dedup_map: HashMap<(String, CaptureReason), Instant>,
    /// Last capture timestamp per task_id for 2s rate limit
    rate_limit_map: HashMap<String, Instant>,
}

impl CaptureState {
    fn new() -> Self {
        Self {
            dedup_map: HashMap::new(),
            rate_limit_map: HashMap::new(),
        }
    }

    /// Check if we should skip this capture due to dedup window (5s)
    fn is_duplicate(&self, task_id: &str, reason: CaptureReason, now: Instant) -> bool {
        if let Some(&last) = self.dedup_map.get(&(task_id.to_string(), reason)) {
            now.duration_since(last) < Duration::from_secs(5)
        } else {
            false
        }
    }

    /// Check if we should skip this capture due to rate limit (2s per task)
    fn is_rate_limited(&self, task_id: &str, now: Instant) -> bool {
        if let Some(&last) = self.rate_limit_map.get(task_id) {
            now.duration_since(last) < Duration::from_secs(2)
        } else {
            false
        }
    }

    /// Record a successful capture
    fn record_capture(&mut self, task_id: &str, reason: CaptureReason, now: Instant) {
        self.dedup_map.insert((task_id.to_string(), reason), now);
        self.rate_limit_map.insert(task_id.to_string(), now);
    }

    /// Clean up old entries (older than 10s to be safe)
    fn cleanup(&mut self, now: Instant) {
        let threshold = Duration::from_secs(10);
        self.dedup_map
            .retain(|_, &mut t| now.duration_since(t) < threshold);
        self.rate_limit_map
            .retain(|_, &mut t| now.duration_since(t) < threshold);
    }
}

/// Service for capturing context snapshots with coordination
pub struct CaptureService {
    store: SnapshotStore,
    session_provider: Option<Arc<dyn SessionProvider>>,
    state: Arc<Mutex<CaptureState>>,
    clock: Arc<dyn Clock>,
}

impl CaptureService {
    /// Create a new capture service
    pub fn new(store: SnapshotStore, session_provider: Option<Arc<dyn SessionProvider>>) -> Self {
        Self::with_clock(store, session_provider, Arc::new(SystemClock))
    }

    /// Create a capture service with a custom clock (for testing)
    pub fn with_clock(
        store: SnapshotStore,
        session_provider: Option<Arc<dyn SessionProvider>>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            store,
            session_provider,
            state: Arc::new(Mutex::new(CaptureState::new())),
            clock,
        }
    }

    /// Capture a snapshot now with dedup, rate limiting, and per-task locking
    ///
    /// Returns Ok(Some(snapshot)) if capture succeeded, Ok(None) if skipped
    /// (due to dedup/rate limit/lock timeout), or Err if storage fails.
    pub fn capture_now(
        &self,
        project_path: &Path,
        task_id: &str,
        task_title: &str,
        session_id: Option<u64>,
        reason: CaptureReason,
        user_note: Option<String>,
    ) -> io::Result<Option<ContextSnapshotV1>> {
        let now = self.clock.now();

        // Check dedup window (5s same task_id + reason)
        {
            let state = self.state.lock().unwrap();
            if state.is_duplicate(task_id, reason, now) {
                eprintln!(
                    "Debug: Skipping capture for task {} (reason {:?}): duplicate within 5s window",
                    task_id, reason
                );
                return Ok(None);
            }
        }

        // Check rate limit (2s per task, any reason)
        {
            let state = self.state.lock().unwrap();
            if state.is_rate_limited(task_id, now) {
                eprintln!(
                    "Debug: Skipping capture for task {}: rate limited (2s cooldown)",
                    task_id
                );
                return Ok(None);
            }
        }

        // Acquire per-task lock with 500ms timeout
        let lock_file = self.get_lock_file_path(project_path, task_id)?;
        let _lock_guard = match acquire_task_lock(&lock_file, Duration::from_millis(500)) {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!(
                    "Warning: Failed to acquire lock for task {} within 500ms: {}. Dropping capture.",
                    task_id,
                    e
                );
                return Ok(None);
            }
        };

        // Build snapshot
        let timestamp = format_timestamp(self.clock.now_utc());
        let snapshot_id = format!("{}_{}", timestamp, task_id);

        let mut snapshot = ContextSnapshotV1::new(
            snapshot_id,
            project_path.to_string_lossy().to_string(),
            task_id.to_string(),
            task_title.to_string(),
            timestamp,
            reason,
        );

        // Capture terminal context if session_id provided
        if let Some(sid) = session_id {
            if let Some(ref provider) = self.session_provider {
                if let Some(session_snapshot) = provider.get_session_state(sid) {
                    let sanitized_tail = sanitize_terminal_output(&session_snapshot.tail);

                    snapshot.terminal = Some(TerminalContext {
                        session_id: sid,
                        status: session_snapshot.status,
                        exit_code: session_snapshot.exit_code,
                        last_attention: session_snapshot.last_attention,
                        tail_inline: if !sanitized_tail.is_empty() {
                            Some(sanitized_tail)
                        } else {
                            None
                        },
                        tail_path: None, // Phase 1 keeps tail inline; tail file comes later
                    });
                }
            }
        }

        // Attach user note if provided
        if let Some(note) = user_note {
            snapshot.user_note = Some(note);
        }

        // Write snapshot atomically
        self.store
            .write_snapshot(project_path, task_id, &snapshot)?;

        // Record successful capture
        {
            let mut state = self.state.lock().unwrap();
            state.record_capture(task_id, reason, now);
            state.cleanup(now);
        }

        eprintln!(
            "Info: Captured snapshot {} for task {} (reason: {:?})",
            snapshot.id, task_id, reason
        );

        Ok(Some(snapshot))
    }

    /// Get the lock file path for a task
    fn get_lock_file_path(
        &self,
        project_path: &Path,
        task_id: &str,
    ) -> io::Result<std::path::PathBuf> {
        let task_dir = self.store.task_dir_public(project_path, task_id);
        std::fs::create_dir_all(&task_dir)?;
        Ok(task_dir.join(".lock"))
    }
}

/// Acquire a flock on the task lock file with timeout
///
/// Returns the file handle which holds the lock (lock is released on drop)
fn acquire_task_lock(lock_path: &Path, timeout: Duration) -> io::Result<File> {
    use fs2::FileExt;
    use std::thread;

    let start = Instant::now();
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(lock_path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }

    loop {
        match file.try_lock_exclusive() {
            Ok(()) => return Ok(file),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                if start.elapsed() >= timeout {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "Failed to acquire lock within timeout",
                    ));
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(e),
        }
    }
}

/// Format SystemTime as ISO8601 UTC timestamp
fn format_timestamp(time: SystemTime) -> String {
    use chrono::{DateTime, Utc};
    let datetime: DateTime<Utc> = time.into();
    datetime.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Test clock with controllable time
    #[derive(Clone)]
    struct TestClock {
        instant: Arc<Mutex<Instant>>,
        system_time: Arc<Mutex<SystemTime>>,
    }

    impl TestClock {
        fn new() -> Self {
            Self {
                instant: Arc::new(Mutex::new(Instant::now())),
                system_time: Arc::new(Mutex::new(SystemTime::now())),
            }
        }

        fn advance(&self, duration: Duration) {
            *self.instant.lock().unwrap() += duration;
            *self.system_time.lock().unwrap() += duration;
        }
    }

    impl Clock for TestClock {
        fn now(&self) -> Instant {
            *self.instant.lock().unwrap()
        }

        fn now_utc(&self) -> SystemTime {
            *self.system_time.lock().unwrap()
        }
    }

    /// Mock session provider
    struct MockSessionProvider {
        snapshots: Arc<Mutex<HashMap<u64, SessionSnapshot>>>,
    }

    impl MockSessionProvider {
        fn new() -> Self {
            Self {
                snapshots: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        fn set(&self, session_id: u64, snapshot: SessionSnapshot) {
            self.snapshots.lock().unwrap().insert(session_id, snapshot);
        }
    }

    impl SessionProvider for MockSessionProvider {
        fn get_session_state(&self, session_id: u64) -> Option<SessionSnapshot> {
            self.snapshots.lock().unwrap().get(&session_id).cloned()
        }
    }

    #[test]
    fn test_dedup_window_blocks_same_task_and_reason() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let clock = Arc::new(TestClock::new());
        let service = CaptureService::with_clock(store, None, clock.clone());

        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        // First capture succeeds
        let result1 = service
            .capture_now(
                &project_path,
                "abc.test-task",
                "Test task",
                None,
                CaptureReason::SessionStopped,
                None,
            )
            .unwrap();
        assert!(result1.is_some(), "First capture should succeed");

        // Second capture with same task + reason within 5s is deduplicated
        clock.advance(Duration::from_secs(3));
        let result2 = service
            .capture_now(
                &project_path,
                "abc.test-task",
                "Test task",
                None,
                CaptureReason::SessionStopped,
                None,
            )
            .unwrap();
        assert!(result2.is_none(), "Second capture should be deduplicated");

        // After 5s window, capture succeeds again
        clock.advance(Duration::from_secs(3)); // total 6s
        let result3 = service
            .capture_now(
                &project_path,
                "abc.test-task",
                "Test task",
                None,
                CaptureReason::SessionStopped,
                None,
            )
            .unwrap();
        assert!(result3.is_some(), "Third capture after 5s should succeed");
    }

    #[test]
    fn test_dedup_allows_different_reasons_after_rate_limit() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let clock = Arc::new(TestClock::new());
        let service = CaptureService::with_clock(store, None, clock.clone());

        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        // First capture with SessionStopped
        let result1 = service
            .capture_now(
                &project_path,
                "xyz.different-reasons",
                "Test",
                None,
                CaptureReason::SessionStopped,
                None,
            )
            .unwrap();
        assert!(result1.is_some());

        // Advance past rate limit (2s) but before dedup window (5s)
        clock.advance(Duration::from_millis(2100));

        // Second capture with different reason (Manual) should succeed
        // (not blocked by dedup since reason is different)
        let result2 = service
            .capture_now(
                &project_path,
                "xyz.different-reasons",
                "Test",
                None,
                CaptureReason::Manual,
                None,
            )
            .unwrap();
        assert!(
            result2.is_some(),
            "Different reason should not be deduplicated"
        );

        // Try same reason (SessionStopped) again - should be blocked by dedup
        clock.advance(Duration::from_millis(2100));
        let result3 = service
            .capture_now(
                &project_path,
                "xyz.different-reasons",
                "Test",
                None,
                CaptureReason::SessionStopped,
                None,
            )
            .unwrap();
        assert!(
            result3.is_none(),
            "Same reason within 5s should be deduplicated"
        );
    }

    #[test]
    fn test_rate_limit_blocks_all_reasons_within_2s() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let clock = Arc::new(TestClock::new());
        let service = CaptureService::with_clock(store, None, clock.clone());

        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO.md\n").unwrap();

        // First capture
        let result1 = service
            .capture_now(
                &project_path,
                "rlm.rate-limit",
                "Test",
                None,
                CaptureReason::SessionStopped,
                None,
            )
            .unwrap();
        assert!(result1.is_some());

        // Second capture with different reason but within 2s is rate limited
        clock.advance(Duration::from_millis(1500));
        let result2 = service
            .capture_now(
                &project_path,
                "rlm.rate-limit",
                "Test",
                None,
                CaptureReason::Manual,
                None,
            )
            .unwrap();
        assert!(result2.is_none(), "Should be rate limited within 2s");

        // After 2s, capture succeeds
        clock.advance(Duration::from_millis(600)); // total 2.1s
        let result3 = service
            .capture_now(
                &project_path,
                "rlm.rate-limit",
                "Test",
                None,
                CaptureReason::IdleTimeout,
                None,
            )
            .unwrap();
        assert!(result3.is_some(), "Should succeed after 2s rate limit");
    }

    #[test]
    fn test_rate_limit_per_task_isolation() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let clock = Arc::new(TestClock::new());
        let service = CaptureService::with_clock(store, None, clock.clone());

        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        // Capture for task A
        let result_a = service
            .capture_now(
                &project_path,
                "aaa.task-a",
                "Task A",
                None,
                CaptureReason::SessionStopped,
                None,
            )
            .unwrap();
        assert!(result_a.is_some());

        // Capture for task B (different task) should succeed immediately
        let result_b = service
            .capture_now(
                &project_path,
                "bbb.task-b",
                "Task B",
                None,
                CaptureReason::SessionStopped,
                None,
            )
            .unwrap();
        assert!(
            result_b.is_some(),
            "Different task should not be rate limited"
        );
    }

    #[test]
    fn test_successful_capture_with_session_provider() {
        use crate::context_resurrection::models::SessionStatus;

        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let clock = Arc::new(TestClock::new());

        let provider = Arc::new(MockSessionProvider::new());
        provider.set(
            42,
            SessionSnapshot {
                status: SessionStatus::Stopped,
                exit_code: Some(0),
                last_attention: None,
                tail: "$ cargo build\n   Compiling...\n   Finished".to_string(),
            },
        );

        let service = CaptureService::with_clock(store.clone(), Some(provider), clock);

        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        let snapshot = service
            .capture_now(
                &project_path,
                "sss.session-test",
                "Session Test",
                Some(42),
                CaptureReason::SessionStopped,
                Some("My note".to_string()),
            )
            .unwrap()
            .expect("Capture should succeed");

        // Verify snapshot fields
        assert!(snapshot.id.contains("sss.session-test"));
        assert_eq!(snapshot.task_id, "sss.session-test");
        assert_eq!(snapshot.task_title_at_capture, "Session Test");
        assert_eq!(snapshot.capture_reason, CaptureReason::SessionStopped);
        assert_eq!(snapshot.user_note, Some("My note".to_string()));

        // Verify terminal context
        let terminal = snapshot.terminal.expect("Terminal should be captured");
        assert_eq!(terminal.session_id, 42);
        assert_eq!(terminal.status, SessionStatus::Stopped);
        assert_eq!(terminal.exit_code, Some(0));
        assert!(terminal.tail_inline.is_some());
        assert!(terminal.tail_inline.unwrap().contains("cargo build"));

        // Verify snapshot was written to disk
        let read_snapshot = store
            .read_snapshot(&project_path, "sss.session-test", &snapshot.id)
            .unwrap();
        assert_eq!(read_snapshot.id, snapshot.id);
    }

    #[test]
    fn test_sanitization_applied_to_terminal_tail() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let clock = Arc::new(TestClock::new());

        let provider = Arc::new(MockSessionProvider::new());
        provider.set(
            99,
            SessionSnapshot {
                status: SessionStatus::Stopped,
                exit_code: Some(0),
                last_attention: None,
                tail: "export API_KEY=secret123\nRunning tests...".to_string(),
            },
        );

        let service = CaptureService::with_clock(store, Some(provider), clock);

        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        let snapshot = service
            .capture_now(
                &project_path,
                "san.sanitize-test",
                "Sanitize Test",
                Some(99),
                CaptureReason::Manual,
                None,
            )
            .unwrap()
            .expect("Capture should succeed");

        let terminal = snapshot.terminal.expect("Terminal should be captured");
        let tail = terminal.tail_inline.expect("Tail should be present");

        // Verify secret was redacted
        assert!(!tail.contains("secret123"));
        assert!(tail.contains("[REDACTED]"));
        assert!(tail.contains("Running tests"));
    }

    #[test]
    fn test_lock_timeout_scenario() {
        // This test simulates lock contention by holding a lock manually
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let clock = Arc::new(TestClock::new());
        let service = CaptureService::with_clock(store.clone(), None, clock);

        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        // Create lock file path
        let task_dir = store.task_dir_public(&project_path, "lck.lock-test");
        std::fs::create_dir_all(&task_dir).unwrap();
        let lock_path = task_dir.join(".lock");

        // Hold the lock in this thread
        use fs2::FileExt;
        let _held_lock = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        _held_lock.lock_exclusive().unwrap();

        // Now try to capture - should fail with timeout
        let result = service.capture_now(
            &project_path,
            "lck.lock-test",
            "Lock Test",
            None,
            CaptureReason::Manual,
            None,
        );

        // Should return Ok(None) when lock times out
        assert!(result.is_ok());
        assert!(
            result.unwrap().is_none(),
            "Capture should be dropped on lock timeout"
        );
    }

    #[test]
    fn test_state_cleanup_removes_old_entries() {
        let mut state = CaptureState::new();
        let base = Instant::now();

        // Record some captures
        state.record_capture("task1", CaptureReason::Manual, base);
        state.record_capture(
            "task2",
            CaptureReason::SessionStopped,
            base + Duration::from_secs(1),
        );

        assert_eq!(state.dedup_map.len(), 2);
        assert_eq!(state.rate_limit_map.len(), 2);

        // Cleanup with time > 10s later
        state.cleanup(base + Duration::from_secs(15));

        // All entries should be removed
        assert_eq!(state.dedup_map.len(), 0);
        assert_eq!(state.rate_limit_map.len(), 0);
    }

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
