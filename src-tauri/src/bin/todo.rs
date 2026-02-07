// todo: CLI for interacting with right-now-daemon
//
// Commands:
//   todo start <task words> [--project <path>] [--cmd "<shell command>"] [--background]
//   todo continue <session-id> [--attach]
//   todo list [--project <path>]
//   todo stop <session-id>
//   todo shell-integration [--install | --uninstall] [--shell <zsh|bash|fish>]

use anyhow::{anyhow, Context, Result};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
#[cfg(unix)]
use libc::{fcntl, F_GETFL, F_SETFL, O_NONBLOCK};
use rn_desktop_2_lib::session::{
    config::Config,
    protocol::{
        deserialize_message, serialize_message, DaemonRequest, DaemonResponse, SessionStatus,
    },
    shell_integration::{self, ShellType},
};
#[cfg(unix)]
use signal_hook::{
    consts::signal::SIGWINCH,
    iterator::{Handle as SignalHandle, Signals},
};
use std::{
    borrow::Cow,
    env,
    io::{self, BufRead, BufReader, Read, Write},
    net::Shutdown,
    os::fd::{AsRawFd, RawFd},
    os::unix::net::UnixStream,
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

const DEFAULT_TAIL_BYTES: usize = 4 * 1024;
const DETACH_BYTE: u8 = 0x1c; // Ctrl-\
const INPUT_IDLE_SLEEP_MS: u64 = 10;

fn print_help() {
    println!(
        r#"todo - CLI for managing TODO terminal sessions

USAGE:
    todo <COMMAND> [OPTIONS]

COMMANDS:
    start <task>           Start a new session and enter it immediately
    continue <id>          Show recent output from a session
    list                   List all sessions
    stop <id>              Stop a running session
    status <id>            Get status of a specific session
    shell-integration      Install/uninstall shell prompt integration
    help                   Show this help message

OPTIONS:
    --project <path>   Path to TODO.md file (defaults to current directory)
    --cmd <command>    Shell command to run (for start)
    --background, -b   Start session in background without attaching
    --attach           Attach to PTY output for 'continue'
    --json             Output in JSON format

EXAMPLES:
    todo start "build pipeline"              # Start and enter session
    todo start "run tests" --background      # Start in background
    todo start "run tests" --cmd "npm test"  # Start with specific command
    todo continue 42 --attach                # Attach to existing session
    todo shell-integration --install         # Install prompt integration
    todo list --project ~/projects/myapp/TODO.md
    todo stop 42

DETACH:
    Press Ctrl-\ to detach from an attached session

DEEP LINKS:
    Open sessions in the UI using: open todos://session/<id>
"#
    );
}

fn find_project_file() -> Option<PathBuf> {
    // Look for TODO.md in current directory or parent directories
    let mut current = env::current_dir().ok()?;

    loop {
        let candidate = current.join("TODO.md");
        if candidate.exists() {
            return Some(candidate);
        }

        // Also check for todo.md (lowercase)
        let candidate_lower = current.join("todo.md");
        if candidate_lower.exists() {
            return Some(candidate_lower);
        }

        if !current.pop() {
            break;
        }
    }

    None
}

fn connect_to_daemon(config: &Config) -> Result<UnixStream> {
    // Try to connect
    match UnixStream::connect(&config.socket_path) {
        Ok(stream) => {
            stream.set_read_timeout(Some(Duration::from_secs(30)))?;
            stream.set_write_timeout(Some(Duration::from_secs(5)))?;
            Ok(stream)
        }
        Err(_) => {
            // Daemon not running, try to start it
            println!("Daemon not running, attempting to start...");

            // Find the daemon binary using several strategies:
            // 1. Next to current_exe() (typical for bundled releases)
            // 2. CliPaths from app-written config
            // 3. Platform-specific fallback locations
            let daemon_path = rn_desktop_2_lib::cli_paths::resolve_daemon_path().ok_or_else(|| {
                anyhow!(
                    "Could not find right-now-daemon binary. Please ensure Right Now is installed correctly."
                )
            })?;

            // Start the daemon as a detached background process
            Command::new(&daemon_path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .with_context(|| format!("Failed to start daemon at {}", daemon_path.display()))?;

            // Wait for daemon to start (check for socket)
            for i in 0..50 {
                std::thread::sleep(Duration::from_millis(100));
                if let Ok(stream) = UnixStream::connect(&config.socket_path) {
                    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
                    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
                    println!("Daemon started successfully");
                    return Ok(stream);
                }
                if i == 49 {
                    anyhow::bail!(
                        "Timed out waiting for daemon to start. Socket not found at: {}",
                        config.socket_path.display()
                    );
                }
            }
            anyhow::bail!("Failed to connect after starting daemon");
        }
    }
}

fn send_request(stream: &mut UnixStream, request: &DaemonRequest) -> Result<DaemonResponse> {
    let bytes = serialize_message(request)?;
    stream.write_all(&bytes)?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    deserialize_message(line.as_bytes()).context("Failed to parse daemon response")
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_help();
        return Ok(());
    }

    let command = &args[1];

    // Parse global options
    let mut project_path: Option<String> = None;
    let mut json_output = false;
    let mut shell_cmd: Option<String> = None;
    let mut tail_bytes: Option<usize> = None;
    let mut attach_mode = false;
    let mut background_mode = false;
    let mut install_mode = false;
    let mut uninstall_mode = false;
    let mut shell_type_arg: Option<String> = None;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--project" | "-p" => {
                i += 1;
                if i < args.len() {
                    project_path = Some(args[i].clone());
                }
            }
            "--json" => {
                json_output = true;
            }
            "--cmd" | "-c" => {
                i += 1;
                if i < args.len() {
                    shell_cmd = Some(args[i].clone());
                }
            }
            "--tail-bytes" => {
                i += 1;
                if i < args.len() {
                    match args[i].parse::<usize>() {
                        Ok(v) => tail_bytes = Some(v),
                        Err(_) => {
                            eprintln!("--tail-bytes must be a positive integer");
                            std::process::exit(1);
                        }
                    }
                }
            }
            "--attach" => {
                attach_mode = true;
            }
            "--background" | "-b" => {
                background_mode = true;
            }
            "--install" => {
                install_mode = true;
            }
            "--uninstall" => {
                uninstall_mode = true;
            }
            "--shell" => {
                i += 1;
                if i < args.len() {
                    shell_type_arg = Some(args[i].clone());
                }
            }
            _ => {}
        }
        i += 1;
    }

    let config = Config::from_env();

    // Get project path from option, env, auto-detect, or current marker
    let project = project_path
        .map(PathBuf::from)
        .or_else(|| env::var("TODO_PROJECT").ok().map(PathBuf::from))
        .or_else(find_project_file)
        .or_else(|| config.read_current_project());

    if attach_mode && command.as_str() != "continue" {
        eprintln!("--attach is only supported with the 'continue' command");
        std::process::exit(1);
    }

    match command.as_str() {
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }

        "start" => {
            if args.len() < 3 {
                eprintln!("Usage: todo start <task name>");
                std::process::exit(1);
            }

            let task_key = args[2].clone();
            let project_path = project
                .ok_or_else(|| anyhow::anyhow!("No TODO.md found. Use --project to specify."))?;

            // Parse TODO.md to extract task_id if present
            let task_id = {
                use rn_desktop_2_lib::session::markdown::{find_task_by_key, parse_body};
                match std::fs::read_to_string(&project_path) {
                    Ok(content) => {
                        let blocks = parse_body(&content);
                        find_task_by_key(&blocks, &task_key).and_then(|task| task.task_id.clone())
                    }
                    Err(_) => None, // File read error; daemon will catch it
                }
            };

            let mut stream = connect_to_daemon(&config)?;

            let shell = shell_cmd.map(|cmd| {
                let shell = Config::default_shell();
                vec![shell[0].clone(), "-c".to_string(), cmd]
            });

            let request = DaemonRequest::Start {
                task_key: task_key.clone(),
                task_id,
                project_path: project_path.to_string_lossy().to_string(),
                shell,
            };

            let response = send_request(&mut stream, &request)?;

            if json_output {
                println!("{}", serde_json::to_string_pretty(&response)?);
                return Ok(());
            }

            match response {
                DaemonResponse::SessionStarted { session } => {
                    println!(
                        "Started session {} for '{}'\n  Deep link: {}",
                        session.id,
                        session.task_key,
                        session.deep_link()
                    );

                    // Unless --background, immediately attach to the session
                    if !background_mode {
                        // Send attach request
                        let attach_request = DaemonRequest::Attach {
                            session_id: session.id,
                            tail_bytes: Some(tail_bytes.unwrap_or(DEFAULT_TAIL_BYTES)),
                        };
                        let attach_response = send_request(&mut stream, &attach_request)?;

                        match attach_response {
                            DaemonResponse::AttachReady {
                                session: attached_session,
                                tail,
                                socket_path,
                            } => {
                                run_attach_session(
                                    &attached_session,
                                    tail.as_deref(),
                                    &socket_path,
                                    config.clone(),
                                )?;
                            }
                            DaemonResponse::Error { code: _, message } => {
                                eprintln!("Error attaching: {}", message);
                                std::process::exit(1);
                            }
                            other => {
                                eprintln!("Unexpected attach response: {:?}", other);
                                std::process::exit(1);
                            }
                        }
                    }
                }
                DaemonResponse::Error { code: _, message } => {
                    eprintln!("Error: {}", message);
                    std::process::exit(1);
                }
                _ => {
                    eprintln!("Unexpected response");
                    std::process::exit(1);
                }
            }
            Ok(())
        }

        "continue" => {
            if args.len() < 3 {
                eprintln!("Usage: todo continue <session-id>");
                std::process::exit(1);
            }

            let session_id: u64 = args[2].parse().context("Session ID must be a number")?;

            let mut stream = connect_to_daemon(&config)?;

            if attach_mode {
                let request = DaemonRequest::Attach {
                    session_id,
                    tail_bytes: Some(tail_bytes.unwrap_or(DEFAULT_TAIL_BYTES)),
                };
                let response = send_request(&mut stream, &request)?;

                if json_output {
                    println!("{}", serde_json::to_string_pretty(&response)?);
                    return Ok(());
                }

                match response {
                    DaemonResponse::AttachReady {
                        session,
                        tail,
                        socket_path,
                    } => {
                        if session.status == SessionStatus::Stopped {
                            eprintln!(
                                "Session {} is already stopped; nothing to attach to.",
                                session.id
                            );
                            std::process::exit(1);
                        }

                        // Print metadata, but replay buffer directly during attach.
                        print_session_summary(&session, None)?;
                        run_attach_session(
                            &session,
                            tail.as_deref(),
                            &socket_path,
                            config.clone(),
                        )?;
                    }
                    DaemonResponse::Error { code: _, message } => {
                        eprintln!("Error: {}", message);
                        std::process::exit(1);
                    }
                    other => {
                        eprintln!("Unexpected response: {:?}", other);
                        std::process::exit(1);
                    }
                }
                Ok(())
            } else {
                let request = DaemonRequest::Continue {
                    session_id,
                    tail_bytes: Some(tail_bytes.unwrap_or(DEFAULT_TAIL_BYTES)),
                };
                let response = send_request(&mut stream, &request)?;

                if json_output {
                    println!("{}", serde_json::to_string_pretty(&response)?);
                } else {
                    match response {
                        DaemonResponse::SessionContinued { session, tail } => {
                            print_session_summary(&session, tail.as_deref())?;
                        }
                        DaemonResponse::Error { code: _, message } => {
                            eprintln!("Error: {}", message);
                            std::process::exit(1);
                        }
                        _ => {
                            eprintln!("Unexpected response");
                            std::process::exit(1);
                        }
                    }
                }
                Ok(())
            }
        }

        "list" => {
            let mut stream = connect_to_daemon(&config)?;

            let request = DaemonRequest::List {
                project_path: project.map(|p| p.to_string_lossy().to_string()),
            };

            let response = send_request(&mut stream, &request)?;

            if json_output {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                match response {
                    DaemonResponse::SessionList { sessions } => {
                        if sessions.is_empty() {
                            println!("No active sessions");
                        } else {
                            for session in sessions {
                                println!(
                                    "[{}] {} — {} — {}",
                                    session.id,
                                    session.task_key,
                                    session.status,
                                    session.deep_link()
                                );
                            }
                        }
                    }
                    DaemonResponse::Error { code: _, message } => {
                        eprintln!("Error: {}", message);
                        std::process::exit(1);
                    }
                    _ => {
                        eprintln!("Unexpected response");
                        std::process::exit(1);
                    }
                }
            }
            Ok(())
        }

        "stop" => {
            if args.len() < 3 {
                eprintln!("Usage: todo stop <session-id>");
                std::process::exit(1);
            }

            let session_id: u64 = args[2].parse().context("Session ID must be a number")?;

            let mut stream = connect_to_daemon(&config)?;
            let request = DaemonRequest::Stop { session_id };
            let response = send_request(&mut stream, &request)?;

            if json_output {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                match response {
                    DaemonResponse::SessionStopped { session } => {
                        println!("Stopped session {} for '{}'", session.id, session.task_key);
                    }
                    DaemonResponse::Error { code: _, message } => {
                        eprintln!("Error: {}", message);
                        std::process::exit(1);
                    }
                    _ => {
                        eprintln!("Unexpected response");
                        std::process::exit(1);
                    }
                }
            }
            Ok(())
        }

        "status" => {
            if args.len() < 3 {
                eprintln!("Usage: todo status <session-id>");
                std::process::exit(1);
            }

            let session_id: u64 = args[2].parse().context("Session ID must be a number")?;

            let mut stream = connect_to_daemon(&config)?;
            let request = DaemonRequest::Status { session_id };
            let response = send_request(&mut stream, &request)?;

            if json_output {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                match response {
                    DaemonResponse::SessionStatus { session } => {
                        println!("Session {} — {}", session.id, session.task_key);
                        println!("  Status: {}", session.status);
                        println!("  Project: {}", session.project_path);
                        println!("  Created: {}", session.created_at);
                        println!("  Deep link: {}", session.deep_link());
                    }
                    DaemonResponse::Error { code: _, message } => {
                        eprintln!("Error: {}", message);
                        std::process::exit(1);
                    }
                    _ => {
                        eprintln!("Unexpected response");
                        std::process::exit(1);
                    }
                }
            }
            Ok(())
        }

        "shell-integration" => {
            // Determine shell type
            let shell_type = match shell_type_arg {
                Some(ref s) => s.parse::<ShellType>()?,
                None => ShellType::detect().ok_or_else(|| {
                    anyhow!(
                        "Could not detect shell type from $SHELL. \
                         Please specify with --shell <zsh|bash|fish>"
                    )
                })?,
            };

            let rc_path = shell_type.rc_file_path()?;

            if install_mode && uninstall_mode {
                eprintln!("Cannot use both --install and --uninstall");
                std::process::exit(1);
            }

            if uninstall_mode {
                let removed = shell_integration::uninstall(&rc_path)?;
                if removed {
                    println!("Removed Right Now integration from {}", rc_path.display());
                    println!("Restart your shell or run: source {}", rc_path.display());
                } else {
                    println!(
                        "Right Now integration was not installed in {}",
                        rc_path.display()
                    );
                }
            } else if install_mode {
                let installed_path = shell_integration::install(shell_type, Some(rc_path.clone()))?;
                println!(
                    "Installed Right Now integration to {}",
                    installed_path.display()
                );
                println!();
                println!(
                    "Restart your shell or run: source {}",
                    installed_path.display()
                );
                println!();
                println!("When working in a Right Now session, your prompt will show:");
                println!("  [#42: Task name] > ");
            } else {
                // Show status
                let installed = shell_integration::is_installed(&rc_path)?;
                println!("Shell: {}", shell_type);
                println!("RC file: {}", rc_path.display());
                println!(
                    "Status: {}",
                    if installed {
                        "installed"
                    } else {
                        "not installed"
                    }
                );
                println!();
                if !installed {
                    println!("To install: todo shell-integration --install");
                } else {
                    println!("To uninstall: todo shell-integration --uninstall");
                }
            }
            Ok(())
        }

        _ => {
            eprintln!("Unknown command: {}", command);
            print_help();
            std::process::exit(1);
        }
    }
}

