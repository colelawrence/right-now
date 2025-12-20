// todo-shim: Stable CLI shim that redirects to the installed Right Now app's todo binary.
//
// This binary should be installed to a stable location (e.g., ~/.local/bin/todo)
// and will automatically find the real todo binary inside the installed app.
// It survives app reinstalls without needing to be re-placed.

use rn_desktop_2_lib::cli_paths::{find_todo_binary, CliPaths};
use std::os::unix::process::CommandExt;
use std::process::{exit, Command};

fn main() {
    let todo_path = match locate_todo() {
        Some(path) => path,
        None => {
            eprintln!("Error: Right Now app not found.");
            eprintln!();
            print_install_help();
            exit(1);
        }
    };

    // Verify the binary exists and is executable
    if !todo_path.is_file() {
        eprintln!("Error: todo binary not found at: {}", todo_path.display());
        eprintln!();
        eprintln!("The Right Now app may have been moved or reinstalled.");
        eprintln!("Please launch the Right Now app once to update the CLI paths.");
        exit(1);
    }

    // Forward all arguments to the real todo binary
    let args: Vec<String> = std::env::args().skip(1).collect();

    // On Unix, use exec() to replace this process entirely
    #[cfg(unix)]
    {
        let err = Command::new(&todo_path).args(&args).exec();
        // exec() only returns on error
        eprintln!("Failed to execute {}: {}", todo_path.display(), err);
        exit(1);
    }

    // On Windows, spawn and wait
    #[cfg(windows)]
    {
        let status = Command::new(&todo_path)
            .args(&args)
            .status()
            .unwrap_or_else(|e| {
                eprintln!("Failed to execute {}: {}", todo_path.display(), e);
                exit(1);
            });

        exit(status.code().unwrap_or(1));
    }
}

/// Try to locate the todo binary using config file first, then fallback heuristics
fn locate_todo() -> Option<std::path::PathBuf> {
    // 1. Try reading from config file (written by the app on startup)
    if let Some(cli_paths) = CliPaths::read() {
        if cli_paths.todo_exists() {
            return Some(cli_paths.todo_path);
        }
    }

    // 2. Try fallback heuristics (known install locations)
    if let Some(path) = find_todo_binary() {
        // Update the config for faster future lookups
        let _ = update_config_from_found_binary(&path);
        return Some(path);
    }

    None
}

/// Update the config file when we find the binary via heuristics
fn update_config_from_found_binary(todo_path: &std::path::Path) -> std::io::Result<()> {
    let exe_dir = todo_path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine exe directory",
        )
    })?;

    let daemon_name = if cfg!(windows) {
        "right-now-daemon.exe"
    } else {
        "right-now-daemon"
    };

    let paths = CliPaths {
        todo_path: todo_path.to_path_buf(),
        daemon_path: exe_dir.join(daemon_name),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    paths.write()
}

fn print_install_help() {
    #[cfg(target_os = "macos")]
    {
        eprintln!("To fix this, either:");
        eprintln!("  1. Install Right Now.app to /Applications or ~/Applications");
        eprintln!("  2. Launch the Right Now app once after installation");
        eprintln!();
        eprintln!("Expected locations:");
        eprintln!("  /Applications/Right Now.app");
        eprintln!("  ~/Applications/Right Now.app");
    }

    #[cfg(target_os = "windows")]
    {
        eprintln!("To fix this, either:");
        eprintln!("  1. Install Right Now from the official installer");
        eprintln!("  2. Launch the Right Now app once after installation");
        eprintln!();
        eprintln!("Expected locations:");
        eprintln!("  %LOCALAPPDATA%\\Programs\\Right Now");
        eprintln!("  %PROGRAMFILES%\\Right Now");
    }

    #[cfg(target_os = "linux")]
    {
        eprintln!("To fix this, either:");
        eprintln!("  1. Install Right Now from your package manager or AppImage");
        eprintln!("  2. Launch the Right Now app once after installation");
        eprintln!();
        eprintln!("Expected locations:");
        eprintln!("  /usr/bin/todo");
        eprintln!("  /opt/Right Now/todo");
        eprintln!("  ~/.local/share/Right Now/todo");
    }
}
