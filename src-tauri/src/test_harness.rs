// Test harness module for E2E testing
// Provides Unix socket server for test control, fixture loading, and state management
//
// Only compiled with the `test-harness` feature flag

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
// TempDir no longer used - we manage ~/.rightnow-test/ manually
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{oneshot, Mutex};

use crate::session::protocol::{deserialize_message, serialize_message};

// ============================================================================
// Test Protocol
// ============================================================================

/// Request message from test runner to test harness
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TestRequest {
    /// Ping to check if harness is alive
    Ping,
    /// Create a new temp directory for test isolation
    CreateTempDir { label: Option<String> },
    /// Load a fixture file into a temp directory
    LoadFixture {
        /// Fixture name (e.g., "minimal" loads minimal.md)
        name: String,
        /// Temp directory path to copy into
        temp_dir: String,
    },
    /// List available fixtures
    ListFixtures,
    /// Get current app state as JSON (sent via frontend bridge)
    GetState,
    /// Reset app to initial state
    ResetState,
    /// Open a project file
    OpenProject { path: String },
    /// Complete a task by name
    CompleteTask { task_name: String },
    /// Change work state
    ChangeState { state: String },
    /// Get event history from EventBus (for testing)
    GetEventHistory,
    /// Clear event history in EventBus (for test isolation)
    ClearEventHistory,
    /// Advance the TestClock by the specified milliseconds
    AdvanceClock { ms: i64 },
    /// Set the TestClock to a specific timestamp
    SetClockTime { timestamp: i64 },
    /// Get the current TestClock time
    GetClockTime,
    /// Cleanup a temp directory
    CleanupTempDir { path: String },
    /// Cleanup all temp directories
    CleanupAll,
    /// Request harness to shut down
    Shutdown,
}

/// Response message from test harness to test runner
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TestResponse {
    /// Pong response
    Pong,
    /// Success with optional data
    Ok {
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
    },
    /// Temp directory created
    TempDirCreated { path: String },
    /// Fixture loaded
    FixtureLoaded { path: String },
    /// List of available fixtures
    FixtureList { fixtures: Vec<String> },
    /// Current app state
    State { state: serde_json::Value },
    /// Error response
    Error { message: String },
    /// Shutdown acknowledged
    ShuttingDown,
}

// ============================================================================
// Temp Directory Management
// ============================================================================

/// Base directory for test temp files: ~/rightnow-test/
/// This is used instead of system temp to avoid Tauri fs:scope issues
/// Note: We don't use a leading dot because Tauri's glob matching may not handle hidden dirs
fn test_base_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not find home directory")?;
    Ok(home.join("rightnow-test"))
}

/// Manages temp directories for test isolation
/// Uses ~/rightnow-test/ instead of system temp for Tauri fs compatibility
pub struct TempDirManager {
    /// Active temp directories (paths we've created)
    dirs: HashMap<String, PathBuf>,
    /// Counter for generating unique names
    counter: u64,
}

impl TempDirManager {
    pub fn new() -> Self {
        Self {
            dirs: HashMap::new(),
            counter: 0,
        }
    }

    /// Create a new temp directory and return its path
    pub fn create(&mut self, label: Option<String>) -> Result<String> {
        let base = test_base_dir()?;

        // Ensure base directory exists
        std::fs::create_dir_all(&base)
            .context("Failed to create test base directory ~/rightnow-test")?;

        // Generate unique subdirectory name
        self.counter += 1;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let dir_name = format!("test-{}-{}", timestamp, self.counter);

        let dir_path = base.join(&dir_name);
        std::fs::create_dir_all(&dir_path)
            .with_context(|| format!("Failed to create temp directory: {:?}", dir_path))?;

        let path_str = dir_path.to_string_lossy().to_string();

        // Use label as key if provided, otherwise use path
        let key = label.unwrap_or_else(|| path_str.clone());
        self.dirs.insert(key, dir_path);

        Ok(path_str)
    }

