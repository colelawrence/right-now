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
///
/// For CR requests, transport failures (connect/timeout) return Ok(DaemonResponse::Error)
/// to enable structured error handling in the TypeScript layer.
pub fn send_request(request: DaemonRequest) -> Result<DaemonResponse> {
    use super::protocol::DaemonErrorCode;

    let config = Config::from_env();

    // Check if this is a CR request (needs structured error handling)
    let is_cr_request = matches!(
        request,
        DaemonRequest::CrLatest { .. }
            | DaemonRequest::CrList { .. }
            | DaemonRequest::CrGet { .. }
            | DaemonRequest::CrCaptureNow { .. }
            | DaemonRequest::CrDeleteTask { .. }
            | DaemonRequest::CrDeleteProject { .. }
    );

    // Connect to daemon (or start it)
    let mut stream = match connect_or_start_daemon(&config) {
        Ok(s) => s,
        Err(e) if is_cr_request => {
            // For CR requests, return structured error instead of propagating
            return Ok(DaemonResponse::Error {
                code: DaemonErrorCode::DaemonUnavailable,
                message: format!("Failed to connect to daemon: {}", e),
            });
        }
        Err(e) => return Err(e),
    };

    // Set read timeout to avoid hanging forever
    if let Err(e) = stream.set_read_timeout(Some(Duration::from_secs(10))) {
        if is_cr_request {
            return Ok(DaemonResponse::Error {
                code: DaemonErrorCode::Internal,
                message: format!("Failed to set read timeout: {}", e),
            });
        }
        return Err(e.into());
    }

    // Perform protocol handshake first
    {
        use super::protocol::PROTOCOL_VERSION;

        let handshake = DaemonRequest::Handshake {
            client_version: PROTOCOL_VERSION,
        };
        let handshake_bytes = match serialize_message(&handshake) {
            Ok(b) => b,
            Err(e) if is_cr_request => {
                return Ok(DaemonResponse::Error {
                    code: DaemonErrorCode::Internal,
                    message: format!("Failed to serialize handshake: {}", e),
                });
            }
            Err(e) => return Err(e.into()),
        };

        if let Err(e) = stream.write_all(&handshake_bytes) {
            if is_cr_request {
                return Ok(DaemonResponse::Error {
                    code: DaemonErrorCode::DaemonUnavailable,
                    message: format!("Failed to send handshake: {}", e),
                });
            }
            return Err(e.into());
        }

        if let Err(e) = stream.flush() {
            if is_cr_request {
                return Ok(DaemonResponse::Error {
                    code: DaemonErrorCode::DaemonUnavailable,
                    message: format!("Failed to flush handshake: {}", e),
                });
            }
            return Err(e.into());
        }

        // Read handshake response
        let handshake_response = match read_response(&mut stream, is_cr_request) {
            Ok(r) => r,
            Err(e) => {
                if is_cr_request {
                    return Ok(DaemonResponse::Error {
                        code: DaemonErrorCode::DaemonUnavailable,
                        message: format!("Handshake failed: {}", e),
                    });
                }
                return Err(e);
            }
        };

        // Check for version mismatch
        match handshake_response {
            DaemonResponse::Handshake {
                protocol_version: _,
            } => {
                // Handshake successful
            }
            DaemonResponse::Error { code, message } if code == DaemonErrorCode::VersionMismatch => {
                if is_cr_request {
                    return Ok(DaemonResponse::Error { code, message });
                }
                return Err(anyhow::anyhow!("Protocol version mismatch: {}", message));
            }
            other => {
                let msg = format!("Expected handshake response, got: {:?}", other);
                if is_cr_request {
                    return Ok(DaemonResponse::Error {
                        code: DaemonErrorCode::Internal,
                        message: msg,
                    });
                }
                return Err(anyhow::anyhow!(msg));
            }
        }
    }

    // Send the actual request
    let request_bytes = match serialize_message(&request) {
        Ok(b) => b,
        Err(e) if is_cr_request => {
            return Ok(DaemonResponse::Error {
                code: DaemonErrorCode::InvalidRequest,
                message: format!("Failed to serialize request: {}", e),
            });
        }
        Err(e) => return Err(e.into()),
    };

    if let Err(e) = stream.write_all(&request_bytes) {
        if is_cr_request {
            return Ok(DaemonResponse::Error {
                code: DaemonErrorCode::DaemonUnavailable,
                message: format!("Failed to send request to daemon: {}", e),
            });
        }
        return Err(e.into());
    }

    if let Err(e) = stream.flush() {
        if is_cr_request {
            return Ok(DaemonResponse::Error {
                code: DaemonErrorCode::DaemonUnavailable,
                message: format!("Failed to flush stream: {}", e),
            });
        }
        return Err(e.into());
    }

    // Read response
    match read_response(&mut stream, is_cr_request) {
        Ok(r) => Ok(r),
        Err(e) if is_cr_request => Ok(DaemonResponse::Error {
            code: DaemonErrorCode::DaemonUnavailable,
            message: format!("Failed to read response: {}", e),
        }),
        Err(e) => Err(e),
    }
}

/// Helper to read a response from the daemon, enforcing frame size limits
fn read_response(stream: &mut UnixStream, is_cr_request: bool) -> Result<DaemonResponse> {
    use super::protocol::{DaemonErrorCode, MAX_RESPONSE_FRAME_SIZE};

    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                if is_cr_request {
                    return Ok(DaemonResponse::Error {
                        code: DaemonErrorCode::DaemonUnavailable,
                        message: "Daemon closed connection unexpectedly".to_string(),
                    });
                }
                return Err(anyhow::anyhow!("Daemon closed connection unexpectedly"));
            }
            Ok(_) => {
                // Enforce max response frame size (10MB)
                if line.len() > MAX_RESPONSE_FRAME_SIZE {
                    if is_cr_request {
                        return Ok(DaemonResponse::Error {
                            code: DaemonErrorCode::Internal,
                            message: format!(
                                "Response frame too large: {} bytes (max {})",
                                line.len(),
                                MAX_RESPONSE_FRAME_SIZE
                            ),
                        });
                    }
                    return Err(anyhow::anyhow!(
                        "Response frame too large: {} bytes (max {})",
                        line.len(),
                        MAX_RESPONSE_FRAME_SIZE
                    ));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                if is_cr_request {
                    return Ok(DaemonResponse::Error {
                        code: DaemonErrorCode::Timeout,
                        message: "Daemon read timeout".to_string(),
                    });
                }
                return Err(e.into());
            }
            Err(e) if is_cr_request => {
                return Ok(DaemonResponse::Error {
                    code: DaemonErrorCode::DaemonUnavailable,
                    message: format!("Failed to read response from daemon: {}", e),
                });
            }
            Err(e) => return Err(e.into()),
        }

        // Try to parse as notification first (we ignore these for now)
        if let Ok(_notification) = deserialize_message::<DaemonNotification>(line.as_bytes()) {
            eprintln!("Received daemon notification (ignored for now)");
            continue;
        }

        // Try to parse as response
        match deserialize_message::<DaemonResponse>(line.as_bytes()) {
            Ok(response) => return Ok(response),
            Err(e) if is_cr_request => {
                return Ok(DaemonResponse::Error {
                    code: DaemonErrorCode::Internal,
                    message: format!("Failed to parse daemon response: {}", e),
                });
            }
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
    if let DaemonResponse::Error { code: _, message } = response {
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
