// PTY runtime management for terminal sessions
//
// Wraps portable-pty to provide async methods for session lifecycle:
// - Spawning PTY child processes
// - Reading output
// - Sending input
// - Tracking idle/activity status
// - Graceful and forced termination

use crate::session::protocol::{SessionId, SessionStatus};
use anyhow::{Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, oneshot};

/// Default PTY size (columns x rows)
const DEFAULT_COLS: u16 = 80;
const DEFAULT_ROWS: u16 = 24;

/// Duration of inactivity before transitioning to Waiting status
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Output buffer size for the ring buffer
const OUTPUT_BUFFER_SIZE: usize = 64 * 1024; // 64KB

/// Maximum number of display characters exposed via RIGHT_NOW_TASK_DISPLAY
const TASK_DISPLAY_MAX_CHARS: usize = 160;

/// Events emitted by the PTY runtime
#[derive(Debug, Clone)]
pub enum PtyEvent {
    /// PTY produced output
    Output(Vec<u8>),
    /// PTY became active (after being idle)
    Active,
    /// PTY became idle (no output for IDLE_TIMEOUT)
    Idle,
    /// PTY process exited
    Exited { exit_code: Option<i32> },
}

fn sanitize_task_display(task_key: &str) -> String {
    let mut result = String::new();
    let mut chars_added = 0;
    let mut prev_was_space = false;

    for ch in task_key.chars() {
        if chars_added >= TASK_DISPLAY_MAX_CHARS {
            break;
        }
        let sanitized = match ch {
            '\n' | '\r' | '\t' => ' ',
            c if c.is_control() => ' ',
            other => other,
        };

        if sanitized.is_whitespace() {
            if prev_was_space || result.is_empty() {
                continue;
            }
            result.push(' ');
            prev_was_space = true;
            chars_added += 1;
        } else {
            result.push(sanitized);
            prev_was_space = false;
            chars_added += 1;
        }
    }

    result.trim().to_string()
}

/// Handle for sending input to and controlling a PTY session
pub struct PtyRuntime {
    session_id: SessionId,
    /// Channel for sending input to the PTY writer task
    input_tx: mpsc::Sender<Vec<u8>>,
    /// Broadcast channel for PTY events
    event_tx: broadcast::Sender<PtyEvent>,
    /// Handle to the PTY master for resizing
    master: Arc<StdMutex<Box<dyn MasterPty + Send>>>,
    /// Shutdown signal sender
    shutdown_tx: Option<oneshot::Sender<()>>,
    /// Flag indicating if the session is still alive
    alive: Arc<AtomicBool>,
    /// PID of the child process (if available)
    child_pid: Option<u32>,
    /// Ring buffer of recent output for resumable reads
    output_buffer: Arc<StdMutex<RingBuffer>>,
    /// Last activity timestamp
    last_activity: Arc<StdMutex<Instant>>,
    /// Cached exit code once the PTY terminates
    exit_code: Arc<StdMutex<Option<i32>>>,
}

/// Simple ring buffer for storing recent PTY output
struct RingBuffer {
    data: Vec<u8>,
    capacity: usize,
}

impl RingBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            capacity,
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        // If adding would exceed capacity, remove oldest data
        let total_len = self.data.len() + bytes.len();
        if total_len > self.capacity {
            let to_remove = total_len - self.capacity;
            if to_remove >= self.data.len() {
                self.data.clear();
            } else {
                self.data.drain(..to_remove);
            }
        }
        self.data.extend_from_slice(bytes);
    }

    fn get_tail(&self, max_bytes: usize) -> Vec<u8> {
        if self.data.len() <= max_bytes {
            self.data.clone()
        } else {
            self.data[self.data.len() - max_bytes..].to_vec()
        }
    }
}

