//! Snapshot store: disk I/O, paths, atomic writes, retention
//!
//! Provides foundation for Context Resurrection snapshot storage:
//! - Project-hash-based directory layout
//! - Atomic writes (temp file + fsync + rename)
//! - Strict file permissions (0600 files, 0700 dirs)
//! - Bounded startup cleanup of stale temp files
//! - Availability flag with graceful degradation
//! - Per-task flock coordination for safe concurrent access

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use fs2::FileExt;
use sha2::{Digest, Sha256};

use super::models::ContextSnapshotV1;

/// Maximum number of files to scan per project-hash directory during cleanup
const CLEANUP_SCAN_LIMIT: usize = 1000;

/// Age threshold for temp file cleanup (1 hour)
const CLEANUP_AGE_THRESHOLD: Duration = Duration::from_secs(3600);

/// Lock acquisition timeout (500ms per plan §1.3.3)
const LOCK_TIMEOUT: Duration = Duration::from_millis(500);

/// Default retention count: last N snapshots per task
const DEFAULT_RETENTION_COUNT: usize = 5;

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

    /// Public accessor for task directory (used by CaptureService for lock files)
    pub fn task_dir_public(&self, project_path: &Path, task_id: &str) -> PathBuf {
        self.task_dir(project_path, task_id)
    }

    /// Get the path for a snapshot JSON file
    fn snapshot_path(&self, project_path: &Path, task_id: &str, snapshot_id: &str) -> PathBuf {
        self.task_dir(project_path, task_id)
            .join(format!("{}.json", snapshot_id))
    }

    /// Get the path for the task lock file
    fn lock_path(&self, project_path: &Path, task_id: &str) -> PathBuf {
        self.task_dir(project_path, task_id).join(".lock")
    }

    /// Acquire an exclusive lock on a task directory
    ///
    /// Returns a locked file handle. The lock is released when the file is dropped.
    /// Timeout is LOCK_TIMEOUT (500ms).
    fn acquire_task_lock(&self, project_path: &Path, task_id: &str) -> io::Result<fs::File> {
        let lock_path = self.lock_path(project_path, task_id);

        // Ensure parent directory exists
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let lock_file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&lock_path)?;

        // Try to acquire lock with timeout
        let start = std::time::Instant::now();
        loop {
            match lock_file.try_lock_exclusive() {
                Ok(()) => return Ok(lock_file),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if start.elapsed() >= LOCK_TIMEOUT {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            format!(
                                "Failed to acquire task lock within {:?} for task {}",
                                LOCK_TIMEOUT, task_id
                            ),
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => return Err(e),
            }
        }
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
    /// Acquires a per-task flock before writing to coordinate with pruning/deletion.
    pub fn write_snapshot(
        &self,
        project_path: &Path,
        task_id: &str,
        snapshot: &ContextSnapshotV1,
    ) -> io::Result<PathBuf> {
        let task_dir = self.ensure_task_dir(project_path, task_id)?;
        let final_path = self.snapshot_path(project_path, task_id, &snapshot.id);

        // Acquire task lock before writing (per §1.3.3)
        let _lock = self.acquire_task_lock(project_path, task_id)?;

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

    /// Read a snapshot by ID
    ///
    /// If the snapshot contains a `tail_path` reference but the file does not exist,
    /// the snapshot is returned with `tail_path` cleared (None) and a debug warning logged.
    /// This ensures missing tail files do not prevent snapshot reads.
    pub fn read_snapshot(
        &self,
        project_path: &Path,
        task_id: &str,
        snapshot_id: &str,
    ) -> io::Result<ContextSnapshotV1> {
        if !self.available {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "SnapshotStore is unavailable",
            ));
        }

        let snapshot_path = self.snapshot_path(project_path, task_id, snapshot_id);
        let content = fs::read_to_string(&snapshot_path)?;
        let mut snapshot: ContextSnapshotV1 = serde_json::from_str(&content)?;

        // Handle missing tail_path (per §1.3.1)
        if let Some(ref terminal) = snapshot.terminal {
            if let Some(ref tail_path) = terminal.tail_path {
                if !Path::new(tail_path).exists() {
                    eprintln!(
                        "Debug: Missing tail_path for snapshot {}: {}",
                        snapshot_id, tail_path
                    );
                    // Clear the tail_path reference
                    if let Some(ref mut term) = snapshot.terminal {
                        term.tail_path = None;
                    }
                }
            }
        }

        Ok(snapshot)
    }

    /// List snapshots for a specific task
    ///
    /// Returns snapshots sorted by captured_at timestamp (newest first).
    /// If `limit` is specified, returns at most that many snapshots.
    pub fn list_snapshots(
        &self,
        project_path: &Path,
        task_id: &str,
        limit: Option<usize>,
    ) -> io::Result<Vec<ContextSnapshotV1>> {
        if !self.available {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "SnapshotStore is unavailable",
            ));
        }

        let task_dir = self.task_dir(project_path, task_id);
        if !task_dir.exists() {
            return Ok(Vec::new());
        }

        let mut snapshots = Vec::new();

        for entry in fs::read_dir(&task_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Only process .json files (skip .tmp.*, tail files, etc.)
            if !path.is_file() {
                continue;
            }
            if let Some(ext) = path.extension() {
                if ext != "json" {
                    continue;
                }
            } else {
                continue;
            }

            // Extract snapshot_id from filename
            if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                match self.read_snapshot(project_path, task_id, filename) {
                    Ok(snapshot) => snapshots.push(snapshot),
                    Err(e) => {
                        eprintln!("Warning: Failed to read snapshot {}: {}", path.display(), e);
                        // Continue processing other snapshots
                    }
                }
            }
        }

        // Sort by captured_at timestamp (newest first)
        snapshots.sort_by(|a, b| b.captured_at.cmp(&a.captured_at));

        // Apply limit if specified
        if let Some(limit) = limit {
            snapshots.truncate(limit);
        }

        Ok(snapshots)
    }

    /// Get the latest snapshot for a specific task
    ///
    /// Returns the snapshot with the most recent captured_at timestamp,
    /// or None if no snapshots exist.
    pub fn latest_snapshot(
        &self,
        project_path: &Path,
        task_id: &str,
    ) -> io::Result<Option<ContextSnapshotV1>> {
        let snapshots = self.list_snapshots(project_path, task_id, Some(1))?;
        Ok(snapshots.into_iter().next())
    }

    /// Prune old snapshots for a specific task
    ///
    /// Retains the last N snapshots (default: 5) and deletes older ones.
    /// Acquires a per-task flock before deleting to coordinate with capture.
    ///
    /// Returns the number of snapshots deleted.
    pub fn prune_snapshots(
        &self,
        project_path: &Path,
        task_id: &str,
        retain_count: Option<usize>,
    ) -> io::Result<usize> {
        if !self.available {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "SnapshotStore is unavailable",
            ));
        }

        let retain_count = retain_count.unwrap_or(DEFAULT_RETENTION_COUNT);

        // Acquire task lock before pruning (per §1.3.2)
        let _lock = self.acquire_task_lock(project_path, task_id)?;

        // List all snapshots sorted by timestamp (newest first)
        let snapshots = self.list_snapshots(project_path, task_id, None)?;

        if snapshots.len() <= retain_count {
            return Ok(0);
        }

        let mut deleted = 0;

        // Delete snapshots beyond the retain count
        for snapshot in snapshots.iter().skip(retain_count) {
            let snapshot_path = self.snapshot_path(project_path, task_id, &snapshot.id);

            match fs::remove_file(&snapshot_path) {
                Ok(()) => {
                    deleted += 1;
                }
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to delete snapshot {}: {}",
                        snapshot_path.display(),
                        e
                    );
                }
            }
        }

        Ok(deleted)
    }

    /// Delete all snapshots for a specific task
    ///
    /// Acquires a per-task flock before deleting to coordinate with capture.
    /// Returns the number of snapshots deleted.
    pub fn delete_task(&self, project_path: &Path, task_id: &str) -> io::Result<usize> {
        if !self.available {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "SnapshotStore is unavailable",
            ));
        }

        let task_dir = self.task_dir(project_path, task_id);
        if !task_dir.exists() {
            return Ok(0);
        }

        // Acquire task lock before deleting (per §1.3.2)
        let _lock = self.acquire_task_lock(project_path, task_id)?;

        let mut deleted = 0;

        // Delete all .json snapshot files
        for entry in fs::read_dir(&task_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            if let Some(ext) = path.extension() {
                if ext == "json" {
                    match fs::remove_file(&path) {
                        Ok(()) => deleted += 1,
                        Err(e) => {
                            eprintln!(
                                "Warning: Failed to delete snapshot {}: {}",
                                path.display(),
                                e
                            );
                        }
                    }
                }
            }
        }

        // Drop the lock before removing the directory
        drop(_lock);

        // Remove the task directory itself (including lock file)
        if let Err(e) = fs::remove_dir_all(&task_dir) {
            eprintln!(
                "Warning: Failed to remove task directory {}: {}",
                task_dir.display(),
                e
            );
        }

        Ok(deleted)
    }

    /// Delete all snapshots for a specific project
    ///
    /// Iterates through all task directories and deletes them with proper locking.
    /// Returns the total number of snapshots deleted.
    pub fn delete_project(&self, project_path: &Path) -> io::Result<usize> {
        if !self.available {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "SnapshotStore is unavailable",
            ));
        }

        let project_dir = self.project_dir(project_path);
        if !project_dir.exists() {
            return Ok(0);
        }

        let mut total_deleted = 0;

        // Find all task directories
        let task_dirs: Vec<_> = fs::read_dir(&project_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        // Delete each task with proper locking
        for task_entry in task_dirs {
            let task_dir_path = task_entry.path();
            if let Some(task_id) = task_dir_path.file_name().and_then(|n| n.to_str()) {
                match self.delete_task(project_path, task_id) {
                    Ok(count) => total_deleted += count,
                    Err(e) => {
                        eprintln!("Warning: Failed to delete task {}: {}", task_id, e);
                    }
                }
            }
        }

        // Remove the project directory itself
        if let Err(e) = fs::remove_dir_all(&project_dir) {
            eprintln!(
                "Warning: Failed to remove project directory {}: {}",
                project_dir.display(),
                e
            );
        }

        Ok(total_deleted)
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

    #[test]
    fn test_read_snapshot_by_id() {
        use super::super::models::{CaptureReason, ContextSnapshotV1};

        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        let snapshot = ContextSnapshotV1::new(
            "2026-02-06T13:12:33Z_abc.read-test".to_string(),
            project_path.to_string_lossy().to_string(),
            "abc.read-test".to_string(),
            "Read test task".to_string(),
            "2026-02-06T13:12:33Z".to_string(),
            CaptureReason::Manual,
        );

        store
            .write_snapshot(&project_path, "abc.read-test", &snapshot)
            .unwrap();

        // Read it back
        let read_snapshot = store
            .read_snapshot(
                &project_path,
                "abc.read-test",
                "2026-02-06T13:12:33Z_abc.read-test",
            )
            .unwrap();

        assert_eq!(read_snapshot.id, snapshot.id);
        assert_eq!(read_snapshot.task_id, snapshot.task_id);
        assert_eq!(read_snapshot.capture_reason, CaptureReason::Manual);
    }

    #[test]
    fn test_list_snapshots_ordering() {
        use super::super::models::{CaptureReason, ContextSnapshotV1};

        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        // Create snapshots with different timestamps
        let snapshot1 = ContextSnapshotV1::new(
            "2026-02-06T10:00:00Z_xyz.list-test".to_string(),
            project_path.to_string_lossy().to_string(),
            "xyz.list-test".to_string(),
            "List test task".to_string(),
            "2026-02-06T10:00:00Z".to_string(), // oldest
            CaptureReason::SessionStopped,
        );

        let snapshot2 = ContextSnapshotV1::new(
            "2026-02-06T11:00:00Z_xyz.list-test".to_string(),
            project_path.to_string_lossy().to_string(),
            "xyz.list-test".to_string(),
            "List test task".to_string(),
            "2026-02-06T11:00:00Z".to_string(), // middle
            CaptureReason::SessionWaiting,
        );

        let snapshot3 = ContextSnapshotV1::new(
            "2026-02-06T12:00:00Z_xyz.list-test".to_string(),
            project_path.to_string_lossy().to_string(),
            "xyz.list-test".to_string(),
            "List test task".to_string(),
            "2026-02-06T12:00:00Z".to_string(), // newest
            CaptureReason::Manual,
        );

        // Write in non-chronological order to test sorting
        store
            .write_snapshot(&project_path, "xyz.list-test", &snapshot2)
            .unwrap();
        store
            .write_snapshot(&project_path, "xyz.list-test", &snapshot1)
            .unwrap();
        store
            .write_snapshot(&project_path, "xyz.list-test", &snapshot3)
            .unwrap();

        // List all snapshots
        let snapshots = store
            .list_snapshots(&project_path, "xyz.list-test", None)
            .unwrap();

        assert_eq!(snapshots.len(), 3);
        // Should be sorted newest first
        assert_eq!(snapshots[0].captured_at, "2026-02-06T12:00:00Z");
        assert_eq!(snapshots[1].captured_at, "2026-02-06T11:00:00Z");
        assert_eq!(snapshots[2].captured_at, "2026-02-06T10:00:00Z");
    }

    #[test]
    fn test_list_snapshots_with_limit() {
        use super::super::models::{CaptureReason, ContextSnapshotV1};

        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        // Create 5 snapshots
        for i in 1..=5 {
            let snapshot = ContextSnapshotV1::new(
                format!("2026-02-06T10:{:02}:00Z_lim.limit-test", i),
                project_path.to_string_lossy().to_string(),
                "lim.limit-test".to_string(),
                "Limit test task".to_string(),
                format!("2026-02-06T10:{:02}:00Z", i),
                CaptureReason::IdleTimeout,
            );
            store
                .write_snapshot(&project_path, "lim.limit-test", &snapshot)
                .unwrap();
        }

        // List with limit of 3
        let limited = store
            .list_snapshots(&project_path, "lim.limit-test", Some(3))
            .unwrap();

        assert_eq!(limited.len(), 3);
        // Should get the 3 newest
        assert!(limited[0].captured_at.contains("10:05:00"));
        assert!(limited[1].captured_at.contains("10:04:00"));
        assert!(limited[2].captured_at.contains("10:03:00"));
    }

    #[test]
    fn test_latest_snapshot() {
        use super::super::models::{CaptureReason, ContextSnapshotV1};

        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        // No snapshots yet
        let latest = store
            .latest_snapshot(&project_path, "lat.latest-test")
            .unwrap();
        assert!(latest.is_none());

        // Create multiple snapshots
        let snapshot1 = ContextSnapshotV1::new(
            "2026-02-06T10:00:00Z_lat.latest-test".to_string(),
            project_path.to_string_lossy().to_string(),
            "lat.latest-test".to_string(),
            "Latest test task".to_string(),
            "2026-02-06T10:00:00Z".to_string(),
            CaptureReason::SessionStopped,
        );

        let snapshot2 = ContextSnapshotV1::new(
            "2026-02-06T11:00:00Z_lat.latest-test".to_string(),
            project_path.to_string_lossy().to_string(),
            "lat.latest-test".to_string(),
            "Latest test task".to_string(),
            "2026-02-06T11:00:00Z".to_string(),
            CaptureReason::Manual,
        );

        store
            .write_snapshot(&project_path, "lat.latest-test", &snapshot1)
            .unwrap();
        store
            .write_snapshot(&project_path, "lat.latest-test", &snapshot2)
            .unwrap();

        // Get latest
        let latest = store
            .latest_snapshot(&project_path, "lat.latest-test")
            .unwrap();

        assert!(latest.is_some());
        let latest = latest.unwrap();
        assert_eq!(latest.captured_at, "2026-02-06T11:00:00Z");
        assert_eq!(latest.capture_reason, CaptureReason::Manual);
    }

    #[test]
    fn test_missing_tail_path_handling() {
        use super::super::models::{
            AttentionSummary, AttentionType, CaptureReason, ContextSnapshotV1, SessionStatus,
            TerminalContext,
        };

        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        // Create a snapshot with a tail_path reference
        let mut snapshot = ContextSnapshotV1::new(
            "2026-02-06T13:30:00Z_mtp.missing-tail".to_string(),
            project_path.to_string_lossy().to_string(),
            "mtp.missing-tail".to_string(),
            "Missing tail test".to_string(),
            "2026-02-06T13:30:00Z".to_string(),
            CaptureReason::SessionStopped,
        );

        let tail_file_path = temp_dir.path().join("test-tail.txt");
        snapshot.terminal = Some(TerminalContext {
            session_id: 99,
            status: SessionStatus::Stopped,
            exit_code: Some(0),
            last_attention: Some(AttentionSummary {
                attention_type: AttentionType::Completed,
                preview: "Build succeeded".to_string(),
                triggered_at: "2026-02-06T13:29:00Z".to_string(),
            }),
            tail_inline: None,
            tail_path: Some(tail_file_path.to_string_lossy().to_string()),
        });

        // Write the tail file initially
        fs::write(&tail_file_path, "Terminal output here").unwrap();

        // Write snapshot
        store
            .write_snapshot(&project_path, "mtp.missing-tail", &snapshot)
            .unwrap();

        // Delete the tail file to simulate missing reference
        fs::remove_file(&tail_file_path).unwrap();

        // Read the snapshot - should succeed with tail_path cleared
        let read_snapshot = store
            .read_snapshot(
                &project_path,
                "mtp.missing-tail",
                "2026-02-06T13:30:00Z_mtp.missing-tail",
            )
            .unwrap();

        // Verify snapshot was read successfully
        assert_eq!(read_snapshot.id, snapshot.id);
        assert!(read_snapshot.terminal.is_some());

        let terminal = read_snapshot.terminal.unwrap();
        // tail_path should be cleared (None)
        assert!(
            terminal.tail_path.is_none(),
            "tail_path should be None when file is missing"
        );
        // Other terminal fields should be intact
        assert_eq!(terminal.session_id, 99);
        assert_eq!(terminal.exit_code, Some(0));
        assert!(terminal.last_attention.is_some());
    }

    #[test]
    fn test_list_snapshots_empty_task() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        // List snapshots for task with no snapshots
        let snapshots = store
            .list_snapshots(&project_path, "emp.empty-task", None)
            .unwrap();

        assert_eq!(snapshots.len(), 0);
    }

    #[test]
    fn test_prune_snapshots_retains_last_n() {
        use super::super::models::{CaptureReason, ContextSnapshotV1};

        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        // Create 7 snapshots
        for i in 1..=7 {
            let snapshot = ContextSnapshotV1::new(
                format!("2026-02-06T10:{:02}:00Z_prn.prune-test", i),
                project_path.to_string_lossy().to_string(),
                "prn.prune-test".to_string(),
                "Prune test task".to_string(),
                format!("2026-02-06T10:{:02}:00Z", i),
                CaptureReason::SessionStopped,
            );
            store
                .write_snapshot(&project_path, "prn.prune-test", &snapshot)
                .unwrap();
        }

        // Verify all 7 exist
        let before = store
            .list_snapshots(&project_path, "prn.prune-test", None)
            .unwrap();
        assert_eq!(before.len(), 7);

        // Prune with default retention (5)
        let deleted = store
            .prune_snapshots(&project_path, "prn.prune-test", None)
            .unwrap();

        assert_eq!(deleted, 2, "Should delete 2 oldest snapshots");

        // Verify only 5 remain
        let after = store
            .list_snapshots(&project_path, "prn.prune-test", None)
            .unwrap();
        assert_eq!(after.len(), 5);

        // Verify the newest 5 remain (timestamps 10:03 through 10:07)
        assert!(after[0].captured_at.contains("10:07:00"));
        assert!(after[1].captured_at.contains("10:06:00"));
        assert!(after[2].captured_at.contains("10:05:00"));
        assert!(after[3].captured_at.contains("10:04:00"));
        assert!(after[4].captured_at.contains("10:03:00"));
    }

    #[test]
    fn test_prune_snapshots_custom_retention() {
        use super::super::models::{CaptureReason, ContextSnapshotV1};

        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        // Create 6 snapshots
        for i in 1..=6 {
            let snapshot = ContextSnapshotV1::new(
                format!("2026-02-06T11:{:02}:00Z_cst.custom-test", i),
                project_path.to_string_lossy().to_string(),
                "cst.custom-test".to_string(),
                "Custom retention test".to_string(),
                format!("2026-02-06T11:{:02}:00Z", i),
                CaptureReason::IdleTimeout,
            );
            store
                .write_snapshot(&project_path, "cst.custom-test", &snapshot)
                .unwrap();
        }

        // Prune with retention of 2
        let deleted = store
            .prune_snapshots(&project_path, "cst.custom-test", Some(2))
            .unwrap();

        assert_eq!(deleted, 4, "Should delete 4 oldest snapshots");

        // Verify only 2 remain
        let after = store
            .list_snapshots(&project_path, "cst.custom-test", None)
            .unwrap();
        assert_eq!(after.len(), 2);
        assert!(after[0].captured_at.contains("11:06:00"));
        assert!(after[1].captured_at.contains("11:05:00"));
    }

    #[test]
    fn test_prune_snapshots_no_deletion_when_under_limit() {
        use super::super::models::{CaptureReason, ContextSnapshotV1};

        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        // Create only 3 snapshots (under default retention of 5)
        for i in 1..=3 {
            let snapshot = ContextSnapshotV1::new(
                format!("2026-02-06T12:{:02}:00Z_nop.no-prune", i),
                project_path.to_string_lossy().to_string(),
                "nop.no-prune".to_string(),
                "No prune test".to_string(),
                format!("2026-02-06T12:{:02}:00Z", i),
                CaptureReason::Manual,
            );
            store
                .write_snapshot(&project_path, "nop.no-prune", &snapshot)
                .unwrap();
        }

        // Prune should delete nothing
        let deleted = store
            .prune_snapshots(&project_path, "nop.no-prune", None)
            .unwrap();

        assert_eq!(deleted, 0, "Should not delete any snapshots");

        // Verify all 3 remain
        let after = store
            .list_snapshots(&project_path, "nop.no-prune", None)
            .unwrap();
        assert_eq!(after.len(), 3);
    }

    #[test]
    fn test_delete_task() {
        use super::super::models::{CaptureReason, ContextSnapshotV1};

        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        // Create snapshots for two tasks
        for i in 1..=3 {
            let snapshot1 = ContextSnapshotV1::new(
                format!("2026-02-06T13:{:02}:00Z_del.delete-test", i),
                project_path.to_string_lossy().to_string(),
                "del.delete-test".to_string(),
                "Task to delete".to_string(),
                format!("2026-02-06T13:{:02}:00Z", i),
                CaptureReason::SessionWaiting,
            );
            store
                .write_snapshot(&project_path, "del.delete-test", &snapshot1)
                .unwrap();

            let snapshot2 = ContextSnapshotV1::new(
                format!("2026-02-06T13:{:02}:00Z_kep.keep-test", i),
                project_path.to_string_lossy().to_string(),
                "kep.keep-test".to_string(),
                "Task to keep".to_string(),
                format!("2026-02-06T13:{:02}:00Z", i),
                CaptureReason::SessionWaiting,
            );
            store
                .write_snapshot(&project_path, "kep.keep-test", &snapshot2)
                .unwrap();
        }

        // Verify both tasks have 3 snapshots
        assert_eq!(
            store
                .list_snapshots(&project_path, "del.delete-test", None)
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            store
                .list_snapshots(&project_path, "kep.keep-test", None)
                .unwrap()
                .len(),
            3
        );

        // Delete one task
        let deleted = store.delete_task(&project_path, "del.delete-test").unwrap();

        assert_eq!(deleted, 3, "Should delete all 3 snapshots");

        // Verify deleted task has no snapshots
        assert_eq!(
            store
                .list_snapshots(&project_path, "del.delete-test", None)
                .unwrap()
                .len(),
            0
        );

        // Verify kept task still has 3 snapshots
        assert_eq!(
            store
                .list_snapshots(&project_path, "kep.keep-test", None)
                .unwrap()
                .len(),
            3
        );

        // Verify task directory was removed
        let task_dir = store.task_dir(&project_path, "del.delete-test");
        assert!(!task_dir.exists(), "Task directory should be removed");
    }

    #[test]
    fn test_delete_task_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        // Delete a task that doesn't exist
        let deleted = store.delete_task(&project_path, "nex.nonexistent").unwrap();

        assert_eq!(deleted, 0, "Should delete 0 snapshots");
    }

    #[test]
    fn test_delete_project() {
        use super::super::models::{CaptureReason, ContextSnapshotV1};

        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        // Create snapshots for 3 tasks
        for task_num in 1..=3 {
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

        // Verify we have 6 total snapshots (3 tasks × 2 snapshots)
        let mut total = 0;
        for task_num in 1..=3 {
            let task_id = format!("tk{}.task-{}", task_num, task_num);
            total += store
                .list_snapshots(&project_path, &task_id, None)
                .unwrap()
                .len();
        }
        assert_eq!(total, 6);

        // Delete the entire project
        let deleted = store.delete_project(&project_path).unwrap();

        assert_eq!(deleted, 6, "Should delete all 6 snapshots");

        // Verify all tasks have no snapshots
        for task_num in 1..=3 {
            let task_id = format!("tk{}.task-{}", task_num, task_num);
            assert_eq!(
                store
                    .list_snapshots(&project_path, &task_id, None)
                    .unwrap()
                    .len(),
                0
            );
        }

        // Verify project directory was removed
        let project_dir = store.project_dir(&project_path);
        assert!(!project_dir.exists(), "Project directory should be removed");
    }

    #[test]
    fn test_delete_project_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("nonexistent-TODO.md");

        // Delete a project that doesn't exist
        let deleted = store.delete_project(&project_path).unwrap();

        assert_eq!(deleted, 0, "Should delete 0 snapshots");
    }

    #[test]
    fn test_task_lock_coordination() {
        use super::super::models::{CaptureReason, ContextSnapshotV1};

        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path());

        let project_path = temp_dir.path().join("TODO.md");
        fs::write(&project_path, "# TODO\n").unwrap();

        // Create a snapshot (which acquires lock during write)
        let snapshot = ContextSnapshotV1::new(
            "2026-02-06T15:00:00Z_lck.lock-test".to_string(),
            project_path.to_string_lossy().to_string(),
            "lck.lock-test".to_string(),
            "Lock test task".to_string(),
            "2026-02-06T15:00:00Z".to_string(),
            CaptureReason::SessionStopped,
        );

        store
            .write_snapshot(&project_path, "lck.lock-test", &snapshot)
            .unwrap();

        // Verify lock file was created
        let lock_path = store.lock_path(&project_path, "lck.lock-test");
        assert!(lock_path.exists(), "Lock file should exist");

        // Prune (which also acquires lock) should succeed
        let deleted = store
            .prune_snapshots(&project_path, "lck.lock-test", None)
            .unwrap();

        assert_eq!(deleted, 0, "No snapshots to delete yet");

        // Delete task (which also acquires lock) should succeed
        let deleted = store.delete_task(&project_path, "lck.lock-test").unwrap();

        assert_eq!(deleted, 1, "Should delete the one snapshot");

        // Lock file should be removed with the task directory
        assert!(
            !lock_path.exists(),
            "Lock file should be removed with task directory"
        );
    }
}
