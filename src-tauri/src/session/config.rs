// Environment configuration helpers for the daemon
// Handles platform-specific paths for sockets, PID files, and data directories

use std::path::PathBuf;

/// Configuration for daemon paths and settings
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory for storing runtime files (socket, PID)
    pub runtime_dir: PathBuf,
    /// Directory for storing persistent state (sessions.json, CR snapshots)
    pub state_dir: PathBuf,
    /// Path to the Unix socket (mac/Linux) or named pipe (Windows future)
    pub socket_path: PathBuf,
    /// Path to the daemon PID file
    pub pid_file: PathBuf,
}

impl Config {
    /// Create configuration using default paths
    pub fn default_paths() -> Self {
        let runtime_dir = Self::default_runtime_dir();
        let state_dir = Self::default_state_dir();

        Self {
            socket_path: runtime_dir.join("daemon.sock"),
            pid_file: runtime_dir.join("daemon.pid"),
            runtime_dir,
            state_dir,
        }
    }

    /// Create configuration from environment variables, falling back to defaults
    pub fn from_env() -> Self {
        // Check for override via environment variable
        // RIGHT_NOW_DAEMON_DIR overrides BOTH runtime_dir and state_dir
        if let Ok(override_dir) = std::env::var("RIGHT_NOW_DAEMON_DIR") {
            let base = PathBuf::from(override_dir);
            return Self {
                socket_path: base.join("daemon.sock"),
                pid_file: base.join("daemon.pid"),
                runtime_dir: base.clone(),
                state_dir: base,
            };
        }

        Self::default_paths()
    }

