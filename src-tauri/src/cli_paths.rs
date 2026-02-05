// CLI Paths Configuration
// Shared module for locating CLI binaries across app, shim, and daemon.
// The main app writes cli-paths.json on startup; the shim reads it to find the real binaries.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Paths to CLI binaries, written by the app and read by the shim
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliPaths {
    /// Path to the `todo` CLI binary
    pub todo_path: PathBuf,
    /// Path to the `right-now-daemon` binary
    pub daemon_path: PathBuf,
    /// When this config was last updated (ISO 8601)
    pub updated_at: String,
}

impl CliPaths {
    /// Get the platform-specific config directory for Right Now
    pub fn config_dir() -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            dirs::home_dir().map(|h| h.join("Library/Application Support/Right Now"))
        }

        #[cfg(target_os = "windows")]
        {
            dirs::config_dir().map(|d| d.join("Right Now"))
        }

        #[cfg(target_os = "linux")]
        {
            dirs::config_dir().map(|d| d.join("right-now"))
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            dirs::home_dir().map(|h| h.join(".right-now"))
        }
    }

    /// Get the path to cli-paths.json
    pub fn config_file() -> Option<PathBuf> {
        Self::config_dir().map(|d| d.join("cli-paths.json"))
    }

    /// Read the CLI paths from the config file
    pub fn read() -> Option<Self> {
        let path = Self::config_file()?;
        let data = std::fs::read(&path).ok()?;
        serde_json::from_slice(&data).ok()
    }

    /// Write the CLI paths to the config file
    pub fn write(&self) -> std::io::Result<()> {
        let dir = Self::config_dir().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine config directory",
            )
        })?;
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("cli-paths.json");
        let data = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, data)
    }

    /// Create CliPaths from the current executable location.
    /// Call this from the main app on startup.
    pub fn from_current_exe() -> std::io::Result<Self> {
        let exe = std::env::current_exe()?;
        let exe_dir = exe.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine exe directory",
            )
        })?;

        let (todo_name, daemon_name) = if cfg!(windows) {
            ("todo.exe", "right-now-daemon.exe")
        } else {
            ("todo", "right-now-daemon")
        };

        Ok(Self {
            todo_path: exe_dir.join(todo_name),
            daemon_path: exe_dir.join(daemon_name),
            updated_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Write CLI paths based on current exe location.
    /// Call this from the main app on startup.
    pub fn write_from_current_exe() -> std::io::Result<()> {
        let paths = Self::from_current_exe()?;
        paths.write()
    }

    /// Check if the todo binary exists at the configured path
    pub fn todo_exists(&self) -> bool {
        self.todo_path.is_file()
    }

    /// Check if the daemon binary exists at the configured path
    pub fn daemon_exists(&self) -> bool {
        self.daemon_path.is_file()
    }
}

// ============================================================================
// Shim Configuration - tracks user's preferred CLI name
// ============================================================================

/// Default CLI name if none is configured
pub const DEFAULT_CLI_NAME: &str = "todo";

/// Configuration for the installed CLI shim
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShimConfig {
    /// The CLI command name (e.g., "todo", "td", "tasks")
    /// On Windows, ".exe" is appended automatically
    pub cli_name: Option<String>,
    /// When this config was last updated (ISO 8601)
    pub updated_at: Option<String>,
}

impl ShimConfig {
    /// Get the path to shim-config.json
    pub fn config_file() -> Option<PathBuf> {
        CliPaths::config_dir().map(|d| d.join("shim-config.json"))
    }

    /// Read the shim config from disk, returning defaults if not found
    pub fn read() -> Self {
        let Some(path) = Self::config_file() else {
            return Self::default();
        };
        let Ok(data) = std::fs::read(&path) else {
            return Self::default();
        };
        serde_json::from_slice(&data).unwrap_or_default()
    }

    /// Write the shim config to disk
    pub fn write(&self) -> std::io::Result<()> {
        let dir = CliPaths::config_dir().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine config directory",
            )
        })?;
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("shim-config.json");
        let data = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, data)
    }

    /// Get the configured CLI name, or the default
    pub fn cli_name(&self) -> &str {
        self.cli_name
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| s.as_str())
            .unwrap_or(DEFAULT_CLI_NAME)
    }
}

/// Get the current configured CLI name (or default "todo")
pub fn current_cli_name() -> String {
    ShimConfig::read().cli_name().to_string()
}

/// Validate a CLI name.
/// Rules:
/// - 1-32 characters
/// - Only ASCII letters, digits, '-', '_'
/// - Not "." or ".."
pub fn validate_cli_name(name: &str) -> Result<(), String> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err("CLI name cannot be empty".into());
    }
    if trimmed.len() > 32 {
        return Err("CLI name is too long (max 32 characters)".into());
    }
    if trimmed == "." || trimmed == ".." {
        return Err("CLI name cannot be '.' or '..'".into());
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("CLI name may only contain letters, numbers, '-' and '_'".into());
    }

    Ok(())
}

