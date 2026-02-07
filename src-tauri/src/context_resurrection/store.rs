//! Snapshot store: disk I/O, paths, atomic writes, retention
//!
//! Provides foundation for Context Resurrection snapshot storage:
//! - Project-hash-based directory layout
//! - Atomic writes (temp file + fsync + rename)
//! - Strict file permissions (0600 files, 0700 dirs)
//! - Bounded startup cleanup of stale temp files
//! - Availability flag with graceful degradation

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

use super::models::ContextSnapshotV1;

/// Maximum number of files to scan per project-hash directory during cleanup
const CLEANUP_SCAN_LIMIT: usize = 1000;

/// Age threshold for temp file cleanup (1 hour)
const CLEANUP_AGE_THRESHOLD: Duration = Duration::from_secs(3600);

/// Snapshot store for Context Resurrection
///
/// Manages on-disk storage of snapshots under:
/// `~/.right-now/context-resurrection/snapshots/<project-hash>/<task-id>/<snapshot-id>.json`
#[derive(Debug, Clone)]
pub struct SnapshotStore {
    /// Base directory: ~/.right-now/context-resurrection/snapshots/
    base_dir: PathBuf,
    /// Whether the store is available (base dir successfully created)
    available: bool,
}

impl SnapshotStore {
    /// Create a new snapshot store
    ///
    /// Attempts to create the base directory with strict permissions (0700).
    /// If creation fails, the store is marked unavailable and all operations
    /// will return errors indicating unavailability.
    pub fn new(daemon_data_dir: &Path) -> Self {
        let base_dir = daemon_data_dir
            .join("context-resurrection")
            .join("snapshots");

        let available = Self::ensure_base_dir(&base_dir);

        Self {
            base_dir,
            available,
        }
    }