fn print_session_summary(
    session: &rn_desktop_2_lib::session::protocol::Session,
    tail_data: Option<&[u8]>,
) -> Result<()> {
    println!(
        "Session {} — {} ({})",
        session.id, session.task_key, session.status
    );
    println!("Project: {}", session.project_path);
    println!("Created: {}", session.created_at);
    println!("Updated: {}", session.updated_at);
    if let Some(code) = session.exit_code {
        println!("Exit code: {}", code);
    }
    println!("Deep link: {}", session.deep_link());
    if let Some(attention) = &session.last_attention {
        println!(
            "Attention: {} ({}) at {}",
            attention.attention_type, attention.profile, attention.triggered_at
        );
        println!("  {}", attention.preview);
    }

    match (session.status, tail_data) {
        (_, Some(data)) if !data.is_empty() => {
            let display = sanitize_tail_bytes(data);
            println!("\nRecent output ({} bytes):\n{}", data.len(), display);
        }
        (SessionStatus::Stopped, _) => {
            println!("\nSession is stopped; no PTY output available.");
        }
        _ => {
            println!("\n[No recent output]");
        }
    }

    Ok(())
}

fn sanitize_tail_bytes(data: &[u8]) -> Cow<'_, str> {
    match std::str::from_utf8(data) {
        Ok(text) => Cow::Borrowed(text),
        Err(err) => {
            let valid = err.valid_up_to();
            if valid == 0 {
                Cow::Owned(String::from_utf8_lossy(data).into_owned())
            } else {
                Cow::Owned(String::from_utf8_lossy(&data[..valid]).into_owned())
            }
        }
    }
}