// ============================================================================
// Shim Installation
// ============================================================================

/// Get the platform-specific install directory for the shim
pub fn shim_install_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        dirs::home_dir().map(|h| h.join(".local/bin"))
    }

    #[cfg(windows)]
    {
        dirs::home_dir().map(|h| h.join("bin"))
    }
}

/// Get the install path for a specific CLI name
pub fn shim_install_path_for(name: &str) -> Option<PathBuf> {
    let dir = shim_install_dir()?;

    #[cfg(windows)]
    {
        let fname = if name.to_lowercase().ends_with(".exe") {
            name.to_string()
        } else {
            format!("{}.exe", name)
        };
        Some(dir.join(fname))
    }

    #[cfg(unix)]
    {
        Some(dir.join(name))
    }
}

/// Get the path where the shim should be installed (using current configured name)
pub fn shim_install_path() -> Option<PathBuf> {
    let name = current_cli_name();
    shim_install_path_for(&name)
}

/// Status of the CLI shim installation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShimStatus {
    /// Shim is installed and working
    Installed { path: PathBuf, name: String },
    /// Shim is not installed
    NotInstalled { name: String },
    /// Shim install directory doesn't exist
    DirectoryMissing,
}

impl ShimStatus {
    /// Check the current installation status of the shim
    pub fn check() -> Self {
        let name = current_cli_name();
        let Some(install_path) = shim_install_path() else {
            return ShimStatus::DirectoryMissing;
        };

        if install_path.is_file() {
            ShimStatus::Installed {
                path: install_path,
                name,
            }
        } else if let Some(dir) = shim_install_dir() {
            if dir.exists() {
                ShimStatus::NotInstalled { name }
            } else {
                ShimStatus::DirectoryMissing
            }
        } else {
            ShimStatus::DirectoryMissing
        }
    }

    /// Check if the shim is installed
    pub fn is_installed(&self) -> bool {
        matches!(self, ShimStatus::Installed { .. })
    }

    /// Get the CLI name (configured or default)
    pub fn cli_name(&self) -> &str {
        match self {
            ShimStatus::Installed { name, .. } => name,
            ShimStatus::NotInstalled { name } => name,
            ShimStatus::DirectoryMissing => DEFAULT_CLI_NAME,
        }
    }

    /// Get a human-readable description
    pub fn description(&self) -> String {
        match self {
            ShimStatus::Installed { path, name } => {
                format!("'{}' installed at {}", name, path.display())
            }
            ShimStatus::NotInstalled { name } => format!("'{}' not installed", name),
            ShimStatus::DirectoryMissing => "Install directory missing".to_string(),
        }
    }
}

/// Install the CLI shim with a specific name.
/// If a different name was previously installed, the old binary is removed.
pub fn install_shim_as(new_name: &str) -> std::io::Result<PathBuf> {
    validate_cli_name(new_name)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    let exe = std::env::current_exe()?;
    let exe_dir = exe.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine exe directory",
        )
    })?;

    // Find the todo-shim binary in our app bundle
    let shim_source_name = if cfg!(windows) {
        "todo-shim.exe"
    } else {
        "todo-shim"
    };
    let shim_source = exe_dir.join(shim_source_name);

    if !shim_source.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Shim binary not found at: {}", shim_source.display()),
        ));
    }

    // Get the install directory
    let install_dir = shim_install_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine install directory",
        )
    })?;
    std::fs::create_dir_all(&install_dir)?;

    // Get old and new paths
    let old_config = ShimConfig::read();
    let old_name = old_config.cli_name();
    let old_path = shim_install_path_for(old_name);

    let new_path = shim_install_path_for(new_name).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine install path",
        )
    })?;

    // Copy shim to new path
    std::fs::copy(&shim_source, &new_path)?;

    // Make it executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&new_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&new_path, perms)?;
    }

    // Remove old binary if it exists and is different from the new one
    if let Some(old_path) = old_path {
        if old_path != new_path && old_path.is_file() {
            let _ = std::fs::remove_file(&old_path);
        }
    }

    // Save the new name to config
    let new_config = ShimConfig {
        cli_name: Some(new_name.to_string()),
        updated_at: Some(chrono::Utc::now().to_rfc3339()),
    };
    // Don't fail install if we can't write config
    let _ = new_config.write();

    Ok(new_path)
}

/// Install the CLI shim using the current configured name
pub fn install_shim() -> std::io::Result<PathBuf> {
    let name = current_cli_name();
    install_shim_as(&name)
}