impl PtyRuntime {
    /// Spawn a new PTY session with the given shell command
    ///
    /// If `shell` is None, uses the default shell from $SHELL or /bin/sh
    /// Sets RIGHT_NOW_SESSION_ID, RIGHT_NOW_TASK_KEY, and RIGHT_NOW_PROJECT env vars
    pub fn spawn(
        session_id: SessionId,
        shell: Option<Vec<String>>,
        task_key: &str,
        project_path: &str,
    ) -> Result<Self> {
        let pty_system = native_pty_system();

        // Create PTY pair
        let pair = pty_system
            .openpty(PtySize {
                rows: DEFAULT_ROWS,
                cols: DEFAULT_COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to open PTY")?;

        let portable_pty::PtyPair { master, slave } = pair;

        // Build command with environment variables for shell integration
        let mut cmd = build_shell_command(shell);
        cmd.env("RIGHT_NOW_SESSION_ID", session_id.to_string());
        cmd.env("RIGHT_NOW_TASK_KEY", task_key);
        cmd.env("RIGHT_NOW_PROJECT", project_path);
        cmd.env("RIGHT_NOW_TASK_DISPLAY", sanitize_task_display(task_key));

        // Spawn child process
        let child = slave
            .spawn_command(cmd)
            .context("Failed to spawn shell process")?;

        let child_pid = child.process_id();

        // Set up communication channels
        let (input_tx, input_rx) = mpsc::channel::<Vec<u8>>(100);
        let (event_tx, _) = broadcast::channel::<PtyEvent>(200);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let alive = Arc::new(AtomicBool::new(true));
        let output_buffer = Arc::new(StdMutex::new(RingBuffer::new(OUTPUT_BUFFER_SIZE)));
        let last_activity = Arc::new(StdMutex::new(Instant::now()));
        let exit_code = Arc::new(StdMutex::new(None));

        let reader = master
            .try_clone_reader()
            .context("Failed to clone PTY reader")?;
        let writer = master.take_writer().context("Failed to take PTY writer")?;
        let master_handle = Arc::new(StdMutex::new(master));

        // Spawn background tasks for I/O
        Self::spawn_reader_task(
            reader,
            event_tx.clone(),
            Arc::clone(&alive),
            Arc::clone(&output_buffer),
            Arc::clone(&last_activity),
        );

        Self::spawn_writer_task(writer, input_rx, Arc::clone(&alive));

        Self::spawn_wait_task(
            child,
            event_tx.clone(),
            shutdown_rx,
            Arc::clone(&alive),
            Arc::clone(&exit_code),
        );

        Ok(Self {
            session_id,
            input_tx,
            event_tx,
            master: master_handle,
            shutdown_tx: Some(shutdown_tx),
            alive,
            child_pid,
            output_buffer,
            last_activity,
            exit_code,
        })
    }

    /// Spawn the reader task that reads PTY output
    ///
    /// Note: Events are sent via broadcast and dropped if no listeners are active.
    /// The ring buffer and activity timestamp are always updated regardless of
    /// event delivery, preventing deadlock when nothing drains the channel.
    fn spawn_reader_task(
        mut reader: Box<dyn Read + Send>,
        event_tx: broadcast::Sender<PtyEvent>,
        alive: Arc<AtomicBool>,
        output_buffer: Arc<StdMutex<RingBuffer>>,
        last_activity: Arc<StdMutex<Instant>>,
    ) {
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut was_idle = false;

            loop {
                if !alive.load(Ordering::SeqCst) {
                    break;
                }

                match reader.read(&mut buf) {
                    Ok(0) => {
                        // EOF - PTY closed
                        break;
                    }
                    Ok(n) => {
                        let data = buf[..n].to_vec();

                        // Update output buffer (always succeeds)
                        {
                            let mut buffer = output_buffer.lock().unwrap();
                            buffer.push(&data);
                        }

                        // Update activity timestamp (always succeeds)
                        {
                            let mut activity = last_activity.lock().unwrap();
                            *activity = Instant::now();
                        }

                        // Send activity event if we were idle (non-blocking)
                        if was_idle {
                            was_idle = false;
                            let _ = event_tx.send(PtyEvent::Active);
                        }

                        // Send output event (non-blocking - drop if channel full)
                        // The ring buffer already has the data, so dropping events is safe
                        let _ = event_tx.send(PtyEvent::Output(data));
                    }
                    Err(e) => {
                        eprintln!("PTY read error: {}", e);
                        break;
                    }
                }
            }
        });
    }

    /// Spawn the writer task that sends input to PTY
    fn spawn_writer_task(
        mut writer: Box<dyn Write + Send>,
        mut input_rx: mpsc::Receiver<Vec<u8>>,
        alive: Arc<AtomicBool>,
    ) {
        std::thread::spawn(move || {
            while let Some(data) = input_rx.blocking_recv() {
                if !alive.load(Ordering::SeqCst) {
                    break;
                }
                if writer.write_all(&data).is_err() {
                    break;
                }
                let _ = writer.flush();
            }
        });
    }

