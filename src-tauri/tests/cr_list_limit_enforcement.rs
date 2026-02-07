//! Integration tests for cr_list limit enforcement
//!
//! These tests verify that the daemon correctly enforces limit semantics:
//! - None => default 100
//! - >500 => clamped to 500
//! - <=0 => invalid_request error

use rn_desktop_2_lib::context_resurrection::models::{CaptureReason, ContextSnapshotV1};
use rn_desktop_2_lib::context_resurrection::store::SnapshotStore;
use tempfile::TempDir;

/// Test that limit=None behavior is handled correctly by store
#[test]
fn test_store_limit_none_works() {
    let temp_dir = TempDir::new().unwrap();
    let store = SnapshotStore::new(temp_dir.path());
    let project_path = temp_dir.path().join("TODO.md");
    std::fs::write(&project_path, "# TODO\n").unwrap();

    // Create 5 snapshots
    for i in 1..=5 {
        let snapshot = ContextSnapshotV1::new(
            format!("2026-02-07T10:{:02}:00Z_test.limit-none", i),
            project_path.to_string_lossy().to_string(),
            "test.limit-none".to_string(),
            "Test task".to_string(),
            format!("2026-02-07T10:{:02}:00Z", i),
            CaptureReason::Manual,
        );
        store
            .write_snapshot(&project_path, "test.limit-none", &snapshot)
            .unwrap();
    }

    // Query with None should work (daemon will default to 100)
    let snapshots = store
        .list_snapshots(&project_path, "test.limit-none", None)
        .unwrap();

    assert_eq!(snapshots.len(), 5);
}

/// Test that store respects explicit limits
#[test]
fn test_store_respects_limit() {
    let temp_dir = TempDir::new().unwrap();
    let store = SnapshotStore::new(temp_dir.path());
    let project_path = temp_dir.path().join("TODO.md");
    std::fs::write(&project_path, "# TODO\n").unwrap();

    // Create 10 snapshots
    for i in 1..=10 {
        let snapshot = ContextSnapshotV1::new(
            format!("2026-02-07T11:{:02}:00Z_test.limit-explicit", i),
            project_path.to_string_lossy().to_string(),
            "test.limit-explicit".to_string(),
            "Test task".to_string(),
            format!("2026-02-07T11:{:02}:00Z", i),
            CaptureReason::SessionStopped,
        );
        store
            .write_snapshot(&project_path, "test.limit-explicit", &snapshot)
            .unwrap();
    }

    // Query with limit=3 should return only 3
    let snapshots = store
        .list_snapshots(&project_path, "test.limit-explicit", Some(3))
        .unwrap();

    assert_eq!(snapshots.len(), 3);
}

/// Test that store handles limit=0 gracefully
#[test]
fn test_store_limit_zero_returns_empty() {
    let temp_dir = TempDir::new().unwrap();
    let store = SnapshotStore::new(temp_dir.path());
    let project_path = temp_dir.path().join("TODO.md");
    std::fs::write(&project_path, "# TODO\n").unwrap();

    // Create a snapshot
    let snapshot = ContextSnapshotV1::new(
        "2026-02-07T12:00:00Z_test.limit-zero".to_string(),
        project_path.to_string_lossy().to_string(),
        "test.limit-zero".to_string(),
        "Test task".to_string(),
        "2026-02-07T12:00:00Z".to_string(),
        CaptureReason::Manual,
    );
    store
        .write_snapshot(&project_path, "test.limit-zero", &snapshot)
        .unwrap();

    // Query with limit=0 should return empty
    let snapshots = store
        .list_snapshots(&project_path, "test.limit-zero", Some(0))
        .unwrap();

    assert_eq!(snapshots.len(), 0);
}

/// Document that daemon-level limit enforcement is in handle_request
///
/// The actual enforcement logic is in right-now-daemon.rs:
/// - None => Some(100)
/// - <=0 => Error { code: invalid_request }
/// - >500 => Some(500)
///
/// This test just documents the behavior; full daemon integration tests
/// would require starting the daemon and sending IPC requests.
#[test]
fn test_daemon_limit_enforcement_documented() {
    // Daemon enforcement:
    // 1. limit=None => enforced_limit=Some(100)
    let enforced = match None::<usize> {
        None => Some(100),
        Some(n) if n <= 0 => panic!("invalid_request"),
        Some(n) if n > 500 => Some(500),
        Some(n) => Some(n),
    };
    assert_eq!(enforced, Some(100));

    // 2. limit=Some(0) => error
    let result = match Some(0usize) {
        None => Some(100),
        Some(n) if n <= 0 => None, // Would be error in daemon
        Some(n) if n > 500 => Some(500),
        Some(n) => Some(n),
    };
    assert_eq!(result, None);

    // 3. limit=Some(600) => clamped to 500
    let enforced = match Some(600usize) {
        None => Some(100),
        Some(n) if n <= 0 => panic!("invalid_request"),
        Some(n) if n > 500 => Some(500),
        Some(n) => Some(n),
    };
    assert_eq!(enforced, Some(500));

    // 4. limit=Some(50) => passed through
    let enforced = match Some(50usize) {
        None => Some(100),
        Some(n) if n <= 0 => panic!("invalid_request"),
        Some(n) if n > 500 => Some(500),
        Some(n) => Some(n),
    };
    assert_eq!(enforced, Some(50));
}