    /// Get the default runtime directory (socket + pid)
    fn default_runtime_dir() -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            // macOS: use same as state_dir
            Self::default_state_dir()
        }

        #[cfg(target_os = "linux")]
        {
            // Linux: prefer XDG_RUNTIME_DIR if set, else fall back to state_dir
            if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
                PathBuf::from(runtime_dir).join("right-now")
            } else {
                Self::default_state_dir()
            }
        }

        #[cfg(target_os = "windows")]
        {
            // Windows: use same as state_dir
            Self::default_state_dir()
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            PathBuf::from("/tmp/right-now")
        }
    }

    /// Get the default state directory (sessions.json, CR snapshots)
    fn default_state_dir() -> PathBuf {
        // All platforms: ~/.right-now/ (or /tmp/right-now if home unavailable)
        dirs::home_dir()
            .map(|h| h.join(".right-now"))
            .unwrap_or_else(|| PathBuf::from("/tmp/right-now"))
    }

    /// Get the runtime directory (socket + pid)
    pub fn runtime_dir(&self) -> &PathBuf {
        &self.runtime_dir
    }

    /// Get the state directory (sessions.json, CR snapshots)
    pub fn state_dir(&self) -> &PathBuf {
        &self.state_dir
    }

    /// Get the sessions.json file path
    pub fn sessions_file(&self) -> PathBuf {
        self.state_dir.join("sessions.json")
    }

    /// Ensure both runtime and state directories exist with appropriate permissions
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        // Create state directory (for durable data)
        std::fs::create_dir_all(&self.state_dir)?;

        // Create runtime directory with 0700 permissions on Unix
        std::fs::create_dir_all(&self.runtime_dir)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.runtime_dir, std::fs::Permissions::from_mode(0o700))?;
        }

        Ok(())
    }

    /// Path to the current project marker file
    pub fn current_project_file(&self) -> PathBuf {
        self.state_dir.join("current_project.txt")
    }

    /// Persist the current project path
    pub fn write_current_project(&self, path: &str) -> std::io::Result<()> {
        self.ensure_dirs()?;
        std::fs::write(self.current_project_file(), path)
    }

    /// Read the last recorded current project path
    pub fn read_current_project(&self) -> Option<PathBuf> {
        std::fs::read_to_string(self.current_project_file())
            .ok()
            .map(|s| PathBuf::from(s.trim()))
            .filter(|p| !p.as_os_str().is_empty() && p.exists())
    }

    /// Clear the current project marker
    pub fn clear_current_project(&self) -> std::io::Result<()> {
        let marker = self.current_project_file();
        if marker.exists() {
            std::fs::remove_file(marker)
        } else {
            Ok(())
        }
    }

    /// Get the default shell to use for sessions
    pub fn default_shell() -> Vec<String> {
        // Check RIGHT_NOW_SHELL environment variable first
        if let Ok(shell) = std::env::var("RIGHT_NOW_SHELL") {
            return vec![shell];
        }

        // Fall back to SHELL or platform defaults
        #[cfg(unix)]
        {
            if let Ok(shell) = std::env::var("SHELL") {
                return vec![shell];
            }
            vec!["/bin/zsh".to_string()]
        }

        // TODO(windows): Use PowerShell or cmd.exe as default
        #[cfg(windows)]
        {
            vec!["powershell.exe".to_string()]
        }

        #[cfg(not(any(unix, windows)))]
        {
            vec!["/bin/sh".to_string()]
        }
    }

    /// Write the daemon PID to the PID file
    pub fn write_pid(&self) -> std::io::Result<()> {
        self.ensure_dirs()?;
        std::fs::write(&self.pid_file, std::process::id().to_string())
    }

    /// Read the daemon PID from the PID file
    pub fn read_pid(&self) -> Option<u32> {
        std::fs::read_to_string(&self.pid_file)
            .ok()
            .and_then(|s| s.trim().parse().ok())
    }

    /// Remove the PID file
    pub fn remove_pid(&self) -> std::io::Result<()> {
        if self.pid_file.exists() {
            std::fs::remove_file(&self.pid_file)
        } else {
            Ok(())
        }
    }

    /// Remove the socket file
    pub fn remove_socket(&self) -> std::io::Result<()> {
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path)
        } else {
            Ok(())
        }
    }

    /// Check if the daemon socket exists (indicating daemon may be running)
    pub fn socket_exists(&self) -> bool {
        self.socket_path.exists()
    }

    /// Check if a process with the stored PID is still running
    #[cfg(unix)]
    pub fn is_daemon_running(&self) -> bool {
        if let Some(pid) = self.read_pid() {
            // Check if process exists by sending signal 0
            unsafe { libc::kill(pid as i32, 0) == 0 }
        } else {
            false
        }
    }

    // TODO(windows): Implement Windows process check
    #[cfg(not(unix))]
    pub fn is_daemon_running(&self) -> bool {
        // Conservative fallback: assume running if socket exists
        self.socket_exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_from_env() {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("RIGHT_NOW_DAEMON_DIR", temp_dir.path());

        let config = Config::from_env();
        // RIGHT_NOW_DAEMON_DIR overrides both runtime_dir and state_dir
        assert_eq!(config.runtime_dir(), temp_dir.path());
        assert_eq!(config.state_dir(), temp_dir.path());
        assert_eq!(config.socket_path, temp_dir.path().join("daemon.sock"));
        assert_eq!(config.pid_file, temp_dir.path().join("daemon.pid"));

        std::env::remove_var("RIGHT_NOW_DAEMON_DIR");
    }

    #[test]
    fn test_sessions_file_path() {
        let config = Config {
            runtime_dir: PathBuf::from("/test/runtime"),
            state_dir: PathBuf::from("/test/state"),
            socket_path: PathBuf::from("/test/runtime/daemon.sock"),
            pid_file: PathBuf::from("/test/runtime/daemon.pid"),
        };

        assert_eq!(
            config.sessions_file(),
            PathBuf::from("/test/state/sessions.json")
        );
    }

    #[test]
    fn test_pid_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            runtime_dir: temp_dir.path().to_path_buf(),
            state_dir: temp_dir.path().to_path_buf(),
            socket_path: temp_dir.path().join("daemon.sock"),
            pid_file: temp_dir.path().join("daemon.pid"),
        };

        config.write_pid().unwrap();
        let pid = config.read_pid().unwrap();
        assert_eq!(pid, std::process::id());

        config.remove_pid().unwrap();
        assert!(config.read_pid().is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_macos_paths() {
        std::env::remove_var("RIGHT_NOW_DAEMON_DIR");
        std::env::remove_var("XDG_RUNTIME_DIR");

        let config = Config::default_paths();
        // macOS: runtime_dir == state_dir == ~/.right-now
        assert_eq!(config.runtime_dir(), config.state_dir());
        assert!(config.state_dir().ends_with(".right-now"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_paths_with_xdg() {
        std::env::remove_var("RIGHT_NOW_DAEMON_DIR");
        std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000");

        let config = Config::default_paths();
        // Linux with XDG_RUNTIME_DIR: runtime_dir is XDG_RUNTIME_DIR/right-now
        assert_eq!(
            config.runtime_dir(),
            &PathBuf::from("/run/user/1000/right-now")
        );
        // state_dir is still ~/.right-now
        assert!(config.state_dir().ends_with(".right-now"));

        std::env::remove_var("XDG_RUNTIME_DIR");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_linux_paths_without_xdg() {
        std::env::remove_var("RIGHT_NOW_DAEMON_DIR");
        std::env::remove_var("XDG_RUNTIME_DIR");

        let config = Config::default_paths();
        // Linux without XDG_RUNTIME_DIR: runtime_dir falls back to state_dir
        assert_eq!(config.runtime_dir(), config.state_dir());
        assert!(config.state_dir().ends_with(".right-now"));
    }

    #[cfg(unix)]
    #[test]
    fn test_ensure_dirs_creates_runtime_dir_with_0700() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            runtime_dir: temp_dir.path().join("runtime"),
            state_dir: temp_dir.path().join("state"),
            socket_path: temp_dir.path().join("runtime/daemon.sock"),
            pid_file: temp_dir.path().join("runtime/daemon.pid"),
        };

        config.ensure_dirs().unwrap();

        // Verify runtime_dir exists with 0700 permissions
        let runtime_metadata = std::fs::metadata(&config.runtime_dir).unwrap();
        let runtime_mode = runtime_metadata.permissions().mode() & 0o777;
        assert_eq!(
            runtime_mode, 0o700,
            "runtime_dir should have 0700 permissions"
        );

        // Verify state_dir exists (permissions not restricted)
        assert!(config.state_dir.exists());
    }

    #[test]
    fn test_sessions_file_uses_state_dir() {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            runtime_dir: temp_dir.path().join("runtime"),
            state_dir: temp_dir.path().join("state"),
            socket_path: temp_dir.path().join("runtime/daemon.sock"),
            pid_file: temp_dir.path().join("runtime/daemon.pid"),
        };

        let sessions_file = config.sessions_file();
        assert!(sessions_file.starts_with(&config.state_dir));
        assert!(sessions_file.ends_with("sessions.json"));
    }

    #[test]
    fn test_socket_and_pid_use_runtime_dir() {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            runtime_dir: temp_dir.path().join("runtime"),
            state_dir: temp_dir.path().join("state"),
            socket_path: temp_dir.path().join("runtime/daemon.sock"),
            pid_file: temp_dir.path().join("runtime/daemon.pid"),
        };

        assert!(config.socket_path.starts_with(&config.runtime_dir));
        assert!(config.pid_file.starts_with(&config.runtime_dir));
    }
}