    /// Spawn the task that waits for child process to exit
    ///
    /// Note: Exit events are broadcast and may be dropped if no listeners exist.
    /// The `alive` flag is always set to false regardless of event delivery.
    fn spawn_wait_task(
        mut child: Box<dyn portable_pty::Child + Send>,
        event_tx: broadcast::Sender<PtyEvent>,
        mut shutdown_rx: oneshot::Receiver<()>,
        alive: Arc<AtomicBool>,
        exit_code: Arc<StdMutex<Option<i32>>>,
    ) {
        std::thread::spawn(move || {
            loop {
                // Check for shutdown signal
                match shutdown_rx.try_recv() {
                    Ok(_) | Err(oneshot::error::TryRecvError::Closed) => {
                        // Shutdown requested, kill child
                        let _ = child.kill();
                        alive.store(false, Ordering::SeqCst);
                        {
                            let mut code = exit_code.lock().unwrap();
                            *code = None;
                        }
                        break;
                    }
                    Err(oneshot::error::TryRecvError::Empty) => {}
                }

                // Try to wait with timeout
                match child.try_wait() {
                    Ok(Some(status)) => {
                        // Child exited - set alive flag first (always succeeds)
                        alive.store(false, Ordering::SeqCst);
                        let code_value = Some(status.exit_code() as i32);
                        {
                            let mut code = exit_code.lock().unwrap();
                            *code = code_value;
                        }
                        // Non-blocking send - the alive flag is the source of truth
                        let _ = event_tx.send(PtyEvent::Exited {
                            exit_code: code_value,
                        });
                        break;
                    }
                    Ok(None) => {
                        // Still running, wait a bit
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => {
                        eprintln!("Error waiting for child: {}", e);
                        alive.store(false, Ordering::SeqCst);
                        {
                            let mut code = exit_code.lock().unwrap();
                            *code = None;
                        }
                        let _ = event_tx.send(PtyEvent::Exited { exit_code: None });
                        break;
                    }
                }
            }
        });
    }

    /// Get the session ID
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Get the child process PID
    pub fn pid(&self) -> Option<u32> {
        self.child_pid
    }

    /// Check if the session is still alive
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Send input to the PTY
    pub async fn send_input(&self, data: Vec<u8>) -> Result<()> {
        self.input_tx
            .send(data)
            .await
            .context("Failed to send input to PTY")
    }

    /// Resize the PTY window
    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        let master = self.master.lock().unwrap();
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("Failed to resize PTY")
    }

    /// Subscribe to PTY events (output, idle/active, exit)
    pub fn subscribe_events(&self) -> broadcast::Receiver<PtyEvent> {
        self.event_tx.subscribe()
    }

    /// Clone of the input sender for streaming input
    pub fn input_sender(&self) -> mpsc::Sender<Vec<u8>> {
        self.input_tx.clone()
    }

    /// Get the last N bytes of output from the ring buffer
    pub async fn get_recent_output(&self, max_bytes: usize) -> Vec<u8> {
        self.get_recent_output_blocking(max_bytes)
    }

    /// Get recent output without requiring async context (used by daemon request handlers)
    pub fn get_recent_output_blocking(&self, max_bytes: usize) -> Vec<u8> {
        self.output_buffer.lock().unwrap().get_tail(max_bytes)
    }

    /// Get the cached exit code if the PTY has terminated
    pub fn exit_code(&self) -> Option<i32> {
        *self.exit_code.lock().unwrap()
    }

    /// Check if the PTY has been idle for longer than the threshold
    pub fn is_idle(&self) -> bool {
        let last = *self.last_activity.lock().unwrap();
        last.elapsed() > IDLE_TIMEOUT
    }

    /// Get the current inferred status based on activity
    pub fn inferred_status(&self) -> SessionStatus {
        if !self.is_alive() {
            SessionStatus::Stopped
        } else if self.is_idle() {
            SessionStatus::Waiting
        } else {
            SessionStatus::Running
        }
    }