    /// Check if the store is available
    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Ensure base directory exists with strict permissions
    fn ensure_base_dir(dir: &Path) -> bool {
        match fs::create_dir_all(dir) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Err(e) = fs::set_permissions(dir, fs::Permissions::from_mode(0o700)) {
                        eprintln!(
                            "Warning: Failed to set permissions on {}: {}",
                            dir.display(),
                            e
                        );
                        return false;
                    }
                }
                true
            }
            Err(e) => {
                eprintln!(
                    "Error: Failed to create snapshot base directory {}: {}",
                    dir.display(),
                    e
                );
                false
            }
        }
    }

    /// Compute project hash from canonical TODO.md path
    ///
    /// Returns SHA-256 hash truncated to 16 hex characters
    pub fn project_hash(project_path: &Path) -> String {
        let canonical = project_path
            .canonicalize()
            .unwrap_or_else(|_| project_path.to_path_buf());

        let mut hasher = Sha256::new();
        hasher.update(canonical.as_os_str().as_encoded_bytes());
        let hash = hasher.finalize();

        // Truncate to 16 hex chars
        hex::encode(&hash[..8])
    }

    /// Get the directory for a specific project
    fn project_dir(&self, project_path: &Path) -> PathBuf {
        let hash = Self::project_hash(project_path);
        self.base_dir.join(hash)
    }

    /// Get the directory for a specific task within a project
    fn task_dir(&self, project_path: &Path, task_id: &str) -> PathBuf {
        self.project_dir(project_path).join(task_id)
    }

    /// Get the path for a snapshot JSON file
    fn snapshot_path(&self, project_path: &Path, task_id: &str, snapshot_id: &str) -> PathBuf {
        self.task_dir(project_path, task_id)
            .join(format!("{}.json", snapshot_id))
    }

    /// Ensure task directory exists with strict permissions
    fn ensure_task_dir(&self, project_path: &Path, task_id: &str) -> io::Result<PathBuf> {
        if !self.available {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "SnapshotStore is unavailable (base directory creation failed)",
            ));
        }

        let task_dir = self.task_dir(project_path, task_id);
        fs::create_dir_all(&task_dir)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Set 0700 on all parent directories
            let project_dir = self.project_dir(project_path);
            for dir in [&project_dir, &task_dir] {
                fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
            }
        }

        Ok(task_dir)
    }

    /// Write a snapshot to disk atomically
    ///
    /// Uses temp file + fsync + rename pattern to ensure readers never see partial writes.
    /// Files are created with 0600 permissions on Unix.
    pub fn write_snapshot(
        &self,
        project_path: &Path,
        task_id: &str,
        snapshot: &ContextSnapshotV1,
    ) -> io::Result<PathBuf> {
        let task_dir = self.ensure_task_dir(project_path, task_id)?;
        let final_path = self.snapshot_path(project_path, task_id, &snapshot.id);

        // Write to temp file in same directory
        let temp_name = format!("{}.json.tmp.{}", snapshot.id, std::process::id());
        let temp_path = task_dir.join(&temp_name);

        // Serialize snapshot
        let json = serde_json::to_string_pretty(snapshot)?;

        // Write temp file
        let mut file = fs::File::create(&temp_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }

        file.write_all(json.as_bytes())?;
        file.sync_all()?; // fsync
        drop(file);

        // Atomic rename
        fs::rename(&temp_path, &final_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Ensure final file has correct permissions (in case rename didn't preserve)
            fs::set_permissions(&final_path, fs::Permissions::from_mode(0o600))?;
        }

        Ok(final_path)
    }

    /// Cleanup stale temp files in a project directory
    ///
    /// Deletes *.tmp.* files older than CLEANUP_AGE_THRESHOLD (1 hour).
    /// Scans at most CLEANUP_SCAN_LIMIT files to avoid blocking startup.
    ///
    /// Returns (deleted_count, scanned_count, hit_limit)
    pub fn cleanup_stale_temps(&self, project_path: &Path) -> io::Result<(usize, usize, bool)> {
        if !self.available {
            return Ok((0, 0, false));
        }

        let project_dir = self.project_dir(project_path);
        if !project_dir.exists() {
            return Ok((0, 0, false));
        }

        let mut scanned = 0;
        let mut deleted = 0;
        let now = SystemTime::now();

        // Walk project directory recursively
        for entry in walkdir::WalkDir::new(&project_dir)
            .max_depth(3) // project-hash/<task-id>/<file>
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if scanned >= CLEANUP_SCAN_LIMIT {
                eprintln!(
                    "Warning: Hit cleanup scan limit ({}) for project {}",
                    CLEANUP_SCAN_LIMIT,
                    project_dir.display()
                );
                return Ok((deleted, scanned, true));
            }

            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            scanned += 1;

            // Check if filename matches *.tmp.* pattern
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.contains(".tmp.") {
                    // Check age
                    if let Ok(metadata) = fs::metadata(path) {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(age) = now.duration_since(modified) {
                                if age > CLEANUP_AGE_THRESHOLD {
                                    if let Err(e) = fs::remove_file(path) {
                                        eprintln!(
                                            "Warning: Failed to delete stale temp file {}: {}",
                                            path.display(),
                                            e
                                        );
                                    } else {
                                        deleted += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok((deleted, scanned, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_project_hash_deterministic() {
        let path1 = PathBuf::from("/Users/test/project/TODO.md");
        let path2 = PathBuf::from("/Users/test/project/TODO.md");
        let path3 = PathBuf::from("/Users/other/project/TODO.md");

        let hash1 = SnapshotStore::project_hash(&path1);
        let hash2 = SnapshotStore::project_hash(&path2);
        let hash3 = SnapshotStore::project_hash(&path3);

        // Same path produces same hash
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 16); // 16 hex chars

        // Different path produces different hash
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_store_availability() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        assert!(store.is_available());
        assert!(store.base_dir.exists());
    }

    #[test]
    fn test_unavailable_store_on_bad_path() {
        // Try to create store in a path that can't be created
        let bad_path = PathBuf::from("/dev/null/cannot-create-here");
        let store = SnapshotStore::new(&bad_path);

        assert!(!store.is_available());
    }

    #[test]
    fn test_atomic_write_produces_final_file() {
        use super::super::models::{CaptureReason, ContextSnapshotV1};

        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        let snapshot = ContextSnapshotV1::new(
            "2026-02-06T13:12:33Z_abc.test-task".to_string(),
            project_path.to_string_lossy().to_string(),
            "abc.test-task".to_string(),
            "Test task".to_string(),
            "2026-02-06T13:12:33Z".to_string(),
            CaptureReason::Manual,
        );

        let result = store.write_snapshot(&project_path, "abc.test-task", &snapshot);
        assert!(result.is_ok());

        let written_path = result.unwrap();
        assert!(written_path.exists());
        assert!(written_path.ends_with("2026-02-06T13:12:33Z_abc.test-task.json"));

        // Verify no temp file remains
        let task_dir = store.task_dir(&project_path, "abc.test-task");
        let temp_files: Vec<_> = fs::read_dir(&task_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map_or(false, |n| n.contains(".tmp."))
            })
            .collect();

        assert_eq!(temp_files.len(), 0, "Temp files should be cleaned up");

        // Verify content can be deserialized
        let content = fs::read_to_string(&written_path).unwrap();
        let deserialized: ContextSnapshotV1 = serde_json::from_str(&content).unwrap();
        assert_eq!(deserialized.id, snapshot.id);
    }

    #[cfg(unix)]
    #[test]
    fn test_permissions_are_correct() {
        use super::super::models::{CaptureReason, ContextSnapshotV1};
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        let snapshot = ContextSnapshotV1::new(
            "2026-02-06T13:12:33Z_xyz.perms-test".to_string(),
            project_path.to_string_lossy().to_string(),
            "xyz.perms-test".to_string(),
            "Perms test".to_string(),
            "2026-02-06T13:12:33Z".to_string(),
            CaptureReason::SessionStopped,
        );

        let written_path = store
            .write_snapshot(&project_path, "xyz.perms-test", &snapshot)
            .unwrap();

        // Check file permissions (0600)
        let file_metadata = fs::metadata(&written_path).unwrap();
        let file_mode = file_metadata.permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600, "File should have 0600 permissions");

        // Check directory permissions (0700)
        let task_dir = store.task_dir(&project_path, "xyz.perms-test");
        let dir_metadata = fs::metadata(&task_dir).unwrap();
        let dir_mode = dir_metadata.permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700, "Directory should have 0700 permissions");
    }

    #[test]
    fn test_cleanup_stale_temps() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        // Create task directory
        let task_dir = store.task_dir(&project_path, "test.task");
        fs::create_dir_all(&task_dir).unwrap();

        // Create a fresh temp file (should not be deleted)
        let fresh_temp = task_dir.join("snapshot.json.tmp.12345");
        fs::write(&fresh_temp, "fresh").unwrap();

        // Create an old temp file (should be deleted)
        let old_temp = task_dir.join("old-snapshot.json.tmp.99999");
        fs::write(&old_temp, "old").unwrap();

        // Set old temp file's mtime to 2 hours ago
        let two_hours_ago = SystemTime::now() - Duration::from_secs(7200);
        filetime::set_file_mtime(
            &old_temp,
            filetime::FileTime::from_system_time(two_hours_ago),
        )
        .unwrap();

        // Run cleanup
        let (deleted, scanned, hit_limit) = store.cleanup_stale_temps(&project_path).unwrap();

        assert!(!hit_limit);
        assert_eq!(deleted, 1, "Should delete 1 old temp file");
        assert!(scanned > 0);

        // Verify old temp was deleted, fresh temp remains
        assert!(!old_temp.exists(), "Old temp should be deleted");
        assert!(fresh_temp.exists(), "Fresh temp should remain");
    }

    #[test]
    fn test_unavailable_store_returns_error() {
        use super::super::models::{CaptureReason, ContextSnapshotV1};

        let bad_path = PathBuf::from("/dev/null/cannot-create");
        let store = SnapshotStore::new(&bad_path);

        assert!(!store.is_available());

        let project_path = PathBuf::from("/tmp/TODO.md");
        let snapshot = ContextSnapshotV1::new(
            "2026-02-06T13:12:33Z_err.test".to_string(),
            project_path.to_string_lossy().to_string(),
            "err.test".to_string(),
            "Error test".to_string(),
            "2026-02-06T13:12:33Z".to_string(),
            CaptureReason::Manual,
        );

        let result = store.write_snapshot(&project_path, "err.test", &snapshot);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unavailable"));
    }
}
