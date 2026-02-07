//! RPC handlers for Context Resurrection requests
//!
//! Implementation of daemon API for CR queries, captures, and deletions.
//! Gracefully handles store unavailability by returning empty results.

use crate::context_resurrection::capture::CaptureService;
use crate::context_resurrection::models::{CaptureReason, ContextSnapshotV1};
use crate::context_resurrection::store::SnapshotStore;
use std::path::Path;

/// Get the latest snapshot for a specific task, or None if no snapshots exist
///
/// If store is unavailable or task_id is None, returns Ok(None).
pub fn cr_latest(
    store: &SnapshotStore,
    project_path: &str,
    task_id: Option<&str>,
) -> Result<Option<ContextSnapshotV1>, String> {
    if !store.is_available() {
        return Ok(None);
    }

    let task_id = match task_id {
        Some(id) => id,
        None => return Ok(None), // No task_id provided
    };

    let project = Path::new(project_path);
    store
        .latest_snapshot(project, task_id)
        .map_err(|e| format!("Failed to get latest snapshot: {}", e))
}

/// List snapshots for a specific task
///
/// If store is unavailable, returns Ok(empty vec).
pub fn cr_list(
    store: &SnapshotStore,
    project_path: &str,
    task_id: &str,
    limit: Option<usize>,
) -> Result<Vec<ContextSnapshotV1>, String> {
    if !store.is_available() {
        return Ok(Vec::new());
    }

    let project = Path::new(project_path);
    store
        .list_snapshots(project, task_id, limit)
        .map_err(|e| format!("Failed to list snapshots: {}", e))
}

/// Get a specific snapshot by ID
///
/// If store is unavailable, returns Err.
pub fn cr_get(
    store: &SnapshotStore,
    project_path: &str,
    task_id: &str,
    snapshot_id: &str,
) -> Result<ContextSnapshotV1, String> {
    if !store.is_available() {
        return Err("Snapshot store is unavailable".to_string());
    }

    let project = Path::new(project_path);
    store
        .read_snapshot(project, task_id, snapshot_id)
        .map_err(|e| format!("Failed to read snapshot: {}", e))
}

/// Trigger a manual snapshot capture
///
/// If store is unavailable, returns Err.
/// If capture is skipped (dedup/rate-limit), returns Ok(None).
pub fn cr_capture_now(
    capture_service: &CaptureService,
    project_path: &str,
    task_id: &str,
    task_title: &str,
    session_id: Option<u64>,
    user_note: Option<String>,
) -> Result<Option<ContextSnapshotV1>, String> {
    let project = Path::new(project_path);
    capture_service
        .capture_now(
            project,
            task_id,
            task_title,
            session_id,
            CaptureReason::Manual,
            user_note,
        )
        .map_err(|e| format!("Failed to capture snapshot: {}", e))
}

/// Delete all snapshots for a specific task
///
/// If store is unavailable, returns Err.
/// Returns number of snapshots deleted.
pub fn cr_delete_task(
    store: &SnapshotStore,
    project_path: &str,
    task_id: &str,
) -> Result<usize, String> {
    if !store.is_available() {
        return Err("Snapshot store is unavailable".to_string());
    }

    let project = Path::new(project_path);
    store
        .delete_task(project, task_id)
        .map_err(|e| format!("Failed to delete task snapshots: {}", e))
}