fn run_attach_session(
    session: &rn_desktop_2_lib::session::protocol::Session,
    tail_data: Option<&[u8]>,
    socket_path: &str,
    config: Config,
) -> Result<()> {
    println!("\nAttaching to live session {}\n", session.id);
    println!("Detach with Ctrl-\\");

    let stream = UnixStream::connect(socket_path)
        .with_context(|| format!("Failed to connect to attach socket '{}'", socket_path))?;
    stream
        .set_read_timeout(None)
        .context("Failed to configure attach socket read timeout")?;
    stream
        .set_write_timeout(None)
        .context("Failed to configure attach socket write timeout")?;

    let reader_stream = stream
        .try_clone()
        .context("Failed to clone attach socket for reading")?;
    let writer_stream = stream;

    let raw_mode = RawModeGuard::enable()?;
    let stdin = io::stdin();
    let stdin_fd = stdin.as_raw_fd();
    let mut stdin_lock = stdin.lock();
    let stdin_guard = NonBlockingFdGuard::new(stdin_fd)?;

    render_attach_banner(session.id, tail_data)?;

    let running = Arc::new(AtomicBool::new(true));

    if let Ok((cols, rows)) = crossterm::terminal::size() {
        if let Err(err) = send_resize_request(&config, session.id, cols, rows) {
            eprintln!("Failed to send initial resize request: {}", err);
        }
    }

    let resize_watcher = match ResizeWatcher::start(session.id, config, Arc::clone(&running)) {
        Ok(watcher) => Some(watcher),
        Err(err) => {
            eprintln!("Warning: terminal resize handling disabled ({})", err);
            None
        }
    };

    let reader_running = Arc::clone(&running);

    let output_thread = thread::spawn(move || {
        if let Err(err) = pump_socket_to_stdout(reader_stream, reader_running) {
            eprintln!("\n[Attach reader error: {}]", err);
        }
    });

    let mut writer = writer_stream;
    let mut detach_requested = false;
    let mut buffer = [0u8; 1024];
    let mut input_error: Option<anyhow::Error> = None;

    'input: while running.load(Ordering::SeqCst) {
        match stdin_lock.read(&mut buffer) {
            Ok(0) => {
                thread::sleep(Duration::from_millis(INPUT_IDLE_SLEEP_MS));
                continue;
            }
            Ok(n) => {
                let mut chunk_start = 0;
                for idx in 0..n {
                    if buffer[idx] == DETACH_BYTE {
                        if idx > chunk_start {
                            if let Err(err) = writer.write_all(&buffer[chunk_start..idx]) {
                                input_error =
                                    Some(anyhow!("Failed to send input to session: {}", err));
                                running.store(false, Ordering::SeqCst);
                                break 'input;
                            }
                        }
                        detach_requested = true;
                        running.store(false, Ordering::SeqCst);
                        chunk_start = idx + 1;
                        break;
                    }
                }
                if !detach_requested && chunk_start < n {
                    if let Err(err) = writer.write_all(&buffer[chunk_start..n]) {
                        input_error = Some(anyhow!("Failed to send input to session: {}", err));
                        running.store(false, Ordering::SeqCst);
                        break;
                    }
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_millis(INPUT_IDLE_SLEEP_MS));
            }
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                running.store(false, Ordering::SeqCst);
                input_error = Some(anyhow!("Failed reading from stdin: {}", e));
                break;
            }
        }
    }

    // Close the socket to signal detach/EOF.
    let _ = writer.shutdown(Shutdown::Both);
    drop(writer);

    // Ensure the reader thread notices shutdown.
    running.store(false, Ordering::SeqCst);
    let _ = output_thread.join();

    drop(stdin_guard);
    drop(raw_mode);

    if let Some(watcher) = resize_watcher {
        watcher.stop();
    }

    if let Some(err) = input_error {
        println!("\n[Attach input error]");
        return Err(err);
    }

    if detach_requested {
        println!("\n[Detached from session {}]\n", session.id);
    } else {
        println!("\n[Session {} ended]\n", session.id);
    }

    Ok(())
}