    /// Cleanup a specific temp directory
    pub fn cleanup(&mut self, path: &str) -> Result<()> {
        // Find and remove the temp dir with matching path
        let key_to_remove: Option<String> = self
            .dirs
            .iter()
            .find(|(_, dir)| dir.to_string_lossy() == path)
            .map(|(k, _)| k.clone());

        if let Some(key) = key_to_remove {
            if let Some(dir_path) = self.dirs.remove(&key) {
                // Actually delete the directory
                if dir_path.exists() {
                    std::fs::remove_dir_all(&dir_path).with_context(|| {
                        format!("Failed to remove temp directory: {:?}", dir_path)
                    })?;
                }
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("Temp directory not found: {}", path))
        }
    }

    /// Cleanup all temp directories
    pub fn cleanup_all(&mut self) {
        for (_, dir_path) in self.dirs.drain() {
            if dir_path.exists() {
                let _ = std::fs::remove_dir_all(&dir_path);
            }
        }
    }
}

impl Default for TempDirManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Fixture Management
// ============================================================================

/// Path to fixtures directory (relative to the Tauri app)
fn fixtures_dir() -> PathBuf {
    // In development, fixtures are in src-tauri/test-fixtures/
    // We'll try multiple locations
    let candidates = [
        PathBuf::from("test-fixtures"),
        PathBuf::from("src-tauri/test-fixtures"),
        PathBuf::from("../test-fixtures"),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return candidate;
        }
    }

    // Default to test-fixtures
    PathBuf::from("test-fixtures")
}