/// Delete all snapshots for a specific project
///
/// If store is unavailable, returns Err.
/// Returns total number of snapshots deleted.
pub fn cr_delete_project(store: &SnapshotStore, project_path: &str) -> Result<usize, String> {
    if !store.is_available() {
        return Err("Snapshot store is unavailable".to_string());
    }

    let project = Path::new(project_path);
    store
        .delete_project(project)
        .map_err(|e| format!("Failed to delete project snapshots: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_resurrection::capture::{Clock, SessionProvider, SessionSnapshot};
    use crate::context_resurrection::models::SessionStatus;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant, SystemTime};
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
    fn test_cr_latest_returns_newest_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        // Create two snapshots with different timestamps
        let snapshot1 = ContextSnapshotV1::new(
            "2026-02-06T10:00:00Z_abc.test-task".to_string(),
            project_path.to_string_lossy().to_string(),
            "abc.test-task".to_string(),
            "Test task".to_string(),
            "2026-02-06T10:00:00Z".to_string(),
            CaptureReason::SessionStopped,
        );

        let snapshot2 = ContextSnapshotV1::new(
            "2026-02-06T11:00:00Z_abc.test-task".to_string(),
            project_path.to_string_lossy().to_string(),
            "abc.test-task".to_string(),
            "Test task".to_string(),
            "2026-02-06T11:00:00Z".to_string(),
            CaptureReason::Manual,
        );

        store
            .write_snapshot(&project_path, "abc.test-task", &snapshot1)
            .unwrap();
        store
            .write_snapshot(&project_path, "abc.test-task", &snapshot2)
            .unwrap();

        // cr_latest should return the newest one
        let result = cr_latest(
            &store,
            project_path.to_str().unwrap(),
            Some("abc.test-task"),
        )
        .expect("Should succeed");

        assert!(result.is_some());
        let latest = result.unwrap();
        assert_eq!(latest.captured_at, "2026-02-06T11:00:00Z");
        assert_eq!(latest.capture_reason, CaptureReason::Manual);
    }

    #[test]
    fn test_cr_latest_returns_none_when_no_snapshots() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        let result = cr_latest(&store, project_path.to_str().unwrap(), Some("xyz.no-snaps"))
            .expect("Should succeed");

        assert!(result.is_none());
    }

    #[test]
    fn test_cr_latest_returns_none_when_task_id_none() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let project_path = temp_dir.path().join("TODO.md");

        let result =
            cr_latest(&store, project_path.to_str().unwrap(), None).expect("Should succeed");

        assert!(result.is_none());
    }

    #[test]
    fn test_cr_list_returns_all_snapshots_sorted() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        // Create 3 snapshots
        for i in 1..=3 {
            let snapshot = ContextSnapshotV1::new(
                format!("2026-02-06T10:{:02}:00Z_lst.list-test", i),
                project_path.to_string_lossy().to_string(),
                "lst.list-test".to_string(),
                "List test".to_string(),
                format!("2026-02-06T10:{:02}:00Z", i),
                CaptureReason::IdleTimeout,
            );
            store
                .write_snapshot(&project_path, "lst.list-test", &snapshot)
                .unwrap();
        }

        let result = cr_list(
            &store,
            project_path.to_str().unwrap(),
            "lst.list-test",
            None,
        )
        .expect("Should succeed");

        assert_eq!(result.len(), 3);
        // Should be sorted newest first
        assert!(result[0].captured_at.contains("10:03:00"));
        assert!(result[1].captured_at.contains("10:02:00"));
        assert!(result[2].captured_at.contains("10:01:00"));
    }

    #[test]
    fn test_cr_list_respects_limit() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        // Create 5 snapshots
        for i in 1..=5 {
            let snapshot = ContextSnapshotV1::new(
                format!("2026-02-06T11:{:02}:00Z_lim.limit-test", i),
                project_path.to_string_lossy().to_string(),
                "lim.limit-test".to_string(),
                "Limit test".to_string(),
                format!("2026-02-06T11:{:02}:00Z", i),
                CaptureReason::SessionWaiting,
            );
            store
                .write_snapshot(&project_path, "lim.limit-test", &snapshot)
                .unwrap();
        }

        let result = cr_list(
            &store,
            project_path.to_str().unwrap(),
            "lim.limit-test",
            Some(2),
        )
        .expect("Should succeed");

        assert_eq!(result.len(), 2);
        // Should get the 2 newest
        assert!(result[0].captured_at.contains("11:05:00"));
        assert!(result[1].captured_at.contains("11:04:00"));
    }

    #[test]
    fn test_cr_get_retrieves_specific_snapshot() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        let snapshot = ContextSnapshotV1::new(
            "2026-02-06T12:30:00Z_get.get-test".to_string(),
            project_path.to_string_lossy().to_string(),
            "get.get-test".to_string(),
            "Get test".to_string(),
            "2026-02-06T12:30:00Z".to_string(),
            CaptureReason::Manual,
        );

        store
            .write_snapshot(&project_path, "get.get-test", &snapshot)
            .unwrap();

        let result = cr_get(
            &store,
            project_path.to_str().unwrap(),
            "get.get-test",
            "2026-02-06T12:30:00Z_get.get-test",
        )
        .expect("Should succeed");

        assert_eq!(result.id, snapshot.id);
        assert_eq!(result.task_id, "get.get-test");
    }

    #[test]
    fn test_cr_get_fails_when_snapshot_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        let result = cr_get(
            &store,
            project_path.to_str().unwrap(),
            "nex.nonexistent",
            "2026-02-06T00:00:00Z_nex.nonexistent",
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_cr_capture_now_creates_snapshot() {
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
                tail: "Build succeeded".to_string(),
            },
        );

        let capture_service = CaptureService::with_clock(store.clone(), Some(provider), clock);

        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        let result = cr_capture_now(
            &capture_service,
            project_path.to_str().unwrap(),
            "cap.capture-test",
            "Capture test",
            Some(42),
            Some("User note".to_string()),
        )
        .expect("Should succeed");

        assert!(result.is_some());
        let snapshot = result.unwrap();
        assert_eq!(snapshot.task_id, "cap.capture-test");
        assert_eq!(snapshot.capture_reason, CaptureReason::Manual);
        assert_eq!(snapshot.user_note, Some("User note".to_string()));

        // Verify snapshot was written to disk
        let snapshots = cr_list(
            &store,
            project_path.to_str().unwrap(),
            "cap.capture-test",
            None,
        )
        .expect("Should succeed");
        assert_eq!(snapshots.len(), 1);
    }

    #[test]
    fn test_cr_capture_now_returns_none_when_deduplicated() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let clock = Arc::new(TestClock::new());
        let capture_service = CaptureService::with_clock(store, None, clock.clone());

        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        // First capture
        let result1 = cr_capture_now(
            &capture_service,
            project_path.to_str().unwrap(),
            "dup.dedup-test",
            "Dedup test",
            None,
            None,
        )
        .expect("Should succeed");
        assert!(result1.is_some());

        // Second capture within 5s window (dedup)
        clock.advance(Duration::from_secs(3));
        let result2 = cr_capture_now(
            &capture_service,
            project_path.to_str().unwrap(),
            "dup.dedup-test",
            "Dedup test",
            None,
            None,
        )
        .expect("Should succeed");
        assert!(result2.is_none(), "Should be deduplicated");
    }

    #[test]
    fn test_cr_delete_task_removes_all_snapshots() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        // Create 3 snapshots
        for i in 1..=3 {
            let snapshot = ContextSnapshotV1::new(
                format!("2026-02-06T13:{:02}:00Z_del.delete-test", i),
                project_path.to_string_lossy().to_string(),
                "del.delete-test".to_string(),
                "Delete test".to_string(),
                format!("2026-02-06T13:{:02}:00Z", i),
                CaptureReason::SessionStopped,
            );
            store
                .write_snapshot(&project_path, "del.delete-test", &snapshot)
                .unwrap();
        }

        // Verify snapshots exist
        let before = cr_list(
            &store,
            project_path.to_str().unwrap(),
            "del.delete-test",
            None,
        )
        .expect("Should succeed");
        assert_eq!(before.len(), 3);

        // Delete task
        let deleted = cr_delete_task(&store, project_path.to_str().unwrap(), "del.delete-test")
            .expect("Should succeed");

        assert_eq!(deleted, 3);

        // Verify snapshots are gone
        let after = cr_list(
            &store,
            project_path.to_str().unwrap(),
            "del.delete-test",
            None,
        )
        .expect("Should succeed");
        assert_eq!(after.len(), 0);
    }

    #[test]
    fn test_cr_delete_project_removes_all_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());
        let project_path = temp_dir.path().join("TODO.md");
        std::fs::write(&project_path, "# TODO\n").unwrap();

        // Create snapshots for 2 tasks
        for task_num in 1..=2 {
            let task_id = format!("tk{}.task-{}", task_num, task_num);
            for snap_num in 1..=2 {
                let snapshot = ContextSnapshotV1::new(
                    format!(
                        "2026-02-06T14:{:02}:00Z_{}",
                        task_num * 10 + snap_num,
                        task_id
                    ),
                    project_path.to_string_lossy().to_string(),
                    task_id.clone(),
                    format!("Task {}", task_num),
                    format!("2026-02-06T14:{:02}:00Z", task_num * 10 + snap_num),
                    CaptureReason::Manual,
                );
                store
                    .write_snapshot(&project_path, &task_id, &snapshot)
                    .unwrap();
            }
        }

        // Verify we have 4 total snapshots
        let mut total = 0;
        for task_num in 1..=2 {
            let task_id = format!("tk{}.task-{}", task_num, task_num);
            total += cr_list(&store, project_path.to_str().unwrap(), &task_id, None)
                .expect("Should succeed")
                .len();
        }
        assert_eq!(total, 4);

        // Delete project
        let deleted =
            cr_delete_project(&store, project_path.to_str().unwrap()).expect("Should succeed");

        assert_eq!(deleted, 4);

        // Verify all snapshots are gone
        for task_num in 1..=2 {
            let task_id = format!("tk{}.task-{}", task_num, task_num);
            let after = cr_list(&store, project_path.to_str().unwrap(), &task_id, None)
                .expect("Should succeed");
            assert_eq!(after.len(), 0);
        }
    }

    #[test]
    fn test_unavailable_store_returns_empty_results() {
        let bad_path = std::path::PathBuf::from("/dev/null/cannot-create");
        let store = SnapshotStore::new(&bad_path);
        assert!(!store.is_available());

        // cr_latest returns None
        let result = cr_latest(&store, "/tmp/TODO.md", Some("task")).expect("Should succeed");
        assert!(result.is_none());

        // cr_list returns empty vec
        let result = cr_list(&store, "/tmp/TODO.md", "task", None).expect("Should succeed");
        assert_eq!(result.len(), 0);

        // cr_get returns Err
        let result = cr_get(&store, "/tmp/TODO.md", "task", "snapshot-id");
        assert!(result.is_err());

        // cr_delete_task returns Err
        let result = cr_delete_task(&store, "/tmp/TODO.md", "task");
        assert!(result.is_err());

        // cr_delete_project returns Err
        let result = cr_delete_project(&store, "/tmp/TODO.md");
        assert!(result.is_err());
    }
}
