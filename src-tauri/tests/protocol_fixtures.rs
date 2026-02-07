//! Protocol fixture tests - validate JSON files can deserialize to Rust types
//!
//! These fixtures are shared between Rust and TypeScript to ensure protocol compatibility.

use rn_desktop_2_lib::context_resurrection::models::{
    AttentionType, CaptureReason, ContextSnapshotV1, SessionStatus, TerminalContext,
};
use rn_desktop_2_lib::session::protocol::{DaemonErrorCode, DaemonRequest, DaemonResponse};
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    PathBuf::from(manifest_dir)
        .parent()
        .unwrap()
        .join("test/fixtures/protocol")
        .join(name)
}

fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(fixture_path(name)).unwrap()
}

#[test]
fn test_cr_latest_request() {
    let json = read_fixture("cr_latest.json");
    let req: DaemonRequest = serde_json::from_str(&json).unwrap();

    match req {
        DaemonRequest::CrLatest {
            project_path,
            task_id,
        } => {
            assert_eq!(project_path, "/Users/test/projects/app/TODO.md");
            assert_eq!(task_id, Some("abc.test-task".to_string()));
        }
        _ => panic!("Expected CrLatest variant"),
    }
}

#[test]
fn test_cr_list_request() {
    let json = read_fixture("cr_list.json");
    let req: DaemonRequest = serde_json::from_str(&json).unwrap();

    match req {
        DaemonRequest::CrList {
            project_path,
            task_id,
            limit,
        } => {
            assert_eq!(project_path, "/Users/test/projects/app/TODO.md");
            assert_eq!(task_id, "abc.test-task");
            assert_eq!(limit, Some(50));
        }
        _ => panic!("Expected CrList variant"),
    }
}

#[test]
fn test_cr_get_request() {
    let json = read_fixture("cr_get.json");
    let req: DaemonRequest = serde_json::from_str(&json).unwrap();

    match req {
        DaemonRequest::CrGet {
            project_path,
            task_id,
            snapshot_id,
        } => {
            assert_eq!(project_path, "/Users/test/projects/app/TODO.md");
            assert_eq!(task_id, "abc.test-task");
            assert_eq!(snapshot_id, "2026-02-07T10:30:00Z_abc.test-task");
        }
        _ => panic!("Expected CrGet variant"),
    }
}

#[test]
fn test_cr_capture_now_request() {
    let json = read_fixture("cr_capture_now.json");
    let req: DaemonRequest = serde_json::from_str(&json).unwrap();

    match req {
        DaemonRequest::CrCaptureNow {
            project_path,
            task_id,
            user_note,
        } => {
            assert_eq!(project_path, "/Users/test/projects/app/TODO.md");
            assert_eq!(task_id, "abc.test-task");
            assert_eq!(
                user_note,
                Some("Manual snapshot before refactoring".to_string())
            );
        }
        _ => panic!("Expected CrCaptureNow variant"),
    }
}

#[test]
fn test_cr_delete_task_request() {
    let json = read_fixture("cr_delete_task.json");
    let req: DaemonRequest = serde_json::from_str(&json).unwrap();

    match req {
        DaemonRequest::CrDeleteTask {
            project_path,
            task_id,
        } => {
            assert_eq!(project_path, "/Users/test/projects/app/TODO.md");
            assert_eq!(task_id, "abc.test-task");
        }
        _ => panic!("Expected CrDeleteTask variant"),
    }
}

#[test]
fn test_cr_delete_project_request() {
    let json = read_fixture("cr_delete_project.json");
    let req: DaemonRequest = serde_json::from_str(&json).unwrap();

    match req {
        DaemonRequest::CrDeleteProject { project_path } => {
            assert_eq!(project_path, "/Users/test/projects/app/TODO.md");
        }
        _ => panic!("Expected CrDeleteProject variant"),
    }
}

