// Environment configuration helpers for the daemon
// Handles platform-specific paths for sockets, PID files, and data directories

use std::path::PathBuf;

/// Configuration for daemon paths and settings
#[derive(Debug, Clone)]
pub struct Config {
    /// Directory for storing daemon data (sessions.json, etc.)
    pub data_dir: PathBuf,
    /// Path to the Unix socket (mac/Linux) or named pipe (Windows future)
    pub socket_path: PathBuf,
    /// Path to the daemon PID file
    pub pid_file: PathBuf,
}

impl Config {
    /// Create configuration using default paths
    pub fn default_paths() -> Self {
        let base_dir = Self::base_dir();

        Self {
            data_dir: base_dir.clone(),
            socket_path: base_dir.join("daemon.sock"),
            pid_file: base_dir.join("daemon.pid"),
        }
    }

    /// Create configuration from environment variables, falling back to defaults
    pub fn from_env() -> Self {
        // Check for override via environment variable
        if let Ok(override_dir) = std::env::var("RIGHT_NOW_DAEMON_DIR") {
            let base = PathBuf::from(override_dir);
            return Self {
                data_dir: base.clone(),
                socket_path: base.join("daemon.sock"),
                pid_file: base.join("daemon.pid"),
            };
        }

        Self::default_paths()
    }

    /// Get the base directory for daemon files
    fn base_dir() -> PathBuf {
        // Platform-specific base directories
        #[cfg(target_os = "macos")]
        {
            // macOS: ~/.right-now/
            dirs::home_dir()
                .map(|h| h.join(".right-now"))
                .unwrap_or_else(|| PathBuf::from("/tmp/right-now"))
        }

        #[cfg(target_os = "linux")]
        {
            // Linux: ~/.right-now/ (or XDG_RUNTIME_DIR if available)
            if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
                PathBuf::from(runtime_dir).join("right-now")
            } else {
                dirs::home_dir()
                    .map(|h| h.join(".right-now"))
                    .unwrap_or_else(|| PathBuf::from("/tmp/right-now"))
            }
        }

        // TODO(windows): Use %APPDATA%\Right Now\ for Windows
        #[cfg(target_os = "windows")]
        {
            dirs::data_dir()
                .map(|d| d.join("Right Now"))
                .unwrap_or_else(|| PathBuf::from("C:\\ProgramData\\Right Now"))
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            PathBuf::from("/tmp/right-now")
        }
    }

    /// Get the sessions.json file path
    pub fn sessions_file(&self) -> PathBuf {
        self.data_dir.join("sessions.json")
    }

    /// Ensure the data directory exists
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)
    }

    /// Path to the current project marker file
    pub fn current_project_file(&self) -> PathBuf {
        self.data_dir.join("current_project.txt")
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
        assert_eq!(config.data_dir, temp_dir.path());
        assert_eq!(config.socket_path, temp_dir.path().join("daemon.sock"));
        assert_eq!(config.pid_file, temp_dir.path().join("daemon.pid"));

        std::env::remove_var("RIGHT_NOW_DAEMON_DIR");
    }

    #[test]
    fn test_sessions_file_path() {
        let config = Config {
            data_dir: PathBuf::from("/test/dir"),
            socket_path: PathBuf::from("/test/dir/daemon.sock"),
            pid_file: PathBuf::from("/test/dir/daemon.pid"),
        };

        assert_eq!(
            config.sessions_file(),
            PathBuf::from("/test/dir/sessions.json")
        );
    }

    #[test]
    fn test_pid_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            data_dir: temp_dir.path().to_path_buf(),
            socket_path: temp_dir.path().join("daemon.sock"),
            pid_file: temp_dir.path().join("daemon.pid"),
        };

        config.write_pid().unwrap();
        let pid = config.read_pid().unwrap();
        assert_eq!(pid, std::process::id());

        config.remove_pid().unwrap();
        assert!(config.read_pid().is_none());
    }
}