/// List available fixtures
pub fn list_fixtures() -> Vec<String> {
    let fixtures_path = fixtures_dir();

    if !fixtures_path.exists() {
        return Vec::new();
    }

    std::fs::read_dir(&fixtures_path)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let path = e.path();
                    if path.extension().map_or(false, |ext| ext == "md") {
                        path.file_stem().map(|s| s.to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Load a fixture into a temp directory
pub fn load_fixture(name: &str, temp_dir: &str) -> Result<String> {
    let fixtures_path = fixtures_dir();
    let fixture_file = fixtures_path.join(format!("{}.md", name));

    if !fixture_file.exists() {
        return Err(anyhow::anyhow!(
            "Fixture not found: {} (looked in {:?})",
            name,
            fixture_file
        ));
    }

    let content = std::fs::read_to_string(&fixture_file).context("Failed to read fixture file")?;

    let dest_path = PathBuf::from(temp_dir).join(format!("{}.md", name));
    std::fs::write(&dest_path, content).context("Failed to write fixture to temp directory")?;

    Ok(dest_path.to_string_lossy().to_string())
}

// ============================================================================
// Test Harness State
// ============================================================================

/// Counter for generating unique request IDs
static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generate a unique request ID
fn next_request_id() -> String {
    REQUEST_ID_COUNTER
        .fetch_add(1, Ordering::SeqCst)
        .to_string()
}

/// Event payload sent to frontend
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum TestCommand {
    GetState {
        request_id: String,
    },
    ResetState {
        request_id: String,
    },
    OpenProject {
        request_id: String,
        path: String,
    },
    CompleteTask {
        request_id: String,
        task_name: String,
    },
    ChangeState {
        request_id: String,
        state: String,
    },
    GetEventHistory {
        request_id: String,
    },
    ClearEventHistory {
        request_id: String,
    },
    AdvanceClock {
        request_id: String,
        ms: i64,
    },
    SetClockTime {
        request_id: String,
        timestamp: i64,
    },
    GetClockTime {
        request_id: String,
    },
}

/// Shared state for the test harness
pub struct TestHarnessState {
    temp_dirs: Mutex<TempDirManager>,
    /// App handle for emitting events (set after app starts)
    app_handle: Mutex<Option<AppHandle>>,
    /// Pending requests waiting for frontend responses
    pending_requests: Mutex<HashMap<String, oneshot::Sender<serde_json::Value>>>,
}

impl TestHarnessState {
    pub fn new() -> Self {
        Self {
            temp_dirs: Mutex::new(TempDirManager::new()),
            app_handle: Mutex::new(None),
            pending_requests: Mutex::new(HashMap::new()),
        }
    }

    /// Set the app handle for event emission
    pub async fn set_app_handle(&self, handle: AppHandle) {
        *self.app_handle.lock().await = Some(handle);
    }

    /// Send a command to the frontend and wait for response
    pub async fn send_and_wait(
        &self,
        command: TestCommand,
        timeout: Duration,
    ) -> Result<serde_json::Value> {
        let request_id = match &command {
            TestCommand::GetState { request_id } => request_id.clone(),
            TestCommand::ResetState { request_id } => request_id.clone(),
            TestCommand::OpenProject { request_id, .. } => request_id.clone(),
            TestCommand::CompleteTask { request_id, .. } => request_id.clone(),
            TestCommand::ChangeState { request_id, .. } => request_id.clone(),
            TestCommand::GetEventHistory { request_id } => request_id.clone(),
            TestCommand::ClearEventHistory { request_id } => request_id.clone(),
            TestCommand::AdvanceClock { request_id, .. } => request_id.clone(),
            TestCommand::SetClockTime { request_id, .. } => request_id.clone(),
            TestCommand::GetClockTime { request_id } => request_id.clone(),
        };

        // Create oneshot channel for response
        let (tx, rx) = oneshot::channel();

        // Register pending request
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(request_id.clone(), tx);
        }

        // Emit event to frontend
        {
            let handle = self.app_handle.lock().await;
            if let Some(handle) = handle.as_ref() {
                handle
                    .emit("test:command", &command)
                    .context("Failed to emit test command")?;
            } else {
                // Clean up pending request
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&request_id);
                return Err(anyhow::anyhow!("App handle not set"));
            }
        }

        // Wait for response with timeout
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => {
                // Channel closed without response
                Err(anyhow::anyhow!("Frontend response channel closed"))
            }
            Err(_) => {
                // Timeout - clean up pending request
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&request_id);
                Err(anyhow::anyhow!("Timeout waiting for frontend response"))
            }
        }
    }

    /// Called by frontend to respond to a command
    pub async fn complete_request(
        &self,
        request_id: String,
        data: serde_json::Value,
    ) -> Result<()> {
        let mut pending = self.pending_requests.lock().await;
        if let Some(tx) = pending.remove(&request_id) {
            let _ = tx.send(data); // Ignore error if receiver dropped
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "No pending request with ID: {}",
                request_id
            ))
        }
    }
}

impl Default for TestHarnessState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Unix Socket Server
// ============================================================================

/// Default socket path for test harness
pub fn default_socket_path() -> PathBuf {
    std::env::temp_dir().join("rightnow-test-harness.sock")
}