#[test]
fn test_cr_snapshot_response() {
    let json = read_fixture("cr_snapshot.json");
    let resp: DaemonResponse = serde_json::from_str(&json).unwrap();

    match resp {
        DaemonResponse::CrSnapshot { snapshot } => {
            let snapshot = snapshot.expect("Expected snapshot to be Some");
            assert_eq!(snapshot.id, "2026-02-07T10:30:00Z_abc.test-task");
            assert_eq!(snapshot.version, 1);
            assert_eq!(snapshot.project_path, "/Users/test/projects/app/TODO.md");
            assert_eq!(snapshot.task_id, "abc.test-task");
            assert_eq!(snapshot.task_title_at_capture, "Implement feature X");
            assert_eq!(snapshot.captured_at, "2026-02-07T10:30:00Z");
            assert_eq!(snapshot.capture_reason, CaptureReason::SessionStopped);

            let terminal = snapshot.terminal.expect("Expected terminal context");
            assert_eq!(terminal.session_id, 42);
            assert_eq!(terminal.status, SessionStatus::Stopped);
            assert_eq!(terminal.exit_code, Some(0));
            assert_eq!(
                terminal.tail_inline,
                Some("Build completed successfully\n".to_string())
            );
        }
        _ => panic!("Expected CrSnapshot variant"),
    }
}

#[test]
fn test_cr_snapshot_null_response() {
    let json = read_fixture("cr_snapshot_null.json");
    let resp: DaemonResponse = serde_json::from_str(&json).unwrap();

    match resp {
        DaemonResponse::CrSnapshot { snapshot } => {
            assert!(snapshot.is_none(), "Expected snapshot to be None");
        }
        _ => panic!("Expected CrSnapshot variant"),
    }
}

#[test]
fn test_cr_snapshots_response() {
    let json = read_fixture("cr_snapshots.json");
    let resp: DaemonResponse = serde_json::from_str(&json).unwrap();

    match resp {
        DaemonResponse::CrSnapshots { snapshots } => {
            assert_eq!(snapshots.len(), 2);

            // First snapshot (manual)
            assert_eq!(snapshots[0].id, "2026-02-07T11:00:00Z_abc.test-task");
            assert_eq!(snapshots[0].capture_reason, CaptureReason::Manual);
            assert_eq!(
                snapshots[0].user_note,
                Some("Before refactoring".to_string())
            );
            assert!(snapshots[0].terminal.is_none());

            // Second snapshot (stopped)
            assert_eq!(snapshots[1].id, "2026-02-07T10:30:00Z_abc.test-task");
            assert_eq!(snapshots[1].capture_reason, CaptureReason::SessionStopped);
            assert!(snapshots[1].terminal.is_some());
        }
        _ => panic!("Expected CrSnapshots variant"),
    }
}

#[test]
fn test_cr_deleted_response() {
    let json = read_fixture("cr_deleted.json");
    let resp: DaemonResponse = serde_json::from_str(&json).unwrap();

    match resp {
        DaemonResponse::CrDeleted { deleted_count } => {
            assert_eq!(deleted_count, 5);
        }
        _ => panic!("Expected CrDeleted variant"),
    }
}

#[test]
fn test_error_response() {
    let json = read_fixture("error.json");
    let resp: DaemonResponse = serde_json::from_str(&json).unwrap();

    match resp {
        DaemonResponse::Error { code, message } => {
            assert_eq!(code, DaemonErrorCode::NotFound);
            assert_eq!(message, "Snapshot not found");
        }
        _ => panic!("Expected Error variant"),
    }
}

#[test]
fn test_all_error_codes_deserialize() {
    let codes = vec![
        ("not_found", DaemonErrorCode::NotFound),
        ("skipped", DaemonErrorCode::Skipped),
        ("invalid_request", DaemonErrorCode::InvalidRequest),
        ("store_unavailable", DaemonErrorCode::StoreUnavailable),
        ("internal", DaemonErrorCode::Internal),
        ("daemon_unavailable", DaemonErrorCode::DaemonUnavailable),
        ("timeout", DaemonErrorCode::Timeout),
    ];

    for (json_str, expected_code) in codes {
        let json = format!(
            r#"{{"type": "error", "code": "{}", "message": "Test"}}"#,
            json_str
        );
        let resp: DaemonResponse = serde_json::from_str(&json).unwrap();

        match resp {
            DaemonResponse::Error { code, .. } => {
                assert_eq!(code, expected_code, "Failed for code: {}", json_str);
            }
            _ => panic!("Expected Error variant for code: {}", json_str),
        }
    }
}

