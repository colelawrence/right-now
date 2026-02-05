// Daemon client module for Tauri commands
// Handles communication with the right-now-daemon over Unix socket

use super::config::Config;
use super::protocol::{
    deserialize_message, serialize_message, DaemonNotification, DaemonRequest, DaemonResponse,
};
use crate::cli_paths::resolve_daemon_path;
use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Connect to the daemon, starting it if necessary
fn connect_or_start_daemon(config: &Config) -> Result<UnixStream> {
    // Try to connect first
    if let Ok(stream) = UnixStream::connect(&config.socket_path) {
        return Ok(stream);
    }

    // Daemon not running, try to start it
    eprintln!("Daemon not running, attempting to start...");

    // Find the daemon binary using several strategies:
    // 1. Next to current_exe() (typical for bundled releases)
    // 2. CliPaths from app-written config
    // 3. Platform-specific fallback locations
    let daemon_path = resolve_daemon_path().ok_or_else(|| {
        anyhow::anyhow!(
            "Could not find right-now-daemon binary. Please ensure Right Now is installed correctly."
        )
    })?;

    // Start the daemon as a detached background process
    Command::new(&daemon_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("Failed to start daemon at {}", daemon_path.display()))?;

    // Wait for the socket to appear (up to 2 seconds)
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(100));
        if config.socket_path.exists() {
            // Try to connect
            if let Ok(stream) = UnixStream::connect(&config.socket_path) {
                eprintln!("Daemon started successfully");
                return Ok(stream);
            }
        }
    }

    Err(anyhow::anyhow!(
        "Daemon did not start within 2 seconds (socket not found at: {})",
        config.socket_path.display()
    ))
}

/// Send a request to the daemon and receive a response
/// Handles notification messages that may arrive before the response
pub fn send_request(request: DaemonRequest) -> Result<DaemonResponse> {
    let config = Config::from_env();

    // Connect to daemon (or start it)
    let mut stream = connect_or_start_daemon(&config)?;

    // Set read timeout to avoid hanging forever
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .context("Failed to set read timeout")?;

    // Send the request
    let request_bytes = serialize_message(&request).context("Failed to serialize request")?;
    stream
        .write_all(&request_bytes)
        .context("Failed to send request to daemon")?;
    stream.flush().context("Failed to flush stream")?;

    // Read response, skipping any notification messages
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        reader
            .read_line(&mut line)
            .context("Failed to read response from daemon")?;

        if line.is_empty() {
            return Err(anyhow::anyhow!("Daemon closed connection unexpectedly"));
        }

        // Try to parse as notification first (we ignore these for now)
        if let Ok(_notification) = deserialize_message::<DaemonNotification>(line.as_bytes()) {
            eprintln!("Received daemon notification (ignored for now)");
            continue;
        }

        // Try to parse as response
        match deserialize_message::<DaemonResponse>(line.as_bytes()) {
            Ok(response) => return Ok(response),
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Failed to parse daemon response: {} (line: {})",
                    e,
                    line.trim()
                ));
            }
        }
    }
}

/// Helper to convert DaemonResponse to user-facing error messages
pub fn response_to_result<T, F>(response: DaemonResponse, extract: F) -> Result<T, String>
where
    F: FnOnce(DaemonResponse) -> Option<T>,
{
    if let DaemonResponse::Error { message } = response {
        return Err(message);
    }

    extract(response).ok_or_else(|| "Unexpected response from daemon".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_ping() {
        // This test requires a running daemon
        // Skip if daemon is not running
        let config = Config::from_env();
        if !config.socket_exists() {
            eprintln!("Skipping test: daemon not running");
            return;
        }

        let response = send_request(DaemonRequest::Ping);
        match response {
            Ok(DaemonResponse::Pong) => {}
            Ok(other) => panic!("Expected Pong, got {:?}", other),
            Err(e) => panic!("Failed to ping daemon: {}", e),
        }
    }
}