/// Handle a single client connection
async fn handle_client(
    stream: UnixStream,
    state: Arc<TestHarnessState>,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;

        if bytes_read == 0 {
            // Connection closed
            break;
        }

        let request: TestRequest = match deserialize_message(line.as_bytes()) {
            Ok(req) => req,
            Err(e) => {
                let response = TestResponse::Error {
                    message: format!("Failed to parse request: {}", e),
                };
                let bytes = serialize_message(&response)?;
                writer.write_all(&bytes).await?;
                continue;
            }
        };

        let response = match request {
            TestRequest::Ping => TestResponse::Pong,

            TestRequest::CreateTempDir { ref label } => {
                let mut temp_dirs = state.temp_dirs.lock().await;
                match temp_dirs.create(label.clone()) {
                    Ok(path) => TestResponse::TempDirCreated { path },
                    Err(e) => TestResponse::Error {
                        message: e.to_string(),
                    },
                }
            }

            TestRequest::LoadFixture {
                ref name,
                ref temp_dir,
            } => match load_fixture(name, temp_dir) {
                Ok(path) => TestResponse::FixtureLoaded { path },
                Err(e) => TestResponse::Error {
                    message: e.to_string(),
                },
            },

            TestRequest::ListFixtures => {
                let fixtures = list_fixtures();
                TestResponse::FixtureList { fixtures }
            }

            TestRequest::GetState => {
                let cmd = TestCommand::GetState {
                    request_id: next_request_id(),
                };
                match state.send_and_wait(cmd, Duration::from_secs(5)).await {
                    Ok(state_value) => TestResponse::State { state: state_value },
                    Err(e) => TestResponse::Error {
                        message: e.to_string(),
                    },
                }
            }

            TestRequest::ResetState => {
                let cmd = TestCommand::ResetState {
                    request_id: next_request_id(),
                };
                match state.send_and_wait(cmd, Duration::from_secs(5)).await {
                    Ok(_) => TestResponse::Ok { data: None },
                    Err(e) => TestResponse::Error {
                        message: e.to_string(),
                    },
                }
            }

            TestRequest::OpenProject { ref path } => {
                let cmd = TestCommand::OpenProject {
                    request_id: next_request_id(),
                    path: path.clone(),
                };
                match state.send_and_wait(cmd, Duration::from_secs(10)).await {
                    Ok(data) => TestResponse::Ok {
                        data: if data.is_null() { None } else { Some(data) },
                    },
                    Err(e) => TestResponse::Error {
                        message: e.to_string(),
                    },
                }
            }

            TestRequest::CompleteTask { ref task_name } => {
                let cmd = TestCommand::CompleteTask {
                    request_id: next_request_id(),
                    task_name: task_name.clone(),
                };
                match state.send_and_wait(cmd, Duration::from_secs(5)).await {
                    Ok(_) => TestResponse::Ok { data: None },
                    Err(e) => TestResponse::Error {
                        message: e.to_string(),
                    },
                }
            }

            TestRequest::ChangeState {
                state: ref work_state,
            } => {
                let cmd = TestCommand::ChangeState {
                    request_id: next_request_id(),
                    state: work_state.clone(),
                };
                match state.send_and_wait(cmd, Duration::from_secs(5)).await {
                    Ok(_) => TestResponse::Ok { data: None },
                    Err(e) => TestResponse::Error {
                        message: e.to_string(),
                    },
                }
            }

            TestRequest::GetEventHistory => {
                let cmd = TestCommand::GetEventHistory {
                    request_id: next_request_id(),
                };
                match state.send_and_wait(cmd, Duration::from_secs(5)).await {
                    Ok(history) => TestResponse::Ok {
                        data: Some(history),
                    },
                    Err(e) => TestResponse::Error {
                        message: e.to_string(),
                    },
                }
            }

            TestRequest::ClearEventHistory => {
                let cmd = TestCommand::ClearEventHistory {
                    request_id: next_request_id(),
                };
                match state.send_and_wait(cmd, Duration::from_secs(5)).await {
                    Ok(_) => TestResponse::Ok { data: None },
                    Err(e) => TestResponse::Error {
                        message: e.to_string(),
                    },
                }
            }

            TestRequest::AdvanceClock { ms } => {
                let cmd = TestCommand::AdvanceClock {
                    request_id: next_request_id(),
                    ms,
                };
                match state.send_and_wait(cmd, Duration::from_secs(5)).await {
                    Ok(_) => TestResponse::Ok { data: None },
                    Err(e) => TestResponse::Error {
                        message: e.to_string(),
                    },
                }
            }

            TestRequest::SetClockTime { timestamp } => {
                let cmd = TestCommand::SetClockTime {
                    request_id: next_request_id(),
                    timestamp,
                };
                match state.send_and_wait(cmd, Duration::from_secs(5)).await {
                    Ok(_) => TestResponse::Ok { data: None },
                    Err(e) => TestResponse::Error {
                        message: e.to_string(),
                    },
                }
            }

            TestRequest::GetClockTime => {
                let cmd = TestCommand::GetClockTime {
                    request_id: next_request_id(),
                };
                match state.send_and_wait(cmd, Duration::from_secs(5)).await {
                    Ok(time) => TestResponse::Ok { data: Some(time) },
                    Err(e) => TestResponse::Error {
                        message: e.to_string(),
                    },
                }
            }

            TestRequest::CleanupTempDir { ref path } => {
                let mut temp_dirs = state.temp_dirs.lock().await;
                match temp_dirs.cleanup(path) {
                    Ok(()) => TestResponse::Ok { data: None },
                    Err(e) => TestResponse::Error {
                        message: e.to_string(),
                    },
                }
            }

            TestRequest::CleanupAll => {
                let mut temp_dirs = state.temp_dirs.lock().await;
                temp_dirs.cleanup_all();
                TestResponse::Ok { data: None }
            }

            TestRequest::Shutdown => {
                let _ = shutdown_tx.send(());
                TestResponse::ShuttingDown
            }
        };

        let bytes = serialize_message(&response)?;
        writer.write_all(&bytes).await?;

        // If shutdown was requested, break after sending response
        if matches!(request, TestRequest::Shutdown) {
            break;
        }
    }

    Ok(())
}