fn render_attach_banner(session_id: u64, tail_data: Option<&[u8]>) -> Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    match tail_data {
        Some(data) if !data.is_empty() => {
            writeln!(
                handle,
                "\r\n[Replaying last {} bytes from session {}]\r",
                data.len(),
                session_id
            )?;
            handle.write_all(data)?;
            writeln!(handle, "\r\n[Live output]\r")?;
        }
        _ => {
            writeln!(handle, "\r\n[No buffered output — streaming live]\r")?;
        }
    }

    handle.flush()?;
    Ok(())
}

fn pump_socket_to_stdout(mut reader: UnixStream, running: Arc<AtomicBool>) -> io::Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let mut buf = [0u8; 4096];

    loop {
        match reader.read(&mut buf) {
            Ok(0) => {
                running.store(false, Ordering::SeqCst);
                break;
            }
            Ok(n) => {
                handle.write_all(&buf[..n])?;
                handle.flush()?;
            }
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                running.store(false, Ordering::SeqCst);
                return Err(e);
            }
        }
    }

    Ok(())
}

fn send_resize_request(config: &Config, session_id: u64, cols: u16, rows: u16) -> Result<()> {
    let mut stream = connect_to_daemon(config)?;
    let request = DaemonRequest::Resize {
        session_id,
        cols,
        rows,
    };
    match send_request(&mut stream, &request)? {
        DaemonResponse::SessionResized { .. } => Ok(()),
        DaemonResponse::Error { code: _, message } => Err(anyhow!(message)),
        other => Err(anyhow!("Unexpected resize response: {:?}", other)),
    }
}

struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> Result<Self> {
        enable_raw_mode().context("Failed to enable raw terminal mode")?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

struct NonBlockingFdGuard {
    fd: RawFd,
    original_flags: i32,
}

impl NonBlockingFdGuard {
    fn new(fd: RawFd) -> Result<Self> {
        unsafe {
            let flags = fcntl(fd, F_GETFL);
            if flags < 0 {
                anyhow::bail!("Failed to read stdin flags via fcntl");
            }
            if fcntl(fd, F_SETFL, flags | O_NONBLOCK) < 0 {
                anyhow::bail!("Failed to set stdin to non-blocking mode");
            }
            Ok(Self {
                fd,
                original_flags: flags,
            })
        }
    }
}

impl Drop for NonBlockingFdGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = fcntl(self.fd, F_SETFL, self.original_flags);
        }
    }
}

#[cfg(unix)]
struct ResizeWatcher {
    signal_handle: SignalHandle,
    thread: thread::JoinHandle<()>,
}

#[cfg(unix)]
impl ResizeWatcher {
    fn start(session_id: u64, config: Config, running: Arc<AtomicBool>) -> Result<Self> {
        let mut signals = Signals::new([SIGWINCH])?;
        let handle = signals.handle();
        let thread = thread::spawn(move || {
            for _ in signals.forever() {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok((cols, rows)) = crossterm::terminal::size() {
                    if let Err(err) = send_resize_request(&config, session_id, cols, rows) {
                        eprintln!("Failed to send resize request: {}", err);
                    }
                }
            }
        });
        Ok(Self {
            signal_handle: handle,
            thread,
        })
    }

    fn stop(self) {
        self.signal_handle.close();
        let _ = self.thread.join();
    }
}

#[cfg(not(unix))]
struct ResizeWatcher;

#[cfg(not(unix))]
impl ResizeWatcher {
    fn start(_session_id: u64, _config: Config, _running: Arc<AtomicBool>) -> Result<Self> {
        Ok(Self)
    }

    fn stop(self) {}
}
