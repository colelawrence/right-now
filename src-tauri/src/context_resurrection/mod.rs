//! Context Resurrection module
//!
//! Provides snapshot capture, storage, and query capabilities for task context.
//! Designed to enable quick task re-entry after interruptions.
//!
//! ## Key features
//! - Atomic snapshot writes with per-task flock coordination
//! - Retention/pruning (default: last 5 snapshots per task)
//! - Safe deletion APIs (delete_task, delete_project)
//! - Graceful degradation when storage is unavailable

pub mod capture;
pub mod models;
pub mod query;
pub mod store;

// Re-export key types
pub use capture::{SessionProvider, SessionSnapshot};
pub use models::{CaptureReason, ContextSnapshotV1, SnapshotId};
pub use store::SnapshotStore;
