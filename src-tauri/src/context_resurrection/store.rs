//! Snapshot store: disk I/O, paths, atomic writes, retention
//!
//! Implementation stub - full implementation comes in Phase 1 per plan.
//!
//! Will implement:
//! - paths (with SHA-256 project hash, 16 hex chars)
//! - atomic writes (temp file + fsync + rename pattern per §1.3.1)
//! - file permissions (0600 for files, 0700 for directories per §0.13.2)
//! - retention/pruning with flock coordination per §1.3.2
//! - list/latest/get
//! - delete (per-task, per-project)
//! - availability flag (cr_store_available) with graceful degradation per §0.12.2
//! - startup orphan cleanup (bounded to 1000 files per project-hash per §1.3.1)
//! - missing tail_path handling (treat as null, log debug warning per §1.3.1)

// SnapshotStore struct will be defined here in Phase 1