#[test]
fn test_all_capture_reasons_deserialize() {
    let reasons = vec![
        ("session_stopped", CaptureReason::SessionStopped),
        ("session_waiting", CaptureReason::SessionWaiting),
        ("session_running", CaptureReason::SessionRunning),
        ("idle_timeout", CaptureReason::IdleTimeout),
        ("manual", CaptureReason::Manual),
    ];

    for (json_str, expected_reason) in reasons {
        let snapshot_json = format!(
            r#"{{
                "id": "test",
                "version": 1,
                "project_path": "/test",
                "task_id": "test",
                "task_title_at_capture": "Test",
                "captured_at": "2026-02-07T00:00:00Z",
                "capture_reason": "{}"
            }}"#,
            json_str
        );

        let snapshot: ContextSnapshotV1 = serde_json::from_str(&snapshot_json).unwrap();
        assert_eq!(
            snapshot.capture_reason, expected_reason,
            "Failed for reason: {}",
            json_str
        );
    }
}

#[test]
fn test_session_status_variants() {
    let variants = vec![
        ("Running", SessionStatus::Running),
        ("Waiting", SessionStatus::Waiting),
        ("Stopped", SessionStatus::Stopped),
    ];

    for (json_str, expected_status) in variants {
        let terminal_json = format!(r#"{{"session_id": 1, "status": "{}"}}"#, json_str);

        let terminal: TerminalContext = serde_json::from_str(&terminal_json).unwrap();
        assert_eq!(
            terminal.status, expected_status,
            "Failed for status: {}",
            json_str
        );
    }
}

#[test]
fn test_attention_type_variants() {
    let variants = vec![
        ("input_required", AttentionType::InputRequired),
        ("decision_point", AttentionType::DecisionPoint),
        ("completed", AttentionType::Completed),
        ("error", AttentionType::Error),
    ];

    for (json_str, expected_type) in variants {
        let attention_json = format!(
            r#"{{"attention_type": "{}", "preview": "test", "triggered_at": "2026-02-07T00:00:00Z"}}"#,
            json_str
        );

        let attention: rn_desktop_2_lib::context_resurrection::models::AttentionSummary =
            serde_json::from_str(&attention_json).unwrap();
        assert_eq!(
            attention.attention_type, expected_type,
            "Failed for attention type: {}",
            json_str
        );
    }
}

#[test]
fn test_handshake_request() {
    let json = read_fixture("handshake_request.json");
    let req: DaemonRequest = serde_json::from_str(&json).unwrap();

    match req {
        DaemonRequest::Handshake { client_version } => {
            assert_eq!(client_version, 1);
        }
        _ => panic!("Expected Handshake variant"),
    }
}

#[test]
fn test_handshake_response() {
    let json = read_fixture("handshake_response.json");
    let resp: DaemonResponse = serde_json::from_str(&json).unwrap();

    match resp {
        DaemonResponse::Handshake { protocol_version } => {
            assert_eq!(protocol_version, 1);
        }
        _ => panic!("Expected Handshake variant"),
    }
}

#[test]
fn test_error_version_mismatch() {
    let json = read_fixture("error_version_mismatch.json");
    let resp: DaemonResponse = serde_json::from_str(&json).unwrap();

    match resp {
        DaemonResponse::Error { code, message } => {
            assert_eq!(code, DaemonErrorCode::VersionMismatch);
            assert_eq!(message, "Daemon is newer than app—please update the app.");
        }
        _ => panic!("Expected Error variant"),
    }
}
