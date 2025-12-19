use super::polling::{wait_for_file_content, WaitError};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use libc;

const READY_TIMEOUT: Duration = Duration::from_secs(5);
const INITIAL_DELAY_MS: u64 = 50;
const MAX_DELAY_MS: u64 = 1_000;

/// RAII wrapper that ensures the daemon process is cleaned up.
pub struct DaemonGuard {
    child: Child,
    data_dir: PathBuf,
}

impl DaemonGuard {
    /// Start the daemon using the compiled binary.
    pub fn start(data_dir: &Path) -> Result<Self, DaemonError> {
        let daemon_bin = find_daemon_binary().ok_or(DaemonError::BinaryNotFound)?;

        let child = Command::new(&daemon_bin)
            .env("RIGHT_NOW_DAEMON_DIR", data_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(DaemonError::SpawnFailed)?;

        let guard = Self {
            child,
            data_dir: data_dir.to_path_buf(),
        };

        println!(
            "Started right-now-daemon (pid {}) in {}",
            guard.pid(),
            data_dir.display()
        );

        guard.wait_for_ready().map_err(DaemonError::ReadyTimeout)?;

        Ok(guard)
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    fn wait_for_ready(&self) -> Result<(), WaitError> {
        let pid_file = self.data_dir.join("daemon.pid");
        wait_for_file_content(
            &pid_file,
            |content| content.trim().parse::<u32>().is_ok(),
            READY_TIMEOUT,
        )
        .map(|_| ())
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        if let Ok(Some(_)) = self.child.try_wait() {
            return;
        }

        if let Err(err) = self.child.kill() {
            eprintln!(
                "Failed to terminate right-now-daemon pid {}: {}",
                self.child.id(),
                err
            );
            return;
        }

        let _ = self.child.wait();
    }
}

/// Start the daemon and return a guard that will clean it up on drop.
pub fn start_daemon(data_dir: &Path) -> Result<DaemonGuard, DaemonError> {
    DaemonGuard::start(data_dir)
}

/// Wait for a process to exit, used by tests to ensure no orphans remain.
pub fn wait_for_process_exit(pid: u32, timeout: Duration) -> bool {
    let start = Instant::now();
    let mut delay = Duration::from_millis(INITIAL_DELAY_MS);

    while process_is_running(pid) && start.elapsed() < timeout {
        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            break;
        }
        thread::sleep(delay.min(remaining));
        delay = delay
            .checked_mul(2)
            .unwrap_or_else(|| Duration::from_millis(MAX_DELAY_MS))
            .min(Duration::from_millis(MAX_DELAY_MS));
    }

    !process_is_running(pid)
}

pub fn is_process_running(pid: u32) -> bool {
    process_is_running(pid)
}

#[cfg(unix)]
fn process_is_running(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(not(unix))]
fn process_is_running(_pid: u32) -> bool {
    false
}

fn find_daemon_binary() -> Option<PathBuf> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    let debug_path = PathBuf::from(manifest_dir).join("../target/debug/right-now-daemon");
    if debug_path.exists() {
        return Some(debug_path);
    }

    let release_path = PathBuf::from(manifest_dir).join("../target/release/right-now-daemon");
    if release_path.exists() {
        return Some(release_path);
    }

    let alt_path = PathBuf::from(manifest_dir).join("target/debug/right-now-daemon");
    if alt_path.exists() {
        return Some(alt_path);
    }

    None
}

/// Errors that can occur when starting the daemon for tests.
#[derive(Debug)]
pub enum DaemonError {
    BinaryNotFound,
    SpawnFailed(std::io::Error),
    ReadyTimeout(WaitError),
}

impl DaemonError {
    pub fn is_missing_binary(&self) -> bool {
        matches!(self, Self::BinaryNotFound)
    }
}

impl fmt::Display for DaemonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BinaryNotFound => write!(
                f,
                "right-now-daemon binary not found. Run `cargo build` before executing integration tests."
            ),
            Self::SpawnFailed(err) => write!(f, "failed to spawn daemon: {}", err),
            Self::ReadyTimeout(err) => write!(f, "daemon never became ready: {}", err),
        }
    }
}

impl std::error::Error for DaemonError {}
