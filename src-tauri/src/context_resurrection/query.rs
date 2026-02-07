//! RPC handlers for Context Resurrection requests
//!
//! Implementation stub - full implementation comes in Phase 1 per plan.
//!
//! Will implement:
//! - handle_cr_request() to process DaemonRequest variants:
//!   - CrLatest { project_path, task_id }
//!   - CrList { project_path, task_id, limit }
//!   - CrGet { snapshot_id }
//!   - CrCaptureNow { project_path, task_id, reason, user_note }
//!   - CrDeleteTask { project_path, task_id }
//!   - CrDeleteProject { project_path }

// Query handlers will be defined here in Phase 1