/// Uninstall the CLI shim (for the current configured name)
pub fn uninstall_shim() -> std::io::Result<()> {
    let install_path = shim_install_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine install path",
        )
    })?;

    if install_path.is_file() {
        std::fs::remove_file(&install_path)?;
    }

    Ok(())
}

/// Set the preferred CLI name without installing
pub fn set_cli_name(name: &str) -> std::io::Result<()> {
    validate_cli_name(name)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    let config = ShimConfig {
        cli_name: Some(name.to_string()),
        updated_at: Some(chrono::Utc::now().to_rfc3339()),
    };
    config.write()
}

// ============================================================================
// Fallback binary discovery (used by the shim)
// ============================================================================

/// Platform-specific fallback locations to search for the app
pub fn fallback_app_locations() -> Vec<PathBuf> {
    let mut locations = Vec::new();

    #[cfg(target_os = "macos")]
    {
        locations.push(PathBuf::from("/Applications/Right Now.app/Contents/MacOS"));
        if let Some(home) = dirs::home_dir() {
            locations.push(home.join("Applications/Right Now.app/Contents/MacOS"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = dirs::data_local_dir() {
            locations.push(local_app_data.join("Programs/Right Now"));
        }
        if let Ok(program_files) = std::env::var("PROGRAMFILES") {
            locations.push(PathBuf::from(program_files).join("Right Now"));
        }
        if let Ok(program_files_x86) = std::env::var("PROGRAMFILES(X86)") {
            locations.push(PathBuf::from(program_files_x86).join("Right Now"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        locations.push(PathBuf::from("/usr/bin"));
        locations.push(PathBuf::from("/usr/local/bin"));
        locations.push(PathBuf::from("/opt/Right Now"));
        if let Some(home) = dirs::home_dir() {
            locations.push(home.join(".local/share/Right Now"));
        }
    }

    locations
}

/// Try to find the todo binary using fallback heuristics
pub fn find_todo_binary() -> Option<PathBuf> {
    let binary_name = if cfg!(windows) { "todo.exe" } else { "todo" };

    for dir in fallback_app_locations() {
        let candidate = dir.join(binary_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

/// Try to find the daemon binary using fallback heuristics
pub fn find_daemon_binary() -> Option<PathBuf> {
    let binary_name = if cfg!(windows) {
        "right-now-daemon.exe"
    } else {
        "right-now-daemon"
    };

    for dir in fallback_app_locations() {
        let candidate = dir.join(binary_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

/// Resolve the `right-now-daemon` binary path for the current process.
///
/// Used by both the desktop app and the `todo` CLI.
/// Resolution order:
/// 1) Next to `current_exe()` (bundled app / cargo target dir)
/// 2) cli-paths.json written by the app
/// 3) Platform fallback locations (/Applications, etc)
pub fn resolve_daemon_path() -> Option<PathBuf> {
    let daemon_name = if cfg!(windows) {
        "right-now-daemon.exe"
    } else {
        "right-now-daemon"
    };

    // 1) Next to current executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(daemon_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // 2) From app-written config
    if let Some(paths) = CliPaths::read() {
        if paths.daemon_exists() {
            return Some(paths.daemon_path);
        }
    }

    // 3) Fallback locations
    find_daemon_binary()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_dir_is_some() {
        assert!(CliPaths::config_dir().is_some());
    }

    #[test]
    fn test_config_file_is_some() {
        assert!(CliPaths::config_file().is_some());
    }

    #[test]
    fn test_fallback_locations_not_empty() {
        let locations = fallback_app_locations();
        assert!(!locations.is_empty());
    }

    #[test]
    fn test_shim_install_dir_is_some() {
        assert!(shim_install_dir().is_some());
    }

    #[test]
    fn test_shim_status_check() {
        let status = ShimStatus::check();
        let _ = status.description();
        let _ = status.cli_name();
    }

    #[test]
    fn test_validate_cli_name() {
        assert!(validate_cli_name("todo").is_ok());
        assert!(validate_cli_name("td").is_ok());
        assert!(validate_cli_name("my-cli").is_ok());
        assert!(validate_cli_name("my_cli").is_ok());
        assert!(validate_cli_name("CLI123").is_ok());

        assert!(validate_cli_name("").is_err());
        assert!(validate_cli_name(".").is_err());
        assert!(validate_cli_name("..").is_err());
        assert!(validate_cli_name("has space").is_err());
        assert!(validate_cli_name("has/slash").is_err());
        assert!(validate_cli_name("a".repeat(33).as_str()).is_err());
    }

    #[test]
    fn test_shim_config_default() {
        let config = ShimConfig::default();
        assert_eq!(config.cli_name(), DEFAULT_CLI_NAME);
    }

    #[test]
    fn test_shim_install_path_for() {
        let path = shim_install_path_for("myapp");
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.to_string_lossy().contains("myapp"));
    }
}