    /// Stop the PTY session
    pub fn stop(&mut self) {
        self.alive.store(false, Ordering::SeqCst);
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for PtyRuntime {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Build a CommandBuilder from shell arguments or default
fn build_shell_command(shell: Option<Vec<String>>) -> CommandBuilder {
    match shell {
        Some(args) if !args.is_empty() => {
            let mut cmd = CommandBuilder::new(&args[0]);
            for arg in args.iter().skip(1) {
                cmd.arg(arg);
            }
            cmd
        }
        _ => {
            // Use default shell
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            let mut cmd = CommandBuilder::new(&shell);
            cmd.arg("-l"); // Login shell
            cmd
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ring_buffer() {
        let mut buf = RingBuffer::new(10);
        buf.push(b"hello");
        assert_eq!(buf.get_tail(100), b"hello");

        buf.push(b"world!");
        // Total is 11, capacity is 10, so oldest byte dropped
        assert_eq!(buf.data.len(), 10);

        let tail = buf.get_tail(5);
        assert_eq!(tail.len(), 5);
    }

    #[tokio::test]
    async fn test_spawn_echo() {
        // Spawn a simple echo command
        let shell = vec!["echo".to_string(), "hello".to_string()];
        let runtime = PtyRuntime::spawn(1, Some(shell), "Test task", "/tmp/TODO.md")
            .expect("Failed to spawn");

        // Wait for output or exit
        let mut events = runtime.subscribe_events();
        let mut got_exit = false;

        for _ in 0..50 {
            tokio::select! {
                event = events.recv() => {
                    match event {
                        Ok(PtyEvent::Output(data)) => {
                            let s = String::from_utf8_lossy(&data);
                            if s.contains("hello") {
                                // Output observed
                            }
                        }
                        Ok(PtyEvent::Exited { exit_code }) => {
                            assert_eq!(exit_code, Some(0));
                            got_exit = true;
                            break;
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            }
        }

        assert!(got_exit, "Process should have exited");
    }

    #[tokio::test]
    async fn test_pty_environment_variables() {
        // Use unique values that couldn't accidentally exist in the environment
        let unique_task = format!("UniqueTask_{}", std::process::id());
        let unique_project = format!("/tmp/unique_project_{}/TODO.md", std::process::id());

        // Spawn a shell script that echoes ALL the environment variables we set
        let shell = vec![
            "sh".to_string(),
            "-c".to_string(),
            "echo SID=$RIGHT_NOW_SESSION_ID TK=$RIGHT_NOW_TASK_KEY PROJ=$RIGHT_NOW_PROJECT"
                .to_string(),
        ];
        let runtime = PtyRuntime::spawn(99999, Some(shell), &unique_task, &unique_project)
            .expect("Failed to spawn");

        // Wait for output
        let mut events = runtime.subscribe_events();
        let mut output_lines = String::new();

        for _ in 0..50 {
            tokio::select! {
                event = events.recv() => {
                    match event {
                        Ok(PtyEvent::Output(data)) => {
                            output_lines.push_str(&String::from_utf8_lossy(&data));
                        }
                        Ok(PtyEvent::Exited { .. }) => break,
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            }
        }

        // Verify all three environment variables with our unique values
        assert!(
            output_lines.contains("SID=99999"),
            "RIGHT_NOW_SESSION_ID not found in output: {}",
            output_lines
        );
        assert!(
            output_lines.contains(&format!("TK={}", unique_task)),
            "RIGHT_NOW_TASK_KEY not found in output: {}",
            output_lines
        );
        assert!(
            output_lines.contains(&format!("PROJ={}", unique_project)),
            "RIGHT_NOW_PROJECT not found in output: {}",
            output_lines
        );
    }

    #[test]
    fn sanitize_removes_control_characters_and_newlines() {
        let input = "Line1\nLine2\t\x07";
        assert_eq!(super::sanitize_task_display(input), "Line1 Line2");
    }

    #[test]
    fn sanitize_truncates_long_values() {
        let long = "x".repeat(super::TASK_DISPLAY_MAX_CHARS + 10);
        let sanitized = super::sanitize_task_display(&long);
        assert_eq!(sanitized.len(), super::TASK_DISPLAY_MAX_CHARS);
    }

    #[test]
    fn sanitize_preserves_literals_for_adversarial_names() {
        let input = "$(whoami); rm -rf /";
        assert_eq!(super::sanitize_task_display(input), "$(whoami); rm -rf /");
    }

    #[test]
    fn sanitize_collapses_repeated_spaces() {
        let input = " Hello   World ";
        assert_eq!(super::sanitize_task_display(input), "Hello World");
    }
}
