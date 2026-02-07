// right-now-daemon: Background daemon for managing TODO terminal sessions
//
// Responsibilities:
// - Own session registry and persist to sessions.json
// - Spawn PTYs via portable-pty for shell sessions
// - Update TODO Markdown files atomically when sessions change state
// - Expose Unix socket protocol for CLI/UI communication
// - Broadcast session updates to subscribed clients

use anyhow::{Context, Result};
use rn_desktop_2_lib::{
    context_resurrection::{
        capture::{CaptureService, SessionProvider, SessionSnapshot},
        models::CaptureReason,
        store::SnapshotStore,
    },
    session::{
        attention,
        config::Config,
        markdown::{
            find_task_by_key, parse_body, update_task_session_in_content, TaskSessionStatus,
        },
        notify::{notify_attention, NotificationDebouncer},
        persistence::{atomic_write, SessionRegistry},
        protocol::{
            deserialize_message, serialize_message, AttentionSummary, DaemonNotification,
            DaemonRequest, DaemonResponse, Session, SessionId, SessionStatus,
        },
        runtime::{PtyEvent, PtyRuntime},
    },
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::task::JoinHandle;

const DEFAULT_TAIL_BYTES: usize = 4 * 1024;

/// Idle timeout threshold for context captures (10 minutes)
const IDLE_CAPTURE_THRESHOLD_SECS: u64 = 10 * 60;

/// Daemon state shared across all client connections
struct DaemonState {
    config: Config,
    registry: RwLock<SessionRegistry>,
    /// Broadcast channel for session updates
    updates_tx: broadcast::Sender<DaemonNotification>,
    /// Active PTY sessions (session_id -> PTY handle)
    pty_handles: Mutex<HashMap<SessionId, PtyRuntime>>,
    /// Completed session tails retained after PTY exit
    completed_tails: Mutex<HashMap<SessionId, Vec<u8>>>,
    /// Active attach socket listeners
    attach_listeners: Mutex<HashMap<SessionId, AttachSocketHandle>>,
    /// Per-session notification debouncers (5s cooldown)
    notification_debouncers: Mutex<HashMap<SessionId, NotificationDebouncer>>,
    /// Context Resurrection capture service (optional - graceful degradation if unavailable)
    capture_service: Mutex<Option<CaptureService>>,
    /// Context Resurrection snapshot store (separate from service for query ops)
    snapshot_store: SnapshotStore,
}

struct AttachSocketHandle {
    path: PathBuf,
    task: JoinHandle<()>,
}

impl DaemonState {
    fn new(config: Config) -> Result<Self> {
        // Load existing registry or create empty one
        let registry = SessionRegistry::load(&config)?;

        // Create broadcast channel for updates
        let (updates_tx, _) = broadcast::channel(100);

        // Initialize snapshot store for CR queries
        let snapshot_store = SnapshotStore::new(config.state_dir());

        Ok(Self {
            config,
            registry: RwLock::new(registry),
            updates_tx,
            pty_handles: Mutex::new(HashMap::new()),
            completed_tails: Mutex::new(HashMap::new()),
            attach_listeners: Mutex::new(HashMap::new()),
            notification_debouncers: Mutex::new(HashMap::new()),
            capture_service: Mutex::new(None), // Initialized after Arc::new in main()
            snapshot_store,
        })
    }

    /// Initialize Context Resurrection capture service (called after Arc::new)
    async fn init_capture_service(self: &Arc<Self>) {
        let snapshot_store = SnapshotStore::new(self.config.state_dir());
        if snapshot_store.is_available() {
            let session_provider: Arc<dyn SessionProvider> =
                Arc::clone(self) as Arc<dyn SessionProvider>;
            let capture_service = CaptureService::new(snapshot_store, Some(session_provider));
            *self.capture_service.lock().await = Some(capture_service);
            eprintln!("Context Resurrection capture service initialized");
        } else {
            eprintln!(
                "Warning: Context Resurrection snapshot store unavailable - captures disabled"
            );
        }
    }

    /// Trigger a context capture if capture service is available
    ///
    /// Skips capture silently if:
    /// - Capture service unavailable
    /// - task_id is None (no stable identifier)
    /// - Capture dedup/rate-limit kicks in
    ///
    /// Note: Uses tokio::task::spawn_blocking to avoid blocking async context
    fn trigger_capture(
        self: &Arc<Self>,
        project_path: String,
        task_id: Option<String>,
        task_title: String,
        session_id: u64,
        reason: CaptureReason,
    ) {
        // Skip if no task_id (per requirements: "if None, skip capture")
        let task_id = match task_id {
            Some(id) => id,
            None => return,
        };

        // Spawn blocking task to avoid blocking async runtime
        let state = Arc::clone(self);
        tokio::task::spawn_blocking(move || {
            // Use blocking_lock since we're in a spawn_blocking context
            if let Some(ref service) = *state.capture_service.blocking_lock() {
                match service.capture_now(
                    std::path::Path::new(&project_path),
                    &task_id,
                    &task_title,
                    Some(session_id),
                    reason,
                    None,
                ) {
                    Ok(Some(_snapshot)) => {
                        // Capture succeeded (logged by CaptureService)
                    }
                    Ok(None) => {
                        // Capture skipped (dedup/rate-limit)
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: Context capture failed for task {}: {}",
                            task_id, e
                        );
                    }
                }
            }
        });
    }

    /// Reconcile persisted sessions on daemon startup
    ///
    /// After a crash/restart, the registry may list Running/Waiting sessions
    /// but no PTY handles exist. Mark them all as Stopped and update their
    /// markdown badges.
    async fn reconcile_stale_sessions(&self) {
        let sessions_to_stop: Vec<(SessionId, String, String)> = {
            let mut registry = self.registry.write().await;
            let mut to_stop = Vec::new();

            for session in registry.sessions.values_mut() {
                if session.status == SessionStatus::Running
                    || session.status == SessionStatus::Waiting
                {
                    eprintln!(
                        "Reconciling stale session {} '{}' (was {:?})",
                        session.id, session.task_key, session.status
                    );
                    to_stop.push((
                        session.id,
                        session.project_path.clone(),
                        session.task_key.clone(),
                    ));
                    session.status = SessionStatus::Stopped;
                    session.updated_at = chrono::Utc::now();
                }
            }

            to_stop
        };

        if sessions_to_stop.is_empty() {
            return;
        }

        // Save registry with updated statuses
        if let Err(e) = self.save_registry().await {
            eprintln!("Failed to save reconciled sessions: {}", e);
        }

        // Update markdown files for each stale session
        for (session_id, project_path, task_key) in sessions_to_stop {
            let project_file = PathBuf::from(&project_path);
            if let Ok(content) = tokio::fs::read_to_string(&project_file).await {
                let session_status = TaskSessionStatus {
                    status: SessionStatus::Stopped,
                    session_id,
                };
                let result =
                    update_task_session_in_content(&content, &task_key, Some(&session_status));
                // If task not found (deleted/renamed), just skip - user cleaned it up
                if result.task_found {
                    if let Err(e) = atomic_write(&project_file, &result.content) {
                        eprintln!(
                            "Failed to update markdown for stale session {}: {}",
                            session_id, e
                        );
                    }
                } else {
                    eprintln!(
                        "Task '{}' no longer exists in '{}', skipping badge update",
                        task_key, project_path
                    );
                }
            }
        }
    }

    /// Save the registry to disk
    async fn save_registry(&self) -> Result<()> {
        let registry = self.registry.read().await;
        registry.save(&self.config)
    }

    /// Broadcast a notification to all subscribed clients
    fn broadcast(&self, notification: DaemonNotification) {
        // Ignore send errors (no subscribers)
        let _ = self.updates_tx.send(notification);
    }

    /// Fetch a tail for a session, whether running or completed
    async fn session_tail(&self, session_id: SessionId, max_bytes: usize) -> Option<Vec<u8>> {
        // First check running PTY handles
        {
            let handles = self.pty_handles.lock().await;
            if let Some(runtime) = handles.get(&session_id) {
                let data = runtime.get_recent_output_blocking(max_bytes);
                return Some(data);
            }
        }

        // Fall back to stored completed tails
        let tails = self.completed_tails.lock().await;
        tails.get(&session_id).map(|data| {
            if data.len() <= max_bytes {
                data.clone()
            } else {
                data[data.len() - max_bytes..].to_vec()
            }
        })
    }

    /// Store the last known tail for a completed session
    async fn store_completed_tail(&self, session_id: SessionId, data: Vec<u8>) {
        let mut tails = self.completed_tails.lock().await;
        tails.insert(session_id, data);
    }

    async fn clear_completed_tail(&self, session_id: SessionId) {
        let mut tails = self.completed_tails.lock().await;
        tails.remove(&session_id);
    }

    fn attach_socket_path(&self, session_id: SessionId) -> PathBuf {
        self.config
            .runtime_dir()
            .join(format!("attach-{}.sock", session_id))
    }

    async fn prepare_attach_socket(self: &Arc<Self>, session_id: SessionId) -> Result<PathBuf> {
        let path = self.attach_socket_path(session_id);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let listener = UnixListener::bind(&path)
            .with_context(|| format!("Failed to bind attach socket {}", path.display()))?;

        let mut listeners = self.attach_listeners.lock().await;
        if let Some(handle) = listeners.remove(&session_id) {
            handle.task.abort();
            let _ = std::fs::remove_file(handle.path);
        }

        let state = Arc::clone(self);
        let join_handle = tokio::spawn(async move {
            if let Err(err) = DaemonState::run_attach_listener(state, session_id, listener).await {
                eprintln!("Attach listener error for session {}: {}", session_id, err);
            }
        });

        listeners.insert(
            session_id,
            AttachSocketHandle {
                path: path.clone(),
                task: join_handle,
            },
        );

        Ok(path)
    }

    async fn run_attach_listener(
        state: Arc<DaemonState>,
        session_id: SessionId,
        listener: UnixListener,
    ) -> Result<()> {
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let state_for_conn = Arc::clone(&state);
                    tokio::spawn(async move {
                        if let Err(err) = DaemonState::stream_attach_connection(
                            state_for_conn,
                            session_id,
                            stream,
                        )
                        .await
                        {
                            eprintln!("Attach stream error for session {}: {}", session_id, err);
                        }
                    });
                }
                Err(e) => {
                    // If the listener was closed during shutdown, just exit.
                    return Err(e.into());
                }
            }
        }
    }

    async fn stream_attach_connection(
        state: Arc<DaemonState>,
        session_id: SessionId,
        stream: UnixStream,
    ) -> Result<()> {
        let (input_tx, mut events) = {
            let handles = state.pty_handles.lock().await;
            match handles.get(&session_id) {
                Some(runtime) => (runtime.input_sender(), runtime.subscribe_events()),
                None => {
                    anyhow::bail!("Session {} has no active PTY to attach", session_id);
                }
            }
        };

        let (mut socket_reader, mut socket_writer) = stream.into_split();

        let input_task = tokio::spawn({
            let input_tx = input_tx.clone();
            async move {
                let mut buf = [0u8; 4096];
                loop {
                    match socket_reader.read(&mut buf).await {
                        Ok(0) => break,
                        Ok(n) => {
                            if input_tx.send(buf[..n].to_vec()).await.is_err() {
                                break;
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
            }
        });

        let output_task = tokio::spawn(async move {
            loop {
                match events.recv().await {
                    Ok(PtyEvent::Output(data)) => {
                        if socket_writer.write_all(&data).await.is_err() {
                            break;
                        }
                    }
                    Ok(PtyEvent::Exited { exit_code }) => {
                        let msg = match exit_code {
                            Some(code) => format!("\r\n[process exited with code {}]\r\n", code),
                            None => "\r\n[process exited]\r\n".to_string(),
                        };
                        let _ = socket_writer.write_all(msg.as_bytes()).await;
                        let _ = socket_writer.flush().await;
                        break;
                    }
                    Ok(PtyEvent::Idle) | Ok(PtyEvent::Active) => {
                        // TODO: forward attention/idle signals to clients in a future phase.
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
        });

        let _ = tokio::join!(input_task, output_task);

        Ok(())
    }

    async fn remove_attach_socket(&self, session_id: SessionId) {
        let mut listeners = self.attach_listeners.lock().await;
        if let Some(handle) = listeners.remove(&session_id) {
            handle.task.abort();
            let _ = std::fs::remove_file(handle.path);
        }
    }

    fn spawn_attention_monitor(self: &Arc<Self>, session_id: SessionId) {
        let state = Arc::clone(self);
        tokio::spawn(async move {
            let mut events = {
                let handles = state.pty_handles.lock().await;
                match handles.get(&session_id) {
                    Some(runtime) => runtime.subscribe_events(),
                    None => return,
                }
            };
            let mut last_preview: Option<String> = None;
            let mut accumulator = attention::AttentionAccumulator::default();
            loop {
                match events.recv().await {
                    Ok(PtyEvent::Output(data)) => {
                        for matched in accumulator.push_chunk(&data) {
                            if last_preview
                                .as_ref()
                                .map(|prev| prev == &matched.preview)
                                .unwrap_or(false)
                            {
                                continue;
                            }
                            last_preview = Some(matched.preview.clone());
                            let summary = AttentionSummary {
                                profile: matched.profile.to_string(),
                                attention_type: matched.attention_type,
                                preview: matched.preview,
                                triggered_at: chrono::Utc::now(),
                            };
                            state.record_attention(session_id, summary).await;
                        }
                    }
                    Ok(PtyEvent::Exited { .. }) => break,
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
        });
    }

    async fn record_attention(&self, session_id: SessionId, summary: AttentionSummary) {
        {
            let mut registry = self.registry.write().await;
            if let Some(session) = registry.get_mut(session_id) {
                session.last_attention = Some(summary.clone());
                session.updated_at = chrono::Utc::now();
            } else {
                return;
            }
        }

        let _ = self.save_registry().await;

        self.broadcast(DaemonNotification::Attention {
            session_id,
            profile: summary.profile.clone(),
            attention_type: summary.attention_type,
            preview: summary.preview.clone(),
            triggered_at: summary.triggered_at,
        });

        // Send terminal notification with time-based debouncing
        {
            let mut debouncers = self.notification_debouncers.lock().await;
            let debouncer = debouncers
                .entry(session_id)
                .or_insert_with(NotificationDebouncer::new);
            if debouncer.should_notify() {
                notify_attention(&summary.profile, summary.attention_type, &summary.preview);
            }
        }
    }

    /// Clean up notification debouncer when session stops
    async fn clear_notification_debouncer(&self, session_id: SessionId) {
        let mut debouncers = self.notification_debouncers.lock().await;
        debouncers.remove(&session_id);
    }
}

/// Update a task's session badge in a markdown file atomically
///
/// This function reads the file fresh before writing to avoid clobbering
/// concurrent edits by the user or UI.
///
/// Returns an error if the task was not found in the file (e.g., if the user
/// renamed or deleted it between the initial parse and this write).
async fn update_markdown_badge(
    project_path: &str,
    task_name: &str,
    session_status: Option<&TaskSessionStatus>,
) -> Result<()> {
    let project_file = PathBuf::from(project_path);

    // Read fresh content to avoid clobbering concurrent edits
    let content = tokio::fs::read_to_string(&project_file)
        .await
        .with_context(|| format!("Failed to read {}", project_path))?;

    // Apply the badge update
    let result = update_task_session_in_content(&content, task_name, session_status);

    // Check if the task was actually found and updated
    if !result.task_found {
        anyhow::bail!(
            "Task '{}' not found in '{}' - it may have been renamed or deleted",
            task_name,
            project_path
        );
    }

    // Write atomically
    atomic_write(&project_file, &result.content)
        .with_context(|| format!("Failed to write {}", project_path))?;

    Ok(())
}

/// Handle a single client connection
async fn handle_client(
    state: Arc<DaemonState>,
    mut stream: UnixStream,
    shutdown_tx: tokio::sync::mpsc::Sender<()>,
) -> Result<()> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // Subscribe to updates for this client
    let mut updates_rx = state.updates_tx.subscribe();

    loop {
        tokio::select! {
            // Handle incoming requests
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => {
                        // Client disconnected
                        break;
                    }
                    Ok(_) => {
                        use rn_desktop_2_lib::session::protocol::{DaemonErrorCode, MAX_REQUEST_FRAME_SIZE};

                        // Enforce max request frame size (1MB)
                        let response = if line.len() > MAX_REQUEST_FRAME_SIZE {
                            DaemonResponse::Error {
                                code: DaemonErrorCode::InvalidRequest,
                                message: format!(
                                    "Request frame too large: {} bytes (max {})",
                                    line.len(),
                                    MAX_REQUEST_FRAME_SIZE
                                ),
                            }
                        } else {
                            match deserialize_message::<DaemonRequest>(line.as_bytes()) {
                                Ok(request) => {
                                    handle_request(&state, request, &shutdown_tx).await
                                }
                                Err(e) => {
                                    DaemonResponse::Error {
                                        code: DaemonErrorCode::InvalidRequest,
                                        message: format!("Failed to parse request: {}", e),
                                    }
                                }
                            }
                        };

                        // Send response
                        let bytes = serialize_message(&response)?;
                        writer.write_all(&bytes).await?;
                        writer.flush().await?;

                        line.clear();
                    }
                    Err(e) => {
                        eprintln!("Error reading from client: {}", e);
                        break;
                    }
                }
            }

            // Forward broadcast updates to client
            result = updates_rx.recv() => {
                match result {
                    Ok(notification) => {
                        let bytes = serialize_message(&notification)?;
                        if writer.write_all(&bytes).await.is_err() {
                            break; // Client disconnected
                        }
                        let _ = writer.flush().await;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Client is too slow, skip missed messages
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Handle a single request from a client
async fn handle_request(
    state: &Arc<DaemonState>,
    request: DaemonRequest,
    shutdown_tx: &tokio::sync::mpsc::Sender<()>,
) -> DaemonResponse {
    use rn_desktop_2_lib::session::protocol::{DaemonErrorCode, PROTOCOL_VERSION};

    match request {
        DaemonRequest::Handshake { client_version } => {
            if client_version != PROTOCOL_VERSION {
                let message = if client_version < PROTOCOL_VERSION {
                    "Daemon is newer than app—please update the app.".to_string()
                } else {
                    "Daemon is outdated—please restart daemon.".to_string()
                };
                return DaemonResponse::Error {
                    code: DaemonErrorCode::VersionMismatch,
                    message,
                };
            }
            DaemonResponse::Handshake {
                protocol_version: PROTOCOL_VERSION,
            }
        }

        DaemonRequest::Ping => DaemonResponse::Pong,

        DaemonRequest::Shutdown => {
            // Signal main loop to shut down
            let _ = shutdown_tx.send(()).await;
            DaemonResponse::ShuttingDown
        }

        DaemonRequest::Start {
            task_key,
            task_id,
            project_path,
            shell,
        } => {
            // Read and parse the markdown file
            let project_file = PathBuf::from(&project_path);
            let content = match tokio::fs::read_to_string(&project_file).await {
                Ok(c) => c,
                Err(e) => {
                    return DaemonResponse::Error {
                        code: DaemonErrorCode::Internal,
                        message: format!("Failed to read project file '{}': {}", project_path, e),
                    };
                }
            };

            // Parse and find the task
            let blocks = parse_body(&content);
            let task = match find_task_by_key(&blocks, &task_key) {
                Some(t) => t,
                None => {
                    return DaemonResponse::Error {
                        code: DaemonErrorCode::Internal,
                        message: format!(
                            "No task matching '{}' found in '{}'",
                            task_key, project_path
                        ),
                    };
                }
            };

            // Use the full task name as the key, and extract task_id from markdown if not provided
            let full_task_name = task.name.clone();
            let resolved_task_id = task_id.or_else(|| task.task_id.clone());

            let mut registry = state.registry.write().await;

            // Check if session already exists for this task
            if let Some(existing) = registry.find_by_task_key(&full_task_name, &project_path) {
                return DaemonResponse::Error {
                    code: DaemonErrorCode::Internal,
                    message: format!(
                        "Session already exists for task '{}' (id: {})",
                        full_task_name, existing.id
                    ),
                };
            }

            // Allocate new session
            let id = registry.allocate_id();
            let mut session = Session::new(
                id,
                full_task_name.clone(),
                resolved_task_id,
                project_path.clone(),
            );
            session.status = SessionStatus::Running;
            session.exit_code = None;

            // Spawn the PTY with environment variables for shell integration
            let pty = match PtyRuntime::spawn(id, shell, &full_task_name, &project_path) {
                Ok(p) => p,
                Err(e) => {
                    return DaemonResponse::Error {
                        code: DaemonErrorCode::Internal,
                        message: format!("Failed to spawn PTY: {}", e),
                    };
                }
            };

            // Store the PTY handle
            {
                let mut handles = state.pty_handles.lock().await;
                handles.insert(id, pty);
            }
            state.clear_completed_tail(id).await;

            // Update the markdown with session badge (reads fresh content to avoid clobbering)
            let session_status = TaskSessionStatus {
                status: SessionStatus::Running,
                session_id: id,
            };
            if let Err(e) =
                update_markdown_badge(&project_path, &full_task_name, Some(&session_status)).await
            {
                // Clean up PTY on failure
                let mut handles = state.pty_handles.lock().await;
                if let Some(mut pty) = handles.remove(&id) {
                    pty.stop();
                }
                return DaemonResponse::Error {
                    code: DaemonErrorCode::Internal,
                    message: format!("Failed to update markdown file: {}", e),
                };
            }

            registry.insert(session.clone());

            // Save to disk
            drop(registry);
            if let Err(e) = state.save_registry().await {
                return DaemonResponse::Error {
                    code: DaemonErrorCode::Internal,
                    message: format!("Failed to save session: {}", e),
                };
            }

            // Spawn output watcher task
            let state_clone = Arc::clone(state);
            let session_id = id;
            let project_path_clone = project_path.clone();
            let task_key_clone = full_task_name.clone();
            tokio::spawn(async move {
                watch_pty_output(state_clone, session_id, project_path_clone, task_key_clone).await;
            });
            state.spawn_attention_monitor(session_id);

            // Broadcast update
            state.broadcast(DaemonNotification::SessionUpdated {
                session: session.clone(),
            });

            DaemonResponse::SessionStarted { session }
        }

        DaemonRequest::Continue {
            session_id,
            tail_bytes,
        } => {
            let registry = state.registry.read().await;

            match registry.get(session_id) {
                Some(session) => {
                    let tail = state
                        .session_tail(session_id, tail_bytes.unwrap_or(DEFAULT_TAIL_BYTES))
                        .await;
                    DaemonResponse::SessionContinued {
                        session: session.clone(),
                        tail,
                    }
                }
                None => DaemonResponse::Error {
                    code: DaemonErrorCode::NotFound,
                    message: format!("Session {} not found", session_id),
                },
            }
        }

        DaemonRequest::Attach {
            session_id,
            tail_bytes,
        } => {
            let session = {
                let registry = state.registry.read().await;
                match registry.get(session_id) {
                    Some(session) => session.clone(),
                    None => {
                        return DaemonResponse::Error {
                            code: DaemonErrorCode::NotFound,
                            message: format!("Session {} not found", session_id),
                        };
                    }
                }
            };

            let is_running = {
                let handles = state.pty_handles.lock().await;
                handles.contains_key(&session_id)
            };

            if !is_running {
                return DaemonResponse::Error {
                    code: DaemonErrorCode::Internal,
                    message: format!("Session {} is not running", session_id),
                };
            }

            let tail = state
                .session_tail(session_id, tail_bytes.unwrap_or(DEFAULT_TAIL_BYTES))
                .await;

            let socket_path = match state.prepare_attach_socket(session_id).await {
                Ok(path) => path,
                Err(e) => {
                    return DaemonResponse::Error {
                        code: DaemonErrorCode::Internal,
                        message: format!("Failed to prepare attach socket: {}", e),
                    };
                }
            };

            DaemonResponse::AttachReady {
                session,
                tail,
                socket_path: socket_path.to_string_lossy().to_string(),
            }
        }

        DaemonRequest::Resize {
            session_id,
            cols,
            rows,
        } => {
            let handles = state.pty_handles.lock().await;
            match handles.get(&session_id) {
                Some(runtime) => match runtime.resize(cols, rows) {
                    Ok(_) => DaemonResponse::SessionResized {
                        session_id,
                        cols,
                        rows,
                    },
                    Err(e) => DaemonResponse::Error {
                        code: DaemonErrorCode::Internal,
                        message: format!("Failed to resize session {}: {}", session_id, e),
                    },
                },
                None => DaemonResponse::Error {
                    code: DaemonErrorCode::Internal,
                    message: format!("Session {} is not running", session_id),
                },
            }
        }

        DaemonRequest::List { project_path } => {
            let registry = state.registry.read().await;

            let sessions = match project_path {
                Some(ref path) => registry
                    .sessions_for_project(path)
                    .into_iter()
                    .cloned()
                    .collect(),
                None => registry.all_sessions().into_iter().cloned().collect(),
            };

            DaemonResponse::SessionList { sessions }
        }

        DaemonRequest::Stop { session_id } => {
            // Stop the PTY first and capture its final output
            let tail_snapshot = {
                let mut handles = state.pty_handles.lock().await;
                if let Some(mut pty) = handles.remove(&session_id) {
                    pty.stop();
                    Some(pty.get_recent_output_blocking(DEFAULT_TAIL_BYTES))
                } else {
                    None
                }
            };
            if let Some(data) = tail_snapshot {
                state.store_completed_tail(session_id, data).await;
            }
            state.remove_attach_socket(session_id).await;
            state.clear_notification_debouncer(session_id).await;

            let mut registry = state.registry.write().await;

            match registry.get_mut(session_id) {
                Some(session) => {
                    session.status = SessionStatus::Stopped;
                    session.exit_code = None;
                    session.updated_at = chrono::Utc::now();
                    let session = session.clone();
                    let project_path = session.project_path.clone();
                    let task_key = session.task_key.clone();

                    // Save to disk first
                    drop(registry);
                    if let Err(e) = state.save_registry().await {
                        return DaemonResponse::Error {
                            code: DaemonErrorCode::Internal,
                            message: format!("Failed to save session: {}", e),
                        };
                    }

                    // Update the markdown file (reads fresh to avoid clobbering)
                    let session_status = TaskSessionStatus {
                        status: SessionStatus::Stopped,
                        session_id,
                    };
                    let _ = update_markdown_badge(&project_path, &task_key, Some(&session_status))
                        .await;

                    // Broadcast update
                    state.broadcast(DaemonNotification::SessionUpdated {
                        session: session.clone(),
                    });

                    DaemonResponse::SessionStopped { session }
                }
                None => DaemonResponse::Error {
                    code: DaemonErrorCode::NotFound,
                    message: format!("Session {} not found", session_id),
                },
            }
        }

        DaemonRequest::Tail { session_id, bytes } => {
            // Default to 4096 bytes if not specified
            let max_bytes = bytes.unwrap_or(DEFAULT_TAIL_BYTES);
            match state.session_tail(session_id, max_bytes).await {
                Some(data) => DaemonResponse::SessionTail { session_id, data },
                None => DaemonResponse::Error {
                    code: DaemonErrorCode::Internal,
                    message: format!(
                        "Session {} is not running or has no PTY output available",
                        session_id
                    ),
                },
            }
        }

        DaemonRequest::Status { session_id } => {
            let registry = state.registry.read().await;

            match registry.get(session_id) {
                Some(session) => DaemonResponse::SessionStatus {
                    session: session.clone(),
                },
                None => DaemonResponse::Error {
                    code: DaemonErrorCode::NotFound,
                    message: format!("Session {} not found", session_id),
                },
            }
        }

        DaemonRequest::CrLatest {
            project_path,
            task_id,
        } => {
            use rn_desktop_2_lib::context_resurrection::query;
            use rn_desktop_2_lib::session::protocol::DaemonErrorCode;

            match query::cr_latest(&state.snapshot_store, &project_path, task_id.as_deref()) {
                Ok(snapshot) => DaemonResponse::CrSnapshot { snapshot },
                Err(e) => DaemonResponse::Error {
                    code: DaemonErrorCode::Internal,
                    message: e,
                },
            }
        }

        DaemonRequest::CrList {
            project_path,
            task_id,
            limit,
        } => {
            use rn_desktop_2_lib::context_resurrection::query;
            use rn_desktop_2_lib::session::protocol::DaemonErrorCode;

            // Enforce limit semantics: None => 100, >500 => 500, <=0 => error
            let enforced_limit = match limit {
                None => Some(100),
                Some(n) if n <= 0 => {
                    return DaemonResponse::Error {
                        code: DaemonErrorCode::InvalidRequest,
                        message: "limit must be greater than 0".to_string(),
                    };
                }
                Some(n) if n > 500 => Some(500),
                Some(n) => Some(n),
            };

            match query::cr_list(
                &state.snapshot_store,
                &project_path,
                &task_id,
                enforced_limit,
            ) {
                Ok(snapshots) => DaemonResponse::CrSnapshots { snapshots },
                Err(e) => DaemonResponse::Error {
                    code: DaemonErrorCode::Internal,
                    message: e,
                },
            }
        }

        DaemonRequest::CrGet {
            project_path,
            task_id,
            snapshot_id,
        } => {
            use rn_desktop_2_lib::session::protocol::DaemonErrorCode;

            if !state.snapshot_store.is_available() {
                DaemonResponse::Error {
                    code: DaemonErrorCode::StoreUnavailable,
                    message: "Snapshot store is unavailable".to_string(),
                }
            } else {
                let project = std::path::Path::new(&project_path);
                match state
                    .snapshot_store
                    .read_snapshot(project, &task_id, &snapshot_id)
                {
                    Ok(snapshot) => DaemonResponse::CrSnapshot {
                        snapshot: Some(snapshot),
                    },
                    Err(e) => {
                        let code = if e.kind() == std::io::ErrorKind::NotFound {
                            DaemonErrorCode::NotFound
                        } else {
                            DaemonErrorCode::Internal
                        };
                        DaemonResponse::Error {
                            code,
                            message: format!("Failed to read snapshot: {}", e),
                        }
                    }
                }
            }
        }

        DaemonRequest::CrCaptureNow {
            project_path,
            task_id,
            user_note,
        } => {
            use rn_desktop_2_lib::context_resurrection::query;
            use rn_desktop_2_lib::session::protocol::DaemonErrorCode;

            // Find the task to get task_title and session_id
            let (task_title, session_id) = {
                let registry = state.registry.read().await;
                let session = registry.sessions.values().find(|s| {
                    s.task_id.as_deref() == Some(&task_id) && s.project_path == project_path
                });

                match session {
                    Some(s) => (s.task_key.clone(), Some(s.id)),
                    None => {
                        // No active session - use task_id as title and no session_id
                        (task_id.clone(), None)
                    }
                }
            };

            // Lock capture service and call handler
            let capture_service_guard = state.capture_service.lock().await;
            match capture_service_guard.as_ref() {
                Some(capture_service) => {
                    match query::cr_capture_now(
                        capture_service,
                        &project_path,
                        &task_id,
                        &task_title,
                        session_id,
                        user_note,
                    ) {
                        Ok(Some(snapshot)) => DaemonResponse::CrSnapshot {
                            snapshot: Some(snapshot),
                        },
                        Ok(None) => DaemonResponse::Error {
                            code: DaemonErrorCode::Skipped,
                            message: "Capture was skipped (dedup/rate-limit)".to_string(),
                        },
                        Err(e) => DaemonResponse::Error {
                            code: DaemonErrorCode::Internal,
                            message: e,
                        },
                    }
                }
                None => DaemonResponse::Error {
                    code: DaemonErrorCode::StoreUnavailable,
                    message: "Context Resurrection capture service is unavailable".to_string(),
                },
            }
        }

        DaemonRequest::CrDeleteTask {
            project_path,
            task_id,
        } => {
            use rn_desktop_2_lib::context_resurrection::query;
            use rn_desktop_2_lib::session::protocol::DaemonErrorCode;

            match query::cr_delete_task(&state.snapshot_store, &project_path, &task_id) {
                Ok(deleted_count) => DaemonResponse::CrDeleted { deleted_count },
                Err(e) => DaemonResponse::Error {
                    code: DaemonErrorCode::Internal,
                    message: e,
                },
            }
        }

        DaemonRequest::CrDeleteProject { project_path } => {
            use rn_desktop_2_lib::context_resurrection::query;
            use rn_desktop_2_lib::session::protocol::DaemonErrorCode;

            match query::cr_delete_project(&state.snapshot_store, &project_path) {
                Ok(deleted_count) => DaemonResponse::CrDeleted { deleted_count },
                Err(e) => DaemonResponse::Error {
                    code: DaemonErrorCode::Internal,
                    message: e,
                },
            }
        }
    }
}

/// Watch PTY output and update session status based on activity
async fn watch_pty_output(
    state: Arc<DaemonState>,
    session_id: SessionId,
    project_path: String,
    task_key: String,
) {
    let mut last_status = SessionStatus::Running;
    let mut idle_start: Option<std::time::Instant> = None;
    let mut last_idle_capture: Option<std::time::Instant> = None;

    loop {
        // Poll every 5 seconds
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        // Check PTY status
        let (is_alive, is_idle) = {
            let handles = state.pty_handles.lock().await;
            match handles.get(&session_id) {
                Some(pty) => {
                    let alive = pty.is_alive();
                    let idle = pty.is_idle();
                    (alive, idle)
                }
                None => break, // PTY was removed
            }
        };

        if !is_alive {
            // PTY exited
            eprintln!("Session {} PTY exited", session_id);
            let (exit_code, tail_snapshot) = {
                let handles = state.pty_handles.lock().await;
                match handles.get(&session_id) {
                    Some(pty) => (
                        pty.exit_code(),
                        Some(pty.get_recent_output_blocking(DEFAULT_TAIL_BYTES)),
                    ),
                    None => (None, None),
                }
            };

            update_session_status(
                &state,
                session_id,
                &project_path,
                &task_key,
                SessionStatus::Stopped,
                exit_code,
            )
            .await;

            if let Some(data) = tail_snapshot {
                state.store_completed_tail(session_id, data).await;
            }
            state.remove_attach_socket(session_id).await;
            state.clear_notification_debouncer(session_id).await;

            // Remove PTY handle
            let mut handles = state.pty_handles.lock().await;
            handles.remove(&session_id);
            break;
        }

        // Determine new status based on idle state
        let new_status = if is_idle {
            SessionStatus::Waiting
        } else {
            SessionStatus::Running
        };

        // Track idle duration for timeout captures
        if is_idle {
            if idle_start.is_none() {
                idle_start = Some(std::time::Instant::now());
            }
        } else {
            idle_start = None;
            last_idle_capture = None; // Reset capture tracking when activity resumes
        }

        // Trigger idle timeout capture if threshold exceeded
        if let Some(start) = idle_start {
            let idle_duration = start.elapsed();
            let threshold = std::time::Duration::from_secs(IDLE_CAPTURE_THRESHOLD_SECS);

            // Capture once per idle period (avoid repeated captures)
            if idle_duration >= threshold && last_idle_capture.is_none() {
                let task_id = {
                    let registry = state.registry.read().await;
                    registry.get(session_id).and_then(|s| s.task_id.clone())
                };

                state.trigger_capture(
                    project_path.clone(),
                    task_id,
                    task_key.clone(),
                    session_id,
                    CaptureReason::IdleTimeout,
                );

                last_idle_capture = Some(std::time::Instant::now());
            }
        }

        // Only update if status changed
        if new_status != last_status {
            update_session_status(
                &state,
                session_id,
                &project_path,
                &task_key,
                new_status,
                None,
            )
            .await;
            last_status = new_status;
        }
    }
}

/// Helper to update session status in registry and markdown
async fn update_session_status(
    state: &Arc<DaemonState>,
    session_id: SessionId,
    project_path: &str,
    task_key: &str,
    new_status: SessionStatus,
    exit_code: Option<i32>,
) {
    let (old_status, task_id) = {
        let mut registry = state.registry.write().await;
        if let Some(session) = registry.get_mut(session_id) {
            if session.status == new_status {
                return; // No change needed
            }
            let old = session.status;
            session.status = new_status;
            session.updated_at = chrono::Utc::now();
            if let Some(code) = exit_code {
                session.exit_code = Some(code);
            }
            (old, session.task_id.clone())
        } else {
            return;
        }
    };

    // Save registry
    let _ = state.save_registry().await;

    // Update markdown file (reads fresh to avoid clobbering concurrent edits)
    let session_status = TaskSessionStatus {
        status: new_status,
        session_id,
    };
    let _ = update_markdown_badge(project_path, task_key, Some(&session_status)).await;

    // Trigger context capture on status transitions
    let capture_reason = match new_status {
        SessionStatus::Stopped => Some(CaptureReason::SessionStopped),
        SessionStatus::Waiting if old_status == SessionStatus::Running => {
            Some(CaptureReason::SessionWaiting)
        }
        SessionStatus::Running if old_status == SessionStatus::Waiting => {
            Some(CaptureReason::SessionRunning)
        }
        _ => None,
    };

    if let Some(reason) = capture_reason {
        state.trigger_capture(
            project_path.to_string(),
            task_id,
            task_key.to_string(),
            session_id,
            reason,
        );
    }

    // Broadcast update
    let registry = state.registry.read().await;
    if let Some(session) = registry.get(session_id) {
        state.broadcast(DaemonNotification::SessionUpdated {
            session: session.clone(),
        });
    }
}

// ============================================================================
// SessionProvider implementation for Context Resurrection
// ============================================================================

/// Tail size for CR captures (8KB is enough for context without bloating snapshots)
const CR_TAIL_BYTES: usize = 8 * 1024;

impl SessionProvider for DaemonState {
    fn get_session_state(&self, session_id: u64) -> Option<SessionSnapshot> {
        // Helper to run async code from sync context, handling nested runtime case
        fn run_async<F, T>(future: F) -> T
        where
            F: std::future::Future<Output = T> + Send,
            T: Send,
        {
            match tokio::runtime::Handle::try_current() {
                Ok(handle) => {
                    // Already in a runtime - use block_in_place to safely block
                    tokio::task::block_in_place(|| handle.block_on(future))
                }
                Err(_) => {
                    // Not in a runtime - create one
                    tokio::runtime::Runtime::new().unwrap().block_on(future)
                }
            }
        }

        // Get session metadata from registry
        let session = run_async(async {
            let registry = self.registry.read().await;
            registry.get(session_id).cloned()
        })?;

        // Get tail from running PTY or completed tail storage
        let tail_bytes = run_async(async { self.session_tail(session_id, CR_TAIL_BYTES).await })
            .unwrap_or_default();

        // Decode tail as UTF-8 (lossy is fine for CR - we just need context)
        let tail = String::from_utf8_lossy(&tail_bytes).to_string();

        // Map protocol SessionStatus to CR SessionStatus
        let status = match session.status {
            SessionStatus::Running => {
                rn_desktop_2_lib::context_resurrection::models::SessionStatus::Running
            }
            SessionStatus::Waiting => {
                rn_desktop_2_lib::context_resurrection::models::SessionStatus::Waiting
            }
            SessionStatus::Stopped => {
                rn_desktop_2_lib::context_resurrection::models::SessionStatus::Stopped
            }
        };

        // Map protocol AttentionSummary to CR AttentionSummary
        let last_attention = session.last_attention.map(|att| {
            rn_desktop_2_lib::context_resurrection::models::AttentionSummary {
                attention_type: match att.attention_type {
                    rn_desktop_2_lib::session::protocol::AttentionType::InputRequired => {
                        rn_desktop_2_lib::context_resurrection::models::AttentionType::InputRequired
                    }
                    rn_desktop_2_lib::session::protocol::AttentionType::DecisionPoint => {
                        rn_desktop_2_lib::context_resurrection::models::AttentionType::DecisionPoint
                    }
                    rn_desktop_2_lib::session::protocol::AttentionType::Completed => {
                        rn_desktop_2_lib::context_resurrection::models::AttentionType::Completed
                    }
                    rn_desktop_2_lib::session::protocol::AttentionType::Error => {
                        rn_desktop_2_lib::context_resurrection::models::AttentionType::Error
                    }
                },
                preview: att.preview,
                triggered_at: att.triggered_at.to_rfc3339(),
            }
        });

        Some(SessionSnapshot {
            status,
            exit_code: session.exit_code,
            last_attention,
            tail,
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = Config::from_env();

    // Ensure data directory exists
    config
        .ensure_dirs()
        .context("Failed to create data directory")?;

    // Clean up stale socket if exists
    if config.socket_exists() {
        if config.is_daemon_running() {
            eprintln!("Daemon already running (PID: {:?})", config.read_pid());
            std::process::exit(1);
        }
        // Stale socket, remove it
        config
            .remove_socket()
            .context("Failed to remove stale socket")?;
    }

    // Write PID file
    config.write_pid().context("Failed to write PID file")?;

    // Initialize daemon state
    let state = Arc::new(DaemonState::new(config.clone())?);

    // Initialize Context Resurrection capture service
    state.init_capture_service().await;

    // Reconcile any stale sessions from previous runs
    state.reconcile_stale_sessions().await;

    // Create Unix socket listener
    let listener = UnixListener::bind(&config.socket_path)
        .with_context(|| format!("Failed to bind socket: {}", config.socket_path.display()))?;

    // Secure socket permissions (Unix only - owner-only access)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&config.socket_path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| {
                format!(
                    "Failed to set socket permissions: {}",
                    config.socket_path.display()
                )
            })?;
    }

    println!("Daemon listening on {}", config.socket_path.display());

    // Shutdown signal channel
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Handle SIGTERM/SIGINT for graceful shutdown
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        let _ = signal::ctrl_c().await;
        let _ = shutdown_tx_clone.send(()).await;
    });

    // Accept connections until shutdown
    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _addr)) => {
                        let state = Arc::clone(&state);
                        let shutdown_tx = shutdown_tx.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_client(state, stream, shutdown_tx).await {
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
                println!("Shutting down daemon...");
                break;
            }
        }
    }

    // Cleanup
    config.remove_pid().ok();
    config.remove_socket().ok();

    println!("Daemon stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rn_desktop_2_lib::session::protocol::AttentionType;
    use rn_desktop_2_lib::test_utils::{assert_eventually, assert_eventually_bool};
    use std::time::Duration;
    use tempfile::TempDir;

    fn test_config() -> (Config, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = Config {
            runtime_dir: temp_dir.path().to_path_buf(),
            state_dir: temp_dir.path().to_path_buf(),
            socket_path: temp_dir.path().join("daemon.sock"),
            pid_file: temp_dir.path().join("daemon.pid"),
        };
        (config, temp_dir)
    }

    #[tokio::test]
    async fn test_start_session_updates_markdown() {
        let (config, temp_dir) = test_config();

        // Create a markdown file
        let markdown_path = temp_dir.path().join("TODO.md");
        let initial_content = "# Tasks\n- [ ] Build feature\n- [ ] Write tests\n";
        tokio::fs::write(&markdown_path, initial_content)
            .await
            .unwrap();

        // Create daemon state
        let state = Arc::new(DaemonState::new(config).unwrap());
        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        // Start a session
        let request = DaemonRequest::Start {
            task_key: "Build".to_string(),
            task_id: None,
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec!["echo".to_string(), "hello".to_string()]),
        };

        let response = handle_request(&state, request, &shutdown_tx).await;

        // Verify response
        match response {
            DaemonResponse::SessionStarted { session } => {
                assert_eq!(session.task_key, "Build feature");
                assert_eq!(session.status, SessionStatus::Running);
            }
            DaemonResponse::Error { code: _, message } => {
                panic!("Start failed: {}", message);
            }
            _ => panic!("Unexpected response"),
        }

        // Verify markdown was updated
        let updated_content = tokio::fs::read_to_string(&markdown_path).await.unwrap();
        assert!(
            updated_content.contains("[Running](todos://session/0)"),
            "Expected markdown to contain session badge. Got: {}",
            updated_content
        );
    }

    #[tokio::test]
    async fn test_stop_session_updates_markdown() {
        let (config, temp_dir) = test_config();

        // Create a markdown file with a running session
        let markdown_path = temp_dir.path().join("TODO.md");
        let initial_content =
            "# Tasks\n- [ ] Build feature [Running](todos://session/0)\n- [ ] Write tests\n";
        tokio::fs::write(&markdown_path, initial_content)
            .await
            .unwrap();

        // Create daemon state with existing session
        let state = Arc::new(DaemonState::new(config).unwrap());
        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        // Manually insert session into registry
        {
            let mut registry = state.registry.write().await;
            let mut session = Session::new(
                0,
                "Build feature".to_string(),
                None,
                markdown_path.to_string_lossy().to_string(),
            );
            session.status = SessionStatus::Running;
            registry.insert(session);
            registry.next_id = 1;
        }

        // Stop the session
        let request = DaemonRequest::Stop { session_id: 0 };
        let response = handle_request(&state, request, &shutdown_tx).await;

        // Verify response
        match response {
            DaemonResponse::SessionStopped { session } => {
                assert_eq!(session.status, SessionStatus::Stopped);
            }
            DaemonResponse::Error { code: _, message } => {
                panic!("Stop failed: {}", message);
            }
            _ => panic!("Unexpected response"),
        }

        // Verify markdown was updated
        let updated_content = tokio::fs::read_to_string(&markdown_path).await.unwrap();
        assert!(
            updated_content.contains("[Stopped](todos://session/0)"),
            "Expected markdown to show Stopped. Got: {}",
            updated_content
        );
    }

    #[tokio::test]
    async fn test_start_nonexistent_task_fails() {
        let (config, temp_dir) = test_config();

        // Create a markdown file
        let markdown_path = temp_dir.path().join("TODO.md");
        let initial_content = "# Tasks\n- [ ] Build feature\n";
        tokio::fs::write(&markdown_path, initial_content)
            .await
            .unwrap();

        // Create daemon state
        let state = Arc::new(DaemonState::new(config).unwrap());
        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        // Try to start a session for nonexistent task
        let request = DaemonRequest::Start {
            task_key: "nonexistent".to_string(),
            task_id: None,
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: None,
        };

        let response = handle_request(&state, request, &shutdown_tx).await;

        // Verify error response
        match response {
            DaemonResponse::Error { code: _, message } => {
                assert!(
                    message.contains("No task matching"),
                    "Expected 'No task matching' error. Got: {}",
                    message
                );
            }
            _ => panic!("Expected error response"),
        }
    }

    #[tokio::test]
    async fn test_start_duplicate_session_fails() {
        let (config, temp_dir) = test_config();

        // Create a markdown file
        let markdown_path = temp_dir.path().join("TODO.md");
        let initial_content = "# Tasks\n- [ ] Build feature\n";
        tokio::fs::write(&markdown_path, initial_content)
            .await
            .unwrap();

        // Create daemon state
        let state = Arc::new(DaemonState::new(config).unwrap());
        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        // Start first session
        let request = DaemonRequest::Start {
            task_key: "Build".to_string(),
            task_id: None,
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec!["echo".to_string(), "hello".to_string()]),
        };
        let _ = handle_request(&state, request, &shutdown_tx).await;

        // Try to start duplicate session
        let request = DaemonRequest::Start {
            task_key: "Build".to_string(),
            task_id: None,
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec!["echo".to_string(), "hello".to_string()]),
        };
        let response = handle_request(&state, request, &shutdown_tx).await;

        // Verify error response
        match response {
            DaemonResponse::Error { code: _, message } => {
                assert!(
                    message.contains("already exists"),
                    "Expected 'already exists' error. Got: {}",
                    message
                );
            }
            _ => panic!("Expected error response"),
        }
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let (config, temp_dir) = test_config();

        // Create a markdown file
        let markdown_path = temp_dir.path().join("TODO.md");
        let initial_content = "# Tasks\n- [ ] Task 1\n- [ ] Task 2\n";
        tokio::fs::write(&markdown_path, initial_content)
            .await
            .unwrap();

        // Create daemon state
        let state = Arc::new(DaemonState::new(config).unwrap());
        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        // Start two sessions
        let request = DaemonRequest::Start {
            task_key: "Task 1".to_string(),
            task_id: None,
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec!["echo".to_string(), "1".to_string()]),
        };
        let _ = handle_request(&state, request, &shutdown_tx).await;

        let request = DaemonRequest::Start {
            task_key: "Task 2".to_string(),
            task_id: None,
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec!["echo".to_string(), "2".to_string()]),
        };
        let _ = handle_request(&state, request, &shutdown_tx).await;

        // List all sessions
        let request = DaemonRequest::List { project_path: None };
        let response = handle_request(&state, request, &shutdown_tx).await;

        match response {
            DaemonResponse::SessionList { sessions } => {
                assert_eq!(sessions.len(), 2);
            }
            _ => panic!("Expected SessionList response"),
        }

        // List sessions for specific project
        let request = DaemonRequest::List {
            project_path: Some(markdown_path.to_string_lossy().to_string()),
        };
        let response = handle_request(&state, request, &shutdown_tx).await;

        match response {
            DaemonResponse::SessionList { sessions } => {
                assert_eq!(sessions.len(), 2);
            }
            _ => panic!("Expected SessionList response"),
        }
    }

    #[tokio::test]
    async fn test_tail_returns_output() {
        let (config, temp_dir) = test_config();

        // Create markdown file
        let markdown_path = temp_dir.path().join("TODO.md");
        let initial_content = "# Tasks\n- [ ] Tail test\n";
        tokio::fs::write(&markdown_path, initial_content)
            .await
            .unwrap();

        let state = Arc::new(DaemonState::new(config).unwrap());
        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        let request = DaemonRequest::Start {
            task_key: "Tail".to_string(),
            task_id: None,
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec!["echo".to_string(), "tail-output".to_string()]),
        };

        let response = handle_request(&state, request, &shutdown_tx).await;
        let session_id = match response {
            DaemonResponse::SessionStarted { session } => session.id,
            other => panic!("Start failed: {:?}", other),
        };

        // Wait for tail to contain expected output (PTY must exit and tail must be stored)
        assert_eventually_bool(
            "tail to contain 'tail-output'",
            Duration::from_secs(3),
            Duration::from_millis(50),
            || {
                let state = Arc::clone(&state);
                let shutdown_tx = shutdown_tx.clone();
                async move {
                    let tail_request = DaemonRequest::Tail {
                        session_id,
                        bytes: Some(1024),
                    };
                    match handle_request(&state, tail_request, &shutdown_tx).await {
                        DaemonResponse::SessionTail { data, .. } => {
                            let text = String::from_utf8_lossy(&data);
                            text.contains("tail-output")
                        }
                        _ => false,
                    }
                }
            },
        )
        .await;
    }

    #[tokio::test]
    async fn test_attach_request_creates_socket() {
        let (config, temp_dir) = test_config();
        let markdown_path = temp_dir.path().join("TODO.md");
        let initial_content = "# Tasks\n- [ ] Attach demo\n";
        tokio::fs::write(&markdown_path, initial_content)
            .await
            .unwrap();

        let state = Arc::new(DaemonState::new(config).unwrap());
        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        let start = DaemonRequest::Start {
            task_key: "Attach".to_string(),
            task_id: None,
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec!["sleep".to_string(), "1".to_string()]),
        };
        let response = handle_request(&state, start, &shutdown_tx).await;
        let session_id = match response {
            DaemonResponse::SessionStarted { session } => session.id,
            other => panic!("Expected SessionStarted, got {:?}", other),
        };

        let attach_req = DaemonRequest::Attach {
            session_id,
            tail_bytes: Some(512),
        };
        let response = handle_request(&state, attach_req, &shutdown_tx).await;
        match response {
            DaemonResponse::AttachReady { socket_path, .. } => {
                let stream = tokio::net::UnixStream::connect(socket_path.clone())
                    .await
                    .expect("failed to connect to attach socket");
                drop(stream);
                assert!(
                    std::path::Path::new(&socket_path).exists(),
                    "attach socket should exist on disk"
                );
            }
            other => panic!("Expected AttachReady response, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_attach_streams_output() {
        let (config, temp_dir) = test_config();
        let markdown_path = temp_dir.path().join("TODO.md");
        tokio::fs::write(&markdown_path, "# Tasks\n- [ ] Attach streaming\n")
            .await
            .unwrap();

        let state = Arc::new(DaemonState::new(config).unwrap());
        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        // Start a session that echoes whatever we send
        let start = DaemonRequest::Start {
            task_key: "Attach".to_string(),
            task_id: None,
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec!["cat".to_string()]),
        };
        let response = handle_request(&state, start, &shutdown_tx).await;
        let session_id = match response {
            DaemonResponse::SessionStarted { session } => session.id,
            other => panic!("Expected SessionStarted, got {:?}", other),
        };

        let attach_req = DaemonRequest::Attach {
            session_id,
            tail_bytes: Some(128),
        };
        let response = handle_request(&state, attach_req, &shutdown_tx).await;
        let socket_path = match response {
            DaemonResponse::AttachReady { socket_path, .. } => socket_path,
            other => panic!("Expected AttachReady response, got {:?}", other),
        };

        let stream = tokio::net::UnixStream::connect(socket_path.clone())
            .await
            .expect("failed to connect to attach socket");
        let (mut reader, mut writer) = stream.into_split();

        writer
            .write_all(b"attach-stream-test\n")
            .await
            .expect("failed to send input");
        writer.flush().await.expect("failed to flush writer");

        let mut collected = Vec::new();
        let mut buf = vec![0u8; 1024];
        let mut attempts = 0;

        loop {
            attempts += 1;
            if attempts > 20 {
                panic!("timed out waiting for attach output");
            }

            match tokio::time::timeout(Duration::from_millis(200), reader.read(&mut buf)).await {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => {
                    collected.extend_from_slice(&buf[..n]);
                    if collected.contains(&b'\n') {
                        break;
                    }
                }
                Ok(Err(e)) => panic!("attach read error: {}", e),
                Err(_) => continue,
            }
        }

        let text = String::from_utf8_lossy(&collected);
        assert!(
            text.contains("attach-stream-test"),
            "attach stream should echo input, got '{}'",
            text
        );

        // Stop session to clean up
        let _ = handle_request(&state, DaemonRequest::Stop { session_id }, &shutdown_tx).await;
    }

    #[tokio::test]
    async fn test_resize_request_succeeds() {
        let (config, temp_dir) = test_config();
        let markdown_path = temp_dir.path().join("TODO.md");
        tokio::fs::write(&markdown_path, "# Tasks\n- [ ] Resize task\n")
            .await
            .unwrap();

        let state = Arc::new(DaemonState::new(config).unwrap());
        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        let start = DaemonRequest::Start {
            task_key: "Resize".to_string(),
            task_id: None,
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec!["sleep".to_string(), "2".to_string()]),
        };
        let response = handle_request(&state, start, &shutdown_tx).await;
        let session_id = match response {
            DaemonResponse::SessionStarted { session } => session.id,
            other => panic!("Expected SessionStarted, got {:?}", other),
        };

        let resize_req = DaemonRequest::Resize {
            session_id,
            cols: 132,
            rows: 40,
        };
        match handle_request(&state, resize_req, &shutdown_tx).await {
            DaemonResponse::SessionResized {
                session_id: sid,
                cols,
                rows,
            } => {
                assert_eq!(sid, session_id);
                assert_eq!(cols, 132);
                assert_eq!(rows, 40);
            }
            other => panic!("Expected SessionResized response, got {:?}", other),
        }

        let _ = handle_request(&state, DaemonRequest::Stop { session_id }, &shutdown_tx).await;
    }

    #[tokio::test]
    async fn test_attention_detection_records_summary() {
        let (config, temp_dir) = test_config();
        let markdown_path = temp_dir.path().join("TODO.md");
        tokio::fs::write(&markdown_path, "# Tasks\n- [ ] Attention test\n")
            .await
            .unwrap();

        let state = Arc::new(DaemonState::new(config).unwrap());
        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        // Delay output slightly so the attention monitor subscribes before the first bytes.
        let script = "sleep 0.2; echo \"error: build failed\"; sleep 0.2";
        let start = DaemonRequest::Start {
            task_key: "Attention".to_string(),
            task_id: None,
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                script.to_string(),
            ]),
        };
        let response = handle_request(&state, start, &shutdown_tx).await;
        let session_id = match response {
            DaemonResponse::SessionStarted { session } => session.id,
            other => panic!("Expected SessionStarted, got {:?}", other),
        };

        // Wait for attention summary to be recorded
        assert_eventually(
            "attention summary to be recorded",
            Duration::from_secs(3),
            Duration::from_millis(50),
            || {
                let state = Arc::clone(&state);
                let shutdown_tx = shutdown_tx.clone();
                async move {
                    let continue_req = DaemonRequest::Continue {
                        session_id,
                        tail_bytes: Some(4096),
                    };

                    match handle_request(&state, continue_req, &shutdown_tx).await {
                        DaemonResponse::SessionContinued { session, .. } => {
                            if let Some(summary) = session.last_attention {
                                if summary.attention_type == AttentionType::Error
                                    && summary.preview.to_lowercase().contains("error")
                                {
                                    Ok(summary)
                                } else {
                                    Err(format!(
                                        "Attention summary type/preview mismatch: {:?}",
                                        summary
                                    ))
                                }
                            } else {
                                Err("no attention summary yet".to_string())
                            }
                        }
                        other => Err(format!("Expected SessionContinued, got {:?}", other)),
                    }
                }
            },
        )
        .await;

        let _ = handle_request(&state, DaemonRequest::Stop { session_id }, &shutdown_tx).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_session_provider_returns_non_empty_tail() {
        use crate::SessionProvider;

        let (config, temp_dir) = test_config();
        let markdown_path = temp_dir.path().join("TODO.md");
        tokio::fs::write(&markdown_path, "# Tasks\n- [ ] Provider test\n")
            .await
            .unwrap();

        let state = Arc::new(DaemonState::new(config).unwrap());
        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        // Start a session that produces output
        let script = "echo 'session-provider-test-output'; sleep 0.1";
        let start = DaemonRequest::Start {
            task_key: "Provider".to_string(),
            task_id: Some("abc.provider-test".to_string()),
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                script.to_string(),
            ]),
        };

        let response = handle_request(&state, start, &shutdown_tx).await;
        let session_id = match response {
            DaemonResponse::SessionStarted { session } => session.id,
            other => panic!("Expected SessionStarted, got {:?}", other),
        };

        // Wait for session provider to return non-empty tail with our test output
        let snapshot = assert_eventually(
            "session provider to return tail with test output",
            Duration::from_secs(3),
            Duration::from_millis(50),
            || {
                let state = Arc::clone(&state);
                async move {
                    match state.get_session_state(session_id) {
                        Some(snap) if snap.tail.contains("session-provider-test-output") => {
                            Ok(snap)
                        }
                        Some(snap) => Err(format!(
                            "Tail doesn't contain expected output yet. Got: {}",
                            snap.tail
                        )),
                        None => Err("Session not found".to_string()),
                    }
                }
            },
        )
        .await;

        // Verify snapshot has non-empty tail with our test output
        assert!(
            !snapshot.tail.is_empty(),
            "Tail should be non-empty for session with output"
        );
        assert!(
            snapshot.tail.contains("session-provider-test-output"),
            "Tail should contain test output. Got: {}",
            snapshot.tail
        );

        // Verify status is Stopped or Running (timing-dependent)
        assert!(
            matches!(
                snapshot.status,
                rn_desktop_2_lib::context_resurrection::models::SessionStatus::Stopped
                    | rn_desktop_2_lib::context_resurrection::models::SessionStatus::Running
            ),
            "Status should be Stopped or Running, got: {:?}",
            snapshot.status
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_session_provider_returns_none_for_missing_session() {
        use crate::SessionProvider;

        let (config, _temp_dir) = test_config();
        let state = Arc::new(DaemonState::new(config).unwrap());

        // Request a session that doesn't exist
        let snapshot = state.get_session_state(99999);
        assert!(snapshot.is_none(), "Expected None for non-existent session");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_session_provider_maps_attention_summary() {
        use crate::SessionProvider;

        let (config, temp_dir) = test_config();
        let markdown_path = temp_dir.path().join("TODO.md");
        tokio::fs::write(&markdown_path, "# Tasks\n- [ ] Attention mapping test\n")
            .await
            .unwrap();

        let state = Arc::new(DaemonState::new(config).unwrap());
        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        // Start a session that triggers attention
        let script = "sleep 0.2; echo \"error: something went wrong\"; sleep 0.2";
        let start = DaemonRequest::Start {
            task_key: "Attention mapping".to_string(),
            task_id: Some("xyz.attention-test".to_string()),
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                script.to_string(),
            ]),
        };

        let response = handle_request(&state, start, &shutdown_tx).await;
        let session_id = match response {
            DaemonResponse::SessionStarted { session } => session.id,
            other => panic!("Expected SessionStarted, got {:?}", other),
        };

        // Wait for attention summary to be captured and mapped
        let attention = assert_eventually(
            "attention summary to be captured via SessionProvider",
            Duration::from_secs(3),
            Duration::from_millis(50),
            || {
                let state = Arc::clone(&state);
                async move {
                    match state.get_session_state(session_id) {
                        Some(snap) => match snap.last_attention {
                            Some(att)
                                if att.attention_type
                                    == rn_desktop_2_lib::context_resurrection::models::AttentionType::Error =>
                            {
                                Ok(att)
                            }
                            Some(att) => Err(format!(
                                "Attention type mismatch: {:?}",
                                att.attention_type
                            )),
                            None => Err("No attention summary yet".to_string()),
                        },
                        None => Err("Session not found".to_string()),
                    }
                }
            },
        )
        .await;

        // Verify attention summary was mapped correctly
        assert_eq!(
            attention.attention_type,
            rn_desktop_2_lib::context_resurrection::models::AttentionType::Error
        );
        assert!(
            attention.preview.to_lowercase().contains("error")
                || attention.preview.to_lowercase().contains("wrong"),
            "Preview should contain error message. Got: {}",
            attention.preview
        );

        // Verify triggered_at is a valid ISO8601 string
        assert!(
            chrono::DateTime::parse_from_rfc3339(&attention.triggered_at).is_ok(),
            "triggered_at should be valid ISO8601: {}",
            attention.triggered_at
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_capture_service_creates_snapshot_on_session_stop() {
        let (config, temp_dir) = test_config();
        let markdown_path = temp_dir.path().join("TODO.md");
        tokio::fs::write(&markdown_path, "# Tasks\n- [ ] Capture test task\n")
            .await
            .unwrap();

        // Create state with capture service
        let state = Arc::new(DaemonState::new(config).unwrap());
        state.init_capture_service().await;

        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        // Start a session with task_id (required for capture)
        let script = "echo 'capture-test-output'; sleep 0.1";
        let start = DaemonRequest::Start {
            task_key: "Capture test".to_string(),
            task_id: Some("cpt.capture-test".to_string()),
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                script.to_string(),
            ]),
        };

        let response = handle_request(&state, start, &shutdown_tx).await;
        let session_id = match response {
            DaemonResponse::SessionStarted { session } => session.id,
            other => panic!("Expected SessionStarted, got {:?}", other),
        };

        // Wait for session to stop (watcher runs every 5s, so allow up to 10s)
        assert_eventually(
            "session to stop",
            Duration::from_secs(10),
            Duration::from_millis(100),
            || {
                let state = Arc::clone(&state);
                async move {
                    let registry = state.registry.read().await;
                    match registry.get(session_id) {
                        Some(s) if s.status == SessionStatus::Stopped => Ok(()),
                        Some(s) => Err(format!("Session status is {:?}, not Stopped", s.status)),
                        None => Err("Session not found".to_string()),
                    }
                }
            },
        )
        .await;

        // Wait for capture to complete (background task needs time to write)
        use rn_desktop_2_lib::context_resurrection::store::SnapshotStore;
        let store = SnapshotStore::new(&temp_dir.path());

        let snapshots = assert_eventually(
            "snapshot to be captured for stopped session",
            Duration::from_secs(3),
            Duration::from_millis(50),
            || async {
                match store.list_snapshots(&markdown_path, "cpt.capture-test", None) {
                    Ok(snaps) if !snaps.is_empty() => Ok(snaps),
                    Ok(_) => Err("No snapshots found yet".to_string()),
                    Err(e) => Err(format!("Failed to list snapshots: {}", e)),
                }
            },
        )
        .await;

        // Verify snapshot has correct metadata
        let latest = &snapshots[0];
        assert_eq!(latest.task_id, "cpt.capture-test");
        assert_eq!(
            latest.capture_reason,
            rn_desktop_2_lib::context_resurrection::models::CaptureReason::SessionStopped
        );

        // Verify terminal context was captured
        let terminal = latest
            .terminal
            .as_ref()
            .expect("Terminal context should be captured");
        assert_eq!(terminal.session_id, session_id);
        assert_eq!(
            terminal.status,
            rn_desktop_2_lib::context_resurrection::models::SessionStatus::Stopped
        );

        // Verify tail contains our test output
        if let Some(ref tail) = terminal.tail_inline {
            assert!(
                tail.as_str().contains("capture-test-output"),
                "Tail should contain test output. Got: {}",
                tail
            );
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_capture_skipped_when_task_id_missing() {
        let (config, temp_dir) = test_config();
        let markdown_path = temp_dir.path().join("TODO.md");
        tokio::fs::write(&markdown_path, "# Tasks\n- [ ] No task ID\n")
            .await
            .unwrap();

        let state = Arc::new(DaemonState::new(config).unwrap());
        state.init_capture_service().await;

        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        // Start a session WITHOUT task_id
        let script = "echo 'no-capture-test'; sleep 0.1";
        let start = DaemonRequest::Start {
            task_key: "No task ID".to_string(),
            task_id: None, // No task_id -> capture should be skipped
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                script.to_string(),
            ]),
        };

        let response = handle_request(&state, start, &shutdown_tx).await;
        let session_id = match response {
            DaemonResponse::SessionStarted { session } => session.id,
            other => panic!("Expected SessionStarted, got {:?}", other),
        };

        // Wait for session to stop
        assert_eventually(
            "session to stop",
            Duration::from_secs(10),
            Duration::from_millis(100),
            || {
                let state = Arc::clone(&state);
                async move {
                    let registry = state.registry.read().await;
                    match registry.get(session_id) {
                        Some(s) if s.status == SessionStatus::Stopped => Ok(()),
                        Some(s) => Err(format!("Session status is {:?}, not Stopped", s.status)),
                        None => Err("Session not found".to_string()),
                    }
                }
            },
        )
        .await;

        // Give background tasks time to potentially write (they shouldn't)
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Verify NO snapshot was created (since task_id is None)
        // We can't list by task_id here since there's no stable ID, but we can verify
        // the project hash directory is either empty or doesn't exist
        use rn_desktop_2_lib::context_resurrection::store::SnapshotStore;

        // Check the project-hash directory
        let project_hash = SnapshotStore::project_hash(&markdown_path);
        let project_dir = temp_dir
            .path()
            .join("context-resurrection")
            .join("snapshots")
            .join(project_hash);

        // Directory might exist but should have no task subdirectories with snapshots
        if project_dir.exists() {
            let entries: Vec<_> = std::fs::read_dir(&project_dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .collect();

            // Should be empty or only contain .lock files, no actual snapshots
            for entry in entries {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                    panic!(
                        "Unexpected snapshot file found when task_id was None: {}",
                        path.display()
                    );
                }
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_cr_protocol_end_to_end() {
        let (config, temp_dir) = test_config();
        let state = Arc::new(DaemonState::new(config).unwrap());
        state.init_capture_service().await;
        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        // Create a markdown file
        let markdown_path = temp_dir.path().join("TODO.md");
        let initial_content = "# Tasks\n- [ ] Test task\n";
        tokio::fs::write(&markdown_path, initial_content)
            .await
            .unwrap();

        // Start a session to get a task_id
        let start_request = DaemonRequest::Start {
            task_key: "Test".to_string(),
            task_id: Some("test.test-task".to_string()),
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec!["echo".to_string(), "hello".to_string()]),
        };

        let start_response = handle_request(&state, start_request, &shutdown_tx).await;
        assert!(matches!(
            start_response,
            DaemonResponse::SessionStarted { .. }
        ));

        // Wait a bit for session to produce output
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Test CrCaptureNow: trigger a manual capture
        let capture_request = DaemonRequest::CrCaptureNow {
            project_path: markdown_path.to_string_lossy().to_string(),
            task_id: "test.test-task".to_string(),
            user_note: Some("Manual test capture".to_string()),
        };

        let capture_response = handle_request(&state, capture_request, &shutdown_tx).await;
        match &capture_response {
            DaemonResponse::CrSnapshot { snapshot } => {
                let snap = snapshot.as_ref().expect("Expected snapshot to be Some");
                assert_eq!(snap.task_id, "test.test-task");
                assert_eq!(snap.user_note.as_deref(), Some("Manual test capture"));
            }
            DaemonResponse::Error { code: _, message } => {
                panic!("CrCaptureNow failed: {}", message);
            }
            _ => panic!("Unexpected response: {:?}", capture_response),
        }

        // Test CrLatest: get the latest snapshot
        let latest_request = DaemonRequest::CrLatest {
            project_path: markdown_path.to_string_lossy().to_string(),
            task_id: Some("test.test-task".to_string()),
        };

        let latest_response = handle_request(&state, latest_request, &shutdown_tx).await;
        match &latest_response {
            DaemonResponse::CrSnapshot { snapshot } => {
                let snap = snapshot.as_ref().expect("Expected snapshot to be Some");
                assert_eq!(snap.task_id, "test.test-task");
            }
            DaemonResponse::Error { code: _, message } => {
                panic!("CrLatest failed: {}", message);
            }
            _ => panic!("Unexpected response: {:?}", latest_response),
        }

        // Test CrList: list all snapshots for the task
        let list_request = DaemonRequest::CrList {
            project_path: markdown_path.to_string_lossy().to_string(),
            task_id: "test.test-task".to_string(),
            limit: None,
        };

        let list_response = handle_request(&state, list_request, &shutdown_tx).await;
        match &list_response {
            DaemonResponse::CrSnapshots { snapshots } => {
                assert_eq!(snapshots.len(), 1);
                let snapshot = &snapshots[0];
                assert_eq!(snapshot.task_id, "test.test-task");
            }
            DaemonResponse::Error { code: _, message } => {
                panic!("CrList failed: {}", message);
            }
            _ => panic!("Unexpected response: {:?}", list_response),
        }

        // Test CrDeleteTask: delete all snapshots for the task
        let delete_task_request = DaemonRequest::CrDeleteTask {
            project_path: markdown_path.to_string_lossy().to_string(),
            task_id: "test.test-task".to_string(),
        };

        let delete_task_response = handle_request(&state, delete_task_request, &shutdown_tx).await;
        match &delete_task_response {
            DaemonResponse::CrDeleted { deleted_count } => {
                assert_eq!(*deleted_count, 1);
            }
            DaemonResponse::Error { code: _, message } => {
                panic!("CrDeleteTask failed: {}", message);
            }
            _ => panic!("Unexpected response: {:?}", delete_task_response),
        }

        // Verify snapshots are gone
        let list_after_delete = DaemonRequest::CrList {
            project_path: markdown_path.to_string_lossy().to_string(),
            task_id: "test.test-task".to_string(),
            limit: None,
        };

        let list_after_response = handle_request(&state, list_after_delete, &shutdown_tx).await;
        match &list_after_response {
            DaemonResponse::CrSnapshots { snapshots } => {
                assert_eq!(snapshots.len(), 0);
            }
            _ => panic!("Unexpected response after delete"),
        }
    }

    /// Regression test for subscribe-late race condition.
    ///
    /// Previously, if a client subscribed to session updates after the session
    /// had already started and produced output, they would miss early events.
    /// This test verifies that clients can reliably wait for session state
    /// changes using polling (via Continue/Status requests) instead of relying
    /// on being subscribed before events happen.
    #[tokio::test]
    async fn test_subscribe_late_race_regression() {
        let (config, temp_dir) = test_config();
        let markdown_path = temp_dir.path().join("TODO.md");
        tokio::fs::write(&markdown_path, "# Tasks\n- [ ] Subscribe late test\n")
            .await
            .unwrap();

        let state = Arc::new(DaemonState::new(config).unwrap());
        let (shutdown_tx, _) = tokio::sync::mpsc::channel::<()>(1);

        // Start a session that produces output quickly and exits
        let script = "echo 'early-output'; echo 'late-output'";
        let start = DaemonRequest::Start {
            task_key: "Subscribe late".to_string(),
            task_id: None,
            project_path: markdown_path.to_string_lossy().to_string(),
            shell: Some(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                script.to_string(),
            ]),
        };

        let response = handle_request(&state, start, &shutdown_tx).await;
        let session_id = match response {
            DaemonResponse::SessionStarted { session } => session.id,
            other => panic!("Expected SessionStarted, got {:?}", other),
        };

        // Simulate a "late subscriber" by not subscribing to the broadcast channel
        // immediately. Instead, poll via Continue requests to ensure we can still
        // observe session state reliably even if we missed the broadcast events.

        // Wait for session to produce output (may happen quickly)
        assert_eventually_bool(
            "session to produce output (early-output visible)",
            Duration::from_secs(2),
            Duration::from_millis(50),
            || {
                let state = Arc::clone(&state);
                let shutdown_tx = shutdown_tx.clone();
                async move {
                    let continue_req = DaemonRequest::Continue {
                        session_id,
                        tail_bytes: Some(4096),
                    };
                    match handle_request(&state, continue_req, &shutdown_tx).await {
                        DaemonResponse::SessionContinued { tail, .. } => {
                            if let Some(data) = tail {
                                let text = String::from_utf8_lossy(&data);
                                text.contains("early-output")
                            } else {
                                false
                            }
                        }
                        _ => false,
                    }
                }
            },
        )
        .await;

        // Wait for session to stop (watcher polls every 5s, so allow up to 10s)
        assert_eventually(
            "session to stop after producing all output",
            Duration::from_secs(10),
            Duration::from_millis(100),
            || {
                let state = Arc::clone(&state);
                async move {
                    let registry = state.registry.read().await;
                    match registry.get(session_id) {
                        Some(s) if s.status == SessionStatus::Stopped => Ok(()),
                        Some(s) => Err(format!("Session status is {:?}, not Stopped", s.status)),
                        None => Err("Session not found".to_string()),
                    }
                }
            },
        )
        .await;

        // Final verification: tail should contain both early and late output
        // This proves that even though we "subscribed late" (didn't listen to broadcasts),
        // we can still retrieve the full session state via polling.
        let final_continue = DaemonRequest::Continue {
            session_id,
            tail_bytes: Some(4096),
        };
        match handle_request(&state, final_continue, &shutdown_tx).await {
            DaemonResponse::SessionContinued { tail, session } => {
                assert_eq!(session.status, SessionStatus::Stopped);
                if let Some(data) = tail {
                    let text = String::from_utf8_lossy(&data);
                    assert!(
                        text.contains("early-output"),
                        "Tail should contain early output. Got: {}",
                        text
                    );
                    assert!(
                        text.contains("late-output"),
                        "Tail should contain late output. Got: {}",
                        text
                    );
                }
            }
            other => panic!("Expected SessionContinued, got {:?}", other),
        }
    }
}