/// Start the test harness Unix socket server
pub async fn start_server(
    socket_path: PathBuf,
    state: Arc<TestHarnessState>,
) -> Result<tokio::sync::broadcast::Receiver<()>> {
    // Remove existing socket if present
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    let listener = UnixListener::bind(&socket_path).context("Failed to bind Unix socket")?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);

    eprintln!("Test harness server listening on {:?}", socket_path);

    // Spawn the server task
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        let mut shutdown_rx = shutdown_tx_clone.subscribe();

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, _)) => {
                            let state = Arc::clone(&state);
                            let shutdown_tx = shutdown_tx.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_client(stream, state, shutdown_tx).await {
                                    eprintln!("Client error: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            eprintln!("Accept error: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    eprintln!("Test harness server shutting down");
                    break;
                }
            }
        }

        // Cleanup socket file
        let _ = std::fs::remove_file(&socket_path);
    });

    Ok(shutdown_rx)
}

// ============================================================================
// Tauri Commands (exposed to frontend)
// ============================================================================

/// Tauri command: Create a temp directory
#[tauri::command]
pub async fn test_create_temp_dir(
    state: tauri::State<'_, Arc<TestHarnessState>>,
    label: Option<String>,
) -> Result<String, String> {
    let mut temp_dirs = state.temp_dirs.lock().await;
    temp_dirs.create(label).map_err(|e| e.to_string())
}

/// Tauri command: Load a fixture into a temp directory
#[tauri::command]
pub async fn test_load_fixture(name: String, temp_dir: String) -> Result<String, String> {
    load_fixture(&name, &temp_dir).map_err(|e| e.to_string())
}

/// Tauri command: List available fixtures
#[tauri::command]
pub async fn test_list_fixtures() -> Vec<String> {
    list_fixtures()
}

/// Tauri command: Cleanup all temp directories
#[tauri::command]
pub async fn test_cleanup_all(
    state: tauri::State<'_, Arc<TestHarnessState>>,
) -> Result<(), String> {
    let mut temp_dirs = state.temp_dirs.lock().await;
    temp_dirs.cleanup_all();
    Ok(())
}

/// Tauri command: Get the socket path for external test runners
#[tauri::command]
pub fn test_get_socket_path() -> String {
    default_socket_path().to_string_lossy().to_string()
}

/// Tauri command: Frontend responds to a test command
#[tauri::command]
pub async fn test_respond(
    state: tauri::State<'_, Arc<TestHarnessState>>,
    request_id: String,
    data: serde_json::Value,
) -> Result<(), String> {
    state
        .complete_request(request_id, data)
        .await
        .map_err(|e| e.to_string())
}

/// Tauri command: Set the app handle for event emission (called on startup)
#[tauri::command]
pub async fn test_set_app_handle(
    app: AppHandle,
    state: tauri::State<'_, Arc<TestHarnessState>>,
) -> Result<(), String> {
    state.set_app_handle(app).await;
    Ok(())
}
