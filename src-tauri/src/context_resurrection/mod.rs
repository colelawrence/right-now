//! Context Resurrection module
//!
//! Provides snapshot capture, storage, and query capabilities for task context.
//! Designed to enable quick task re-entry after interruptions.

pub mod capture;
pub mod models;
pub mod query;
pub mod store;

// Re-export key types
pub use capture::{SessionProvider, SessionSnapshot};
pub use models::{CaptureReason, ContextSnapshotV1, SnapshotId};
pub use store::SnapshotStore;
