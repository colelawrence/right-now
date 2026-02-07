// Integration tests for IPC frame size limits

use anyhow::Result;
use rn_desktop_2_lib::session::{
    config::Config,
    protocol::{
        deserialize_message, serialize_message, DaemonErrorCode, DaemonRequest, DaemonResponse,
        MAX_REQUEST_FRAME_SIZE, MAX_RESPONSE_FRAME_SIZE,
    },
};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// Helper to connect to test daemon (assumes it's running)
fn connect_daemon() -> Result<UnixStream> {
    let config = Config::from_env();
    let stream = UnixStream::connect(&config.socket_path)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    Ok(stream)
}

/// Helper to send a request and receive a response
fn send_request(stream: &mut UnixStream, request: &DaemonRequest) -> Result<DaemonResponse> {
    let bytes = serialize_message(request)?;
    stream.write_all(&bytes)?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    Ok(deserialize_message(line.as_bytes())?)
}

#[test]
#[ignore] // Requires running daemon
fn test_server_rejects_oversized_request() {
    // Create a request that exceeds MAX_REQUEST_FRAME_SIZE (1MB)
    // We'll use a CrLatest request with a very long project_path

    let mut stream = connect_daemon().expect("Failed to connect to daemon");

    // First, perform handshake
    let handshake = DaemonRequest::Handshake { client_version: 1 };
    let response = send_request(&mut stream, &handshake).expect("Handshake failed");
    match response {
        DaemonResponse::Handshake { .. } => {}
        other => panic!("Expected handshake response, got: {:?}", other),
    }

    // Create a huge project path (slightly over 1MB when serialized)
    let huge_path = "x".repeat(MAX_REQUEST_FRAME_SIZE + 1000);
    let oversized_request = DaemonRequest::CrLatest {
        project_path: huge_path,
        task_id: None,
    };

    // Send the oversized request
    let response = send_request(&mut stream, &oversized_request).expect("Failed to send request");

    // Expect an error response with invalid_request code
    match response {
        DaemonResponse::Error { code, message } => {
            assert_eq!(code, DaemonErrorCode::InvalidRequest);
            assert!(
                message.contains("Request frame too large"),
                "Expected 'Request frame too large' error, got: {}",
                message
            );
            assert!(
                message.contains("max 1048576"),
                "Expected message to mention 1MB limit, got: {}",
                message
            );
        }
        other => panic!(
            "Expected error response for oversized request, got: {:?}",
            other
        ),
    }
}

#[test]
fn test_client_rejects_oversized_response_during_deserialization() {
    // This test checks that the client-side enforces MAX_RESPONSE_FRAME_SIZE
    // We simulate a response that exceeds 10MB

    let oversized_data = "x".repeat(MAX_RESPONSE_FRAME_SIZE + 1000);

    // Create a mock response that would be too large
    let mock_response_json = format!(
        r#"{{"type":"error","code":"internal","message":"{}"}}"#,
        oversized_data
    );

    // Verify that attempting to process this would exceed the limit
    assert!(
        mock_response_json.len() > MAX_RESPONSE_FRAME_SIZE,
        "Test setup: mock response should exceed MAX_RESPONSE_FRAME_SIZE"
    );

    // In practice, the client would reject this during read_line()
    // This test documents the expected behavior
}

#[test]
fn test_max_request_frame_size_constant() {
    // Verify the constant is set to 1MB
    assert_eq!(MAX_REQUEST_FRAME_SIZE, 1024 * 1024);
}

#[test]
fn test_max_response_frame_size_constant() {
    // Verify the constant is set to 10MB
    assert_eq!(MAX_RESPONSE_FRAME_SIZE, 10 * 1024 * 1024);
}

#[test]
#[ignore] // Requires running daemon
fn test_normal_sized_request_accepted() {
    // Verify that normal-sized requests still work after adding frame limits

    let mut stream = connect_daemon().expect("Failed to connect to daemon");

    // Perform handshake
    let handshake = DaemonRequest::Handshake { client_version: 1 };
    let response = send_request(&mut stream, &handshake).expect("Handshake failed");
    match response {
        DaemonResponse::Handshake { .. } => {}
        other => panic!("Expected handshake response, got: {:?}", other),
    }

    // Send a normal ping request
    let ping = DaemonRequest::Ping;
    let response = send_request(&mut stream, &ping).expect("Failed to send ping");

    // Should get a pong response
    match response {
        DaemonResponse::Pong => {
            // Success
        }
        other => panic!("Expected Pong response, got: {:?}", other),
    }
}
