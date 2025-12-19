// Persistence helpers for session registry
// Sessions are persisted to $APPDATA/right-now/sessions.json with file locking

use crate::session::config::Config;
use crate::session::protocol::{Session, SessionId};
use anyhow::{Context, Result};
use fs2::FileExt;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

/// Session registry persisted to disk
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SessionRegistry {
    /// Next session ID to assign
    pub next_id: SessionId,
    /// Map of session ID to session data
    pub sessions: HashMap<SessionId, Session>,
}

impl SessionRegistry {
    /// Load the session registry from disk, creating an empty one if it doesn't exist
    pub fn load(config: &Config) -> Result<Self> {
        let path = config.sessions_file();

        if !path.exists() {
            return Ok(Self::default());
        }

        let mut file = File::open(&path)
            .with_context(|| format!("Failed to open sessions file: {}", path.display()))?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .with_context(|| format!("Failed to read sessions file: {}", path.display()))?;

        if contents.trim().is_empty() {
            return Ok(Self::default());
        }

        serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse sessions file: {}", path.display()))
    }

    /// Save the session registry to disk with exclusive file locking
    pub fn save(&self, config: &Config) -> Result<()> {
        let path = config.sessions_file();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create sessions directory: {}", parent.display())
            })?;
        }

        // Open file for writing with exclusive lock
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .with_context(|| {
                format!(
                    "Failed to open sessions file for writing: {}",
                    path.display()
                )
            })?;

        // Acquire exclusive lock (blocking)
        // TODO(windows): Use CreateFile locking on Windows
        file.lock_exclusive()
            .with_context(|| "Failed to acquire exclusive lock on sessions file")?;

        // Write JSON
        let contents =
            serde_json::to_string_pretty(self).with_context(|| "Failed to serialize sessions")?;

        file.write_all(contents.as_bytes())
            .with_context(|| "Failed to write sessions file")?;

        // Lock is automatically released when file is dropped
        Ok(())
    }

    /// Allocate a new session ID
    pub fn allocate_id(&mut self) -> SessionId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Insert a session into the registry
    pub fn insert(&mut self, session: Session) {
        self.sessions.insert(session.id, session);
    }

    /// Get a session by ID
    pub fn get(&self, id: SessionId) -> Option<&Session> {
        self.sessions.get(&id)
    }

    /// Get a mutable reference to a session by ID
    pub fn get_mut(&mut self, id: SessionId) -> Option<&mut Session> {
        self.sessions.get_mut(&id)
    }

    /// Remove a session by ID
    pub fn remove(&mut self, id: SessionId) -> Option<Session> {
        self.sessions.remove(&id)
    }

    /// Get all sessions for a specific project path
    pub fn sessions_for_project(&self, project_path: &str) -> Vec<&Session> {
        self.sessions
            .values()
            .filter(|s| s.project_path == project_path)
            .collect()
    }

    /// Get all sessions
    pub fn all_sessions(&self) -> Vec<&Session> {
        self.sessions.values().collect()
    }

    /// Find a session by task key in a specific project
    ///
    /// Uses exact case-insensitive matching to avoid treating similarly-named
    /// tasks (e.g., "Build feature" and "Build feature - backend") as duplicates.
    pub fn find_by_task_key(&self, task_key: &str, project_path: &str) -> Option<&Session> {
        self.sessions.values().find(|s| {
            s.project_path == project_path && s.task_key.to_lowercase() == task_key.to_lowercase()
        })
    }
}

/// Atomically save data to a file using write-to-temp + rename
/// This ensures the UI watcher sees a single change event
pub fn atomic_write(path: &PathBuf, contents: &str) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("Invalid path: {}", path.display()))?;

    // Create temp file in same directory to ensure same filesystem for rename
    let temp_path = parent.join(format!(
        ".{}.tmp.{}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown"),
        std::process::id()
    ));

    // Write to temp file
    fs::write(&temp_path, contents)
        .with_context(|| format!("Failed to write temp file: {}", temp_path.display()))?;

    // Atomic rename
    fs::rename(&temp_path, path).with_context(|| {
        format!(
            "Failed to rename {} to {}",
            temp_path.display(),
            path.display()
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::protocol::SessionStatus;
    use tempfile::TempDir;

    fn test_config() -> (Config, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            data_dir: temp_dir.path().to_path_buf(),
            socket_path: temp_dir.path().join("daemon.sock"),
            pid_file: temp_dir.path().join("daemon.pid"),
        };
        (config, temp_dir)
    }

    #[test]
    fn test_registry_roundtrip() {
        let (config, _temp) = test_config();
        let mut registry = SessionRegistry::default();

        let id = registry.allocate_id();
        let mut session = Session::new(id, "Test task".to_string(), "/test/TODO.md".to_string());
        session.status = SessionStatus::Running;
        registry.insert(session);

        registry.save(&config).unwrap();

        let loaded = SessionRegistry::load(&config).unwrap();
        assert_eq!(loaded.next_id, 1);
        assert_eq!(loaded.sessions.len(), 1);

        let loaded_session = loaded.get(id).unwrap();
        assert_eq!(loaded_session.task_key, "Test task");
        assert_eq!(loaded_session.status, SessionStatus::Running);
    }

    #[test]
    fn test_find_by_task_key() {
        let mut registry = SessionRegistry::default();

        let id1 = registry.allocate_id();
        registry.insert(Session::new(
            id1,
            "Implement reports".to_string(),
            "/test/TODO.md".to_string(),
        ));

        let id2 = registry.allocate_id();
        registry.insert(Session::new(
            id2,
            "Build pipeline".to_string(),
            "/test/TODO.md".to_string(),
        ));

        // Should match exact task key (case-insensitively)
        let found = registry.find_by_task_key("Implement reports", "/test/TODO.md");
        assert!(found.is_some());
        assert_eq!(found.unwrap().task_key, "Implement reports");

        // Should match case-insensitively
        let found = registry.find_by_task_key("BUILD PIPELINE", "/test/TODO.md");
        assert!(found.is_some());
        assert_eq!(found.unwrap().task_key, "Build pipeline");

        // Should NOT match by prefix (exact match only)
        let found = registry.find_by_task_key("Build", "/test/TODO.md");
        assert!(found.is_none(), "Prefix should not match");

        // Should not match different project
        let found = registry.find_by_task_key("Implement reports", "/other/TODO.md");
        assert!(found.is_none());
    }

    #[test]
    fn test_find_by_task_key_allows_similar_names() {
        let mut registry = SessionRegistry::default();

        // Two tasks with similar names
        let id1 = registry.allocate_id();
        registry.insert(Session::new(
            id1,
            "Build feature".to_string(),
            "/test/TODO.md".to_string(),
        ));

        let id2 = registry.allocate_id();
        registry.insert(Session::new(
            id2,
            "Build feature - backend".to_string(),
            "/test/TODO.md".to_string(),
        ));

        // Each should be found independently
        let found = registry.find_by_task_key("Build feature", "/test/TODO.md");
        assert!(found.is_some());
        assert_eq!(found.unwrap().task_key, "Build feature");

        let found = registry.find_by_task_key("Build feature - backend", "/test/TODO.md");
        assert!(found.is_some());
        assert_eq!(found.unwrap().task_key, "Build feature - backend");
    }

    #[test]
    fn test_atomic_write() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.md");

        atomic_write(&path, "# Test\n- [ ] Task\n").unwrap();

        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents, "# Test\n- [ ] Task\n");
    }
}
