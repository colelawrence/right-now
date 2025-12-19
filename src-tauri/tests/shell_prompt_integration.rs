//! Integration test: Verify shell prompt shows task info in spawned PTY
//!
//! This test exercises the full flow:
//! 1. Start daemon directly (not via cargo run)
//! 2. Create TODO.md with a task
//! 3. Install shell integration to temp rc file
//! 4. Run `todo start` with custom BASH_ENV
//! 5. Verify prompt appears in PTY output
//! 6. Clean shutdown
//!
//! Run with: cargo test --test shell_prompt_integration

mod helpers;

use helpers::daemon_guard::{is_process_running, start_daemon, wait_for_process_exit, DaemonGuard};
use helpers::polling::wait_for_file_content;
use regex::Regex;
use rn_desktop_2_lib::session::shell_integration::{install, ShellType};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use tempfile::{Builder, TempDir};

const FILE_WAIT_TIMEOUT: Duration = Duration::from_secs(5);

/// Test that verifies environment variables are passed to the PTY
/// and can be used by shell scripts.
///
/// NOTE: This test uses --background mode because there's a known protocol issue
/// where the CLI can't parse `session_updated` notifications during attach.
/// The env vars are still verified by having the shell write them to a file.
#[test]
fn test_todo_start_shows_prompt_in_pty() {
    // Skip if binaries aren't built
    let todo_bin = match find_todo_binary() {
        Some(path) => path,
        None => {
            eprintln!("Skipping test: todo binary not found. Run `cargo build` first.");
            return;
        }
    };

    // 1. Create temp directory for test artifacts
    let temp_dir = create_test_tempdir();
    let daemon_dir = temp_dir.path().join("daemon");
    std::fs::create_dir_all(&daemon_dir).expect("Failed to create daemon dir");

    let todo_file = temp_dir.path().join("TODO.md");
    let env_output_file = temp_dir.path().join("env_output.txt");

    // 2. Create a TODO.md with a task
    std::fs::write(
        &todo_file,
        r#"---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---

- [ ] Test shell prompt
"#,
    )
    .expect("Failed to write TODO.md");

    // Pre-start the daemon
    let _daemon_guard = match start_daemon_or_skip(&daemon_dir) {
        Some(guard) => guard,
        None => return,
    };

    // 3. Use --background mode with a command that writes env vars to a file
    // This tests that env vars are actually set in the PTY
    let cmd = format!(
        "echo \"SID=$RIGHT_NOW_SESSION_ID TK=$RIGHT_NOW_TASK_KEY PROJ=$RIGHT_NOW_PROJECT DISPLAY=$RIGHT_NOW_TASK_DISPLAY\" > '{}'",
        env_output_file.display()
    );

    let output = Command::new(&todo_bin)
        .args([
            "start",
            "Test shell prompt",
            "--project",
            todo_file.to_str().unwrap(),
            "--background",
            "--cmd",
            &cmd,
        ])
        .env("HOME", temp_dir.path())
        .env("RIGHT_NOW_DAEMON_DIR", &daemon_dir)
        .output()
        .expect("Failed to run todo start");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Verify session started
    assert!(
        stdout.contains("Started session"),
        "Should start session. stdout: {}, stderr: {}",
        stdout,
        stderr
    );

    // 4. Read the output file and verify env vars were set
    let env_output = wait_for_file_content(
        &env_output_file,
        |content| {
            content.contains("SID=")
                && content.contains("TK=")
                && content.contains("PROJ=")
                && content.contains("DISPLAY=")
        },
        FILE_WAIT_TIMEOUT,
    )
    .unwrap_or_else(|err| {
        panic!(
            "Timed out waiting for env output file {}: {}. stdout: {}, stderr: {}",
            env_output_file.display(),
            err,
            stdout,
            stderr
        )
    });

    assert!(
        env_output.contains("SID=") && !env_output.contains("SID= "),
        "RIGHT_NOW_SESSION_ID should be set. Got: {}",
        env_output
    );
    assert!(
        env_output.contains("TK=Test shell prompt"),
        "RIGHT_NOW_TASK_KEY should be set. Got: {}",
        env_output
    );
    assert!(
        env_output.contains("PROJ=") && env_output.contains("TODO.md"),
        "RIGHT_NOW_PROJECT should be set. Got: {}",
        env_output
    );
    assert!(
        env_output.contains("DISPLAY=Test shell prompt"),
        "RIGHT_NOW_TASK_DISPLAY should mirror sanitized task key. Got: {}",
        env_output
    );
    println!("SUCCESS: Environment variables verified in PTY");
    println!("Output: {}", env_output);
}

#[test]
fn test_todo_start_background_does_not_attach() {
    let todo_bin = find_todo_binary();
    if todo_bin.is_none() {
        eprintln!("Skipping test: todo binary not found");
        return;
    }
    let todo_bin = todo_bin.unwrap();

    let temp_dir = create_test_tempdir();
    let daemon_dir = temp_dir.path().join("daemon");
    std::fs::create_dir_all(&daemon_dir).expect("Failed to create daemon dir");

    let todo_file = temp_dir.path().join("TODO.md");

    std::fs::write(
        &todo_file,
        "---\npomodoro_settings:\n  work_duration: 25\n---\n\n- [ ] Background test\n",
    )
    .expect("Failed to write TODO.md");

    // Pre-start the daemon
    let _daemon_guard = match start_daemon_or_skip(&daemon_dir) {
        Some(guard) => guard,
        None => return,
    };

    // Run with --background flag - should return immediately
    let start = Instant::now();
    let output = Command::new(&todo_bin)
        .args([
            "start",
            "Background test",
            "--project",
            todo_file.to_str().unwrap(),
            "--background",
            "--cmd",
            "sleep 60", // Long-running command
        ])
        .env("HOME", temp_dir.path())
        .env("RIGHT_NOW_DAEMON_DIR", &daemon_dir)
        .output()
        .expect("Failed to run todo start --background");

    let elapsed = start.elapsed();

    // Should complete quickly (not wait for the sleep)
    assert!(
        elapsed < Duration::from_secs(10),
        "Background mode should return quickly, took {:?}",
        elapsed
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("Started session"),
        "Should show session started message. stdout: {}, stderr: {}",
        stdout,
        stderr
    );
}

/// Helper: Run a full E2E test for a specific shell
///
/// Tests the complete chain: daemon → PTY → env vars → shell sources integration → prompt function → output
fn run_shell_integration_e2e(shell_type: ShellType, shell_path: &str, task_name: &str) {
    let todo_bin = match find_todo_binary() {
        Some(path) => path,
        None => {
            eprintln!("Skipping test: todo binary not found. Run `cargo build` first.");
            return;
        }
    };

    // Check if the shell is available
    if !Path::new(shell_path).exists() {
        eprintln!("Skipping test: {} not found at {}", shell_type, shell_path);
        return;
    }

    log_shell_version(shell_type, shell_path);

    // 1. Create temp directory for test artifacts
    let temp_dir = create_test_tempdir();
    let daemon_dir = temp_dir.path().join("daemon");
    std::fs::create_dir_all(&daemon_dir).expect("Failed to create daemon dir");

    let todo_file = temp_dir.path().join("TODO.md");
    let rc_file = temp_dir.path().join(format!(".{}rc", shell_type));
    let prompt_output_file = temp_dir.path().join("prompt_output.txt");
    let title_output_file = temp_dir.path().join("title_output.txt");

    // 2. Create a TODO.md with a task
    std::fs::write(
        &todo_file,
        format!(
            "---\npomodoro_settings:\n  work_duration: 25\n---\n\n- [ ] {}\n",
            task_name
        ),
    )
    .expect("Failed to write TODO.md");

    // 3. Install shell integration to the temp rc file
    install(shell_type, Some(rc_file.clone())).expect("Failed to install shell integration");

    // Verify installation
    let rc_content = std::fs::read_to_string(&rc_file).expect("Failed to read rc file");
    assert!(
        rc_content.contains("_right_now_prompt"),
        "Shell integration should be installed for {}",
        shell_type
    );

    let title_fn = title_function_name(shell_type);

    let _daemon_guard = match start_daemon_or_skip(&daemon_dir) {
        Some(guard) => guard,
        None => return,
    };

    // 4. Run todo start with command that sources rc file, then captures prompt + title output
    let cmd = format!(
        "source '{}' && _right_now_prompt > '{}' && {} > '{}'",
        rc_file.display(),
        prompt_output_file.display(),
        title_fn,
        title_output_file.display()
    );

    let output = Command::new(&todo_bin)
        .args([
            "start",
            task_name,
            "--project",
            todo_file.to_str().unwrap(),
            "--background",
            "--cmd",
            &cmd,
        ])
        .env("HOME", temp_dir.path())
        .env("RIGHT_NOW_DAEMON_DIR", &daemon_dir)
        // Force the specific shell for this test
        .env("RIGHT_NOW_SHELL", shell_path)
        .output()
        .expect("Failed to run todo start");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Verify session started
    assert!(
        stdout.contains("Started session"),
        "Should start session. stdout: {}, stderr: {}",
        stdout,
        stderr
    );

    let prompt_output = wait_for_file_content(
        &prompt_output_file,
        |content| content.contains("[#") && content.contains(task_name),
        FILE_WAIT_TIMEOUT,
    )
    .unwrap_or_else(|err| {
        panic!(
            "{}: Timed out waiting for prompt output {}: {}. stdout: {}, stderr: {}",
            shell_type,
            prompt_output_file.display(),
            err,
            stdout,
            stderr
        )
    });

    let title_output = wait_for_file_content(
        &title_output_file,
        |content| content.contains("\x1b]0;#"),
        FILE_WAIT_TIMEOUT,
    )
    .unwrap_or_else(|err| {
        panic!(
            "{}: Timed out waiting for title output {}: {}. stdout: {}, stderr: {}",
            shell_type,
            title_output_file.display(),
            err,
            stdout,
            stderr
        )
    });

    // The prompt function should output: [#<session_id>: <task_key>]
    let expected_suffix = format!(": {}]", task_name);
    assert!(
        prompt_output.contains("[#") && prompt_output.contains(&expected_suffix),
        "{}: Prompt should contain '[#<id>: {}]'. Got: '{}'",
        shell_type,
        task_name,
        prompt_output
    );

    println!(
        "SUCCESS: {} shell integration prompt function verified!",
        shell_type
    );
    println!("Prompt output: {}", prompt_output.trim());

    verify_terminal_title(&title_output, task_name, shell_type);
}

/// Full E2E test for BASH: Install shell integration, start session, verify prompt function output
///
/// This tests: daemon → PTY → env vars → bash sources integration → prompt function → output
#[test]
fn test_shell_integration_prompt_e2e_bash() {
    run_shell_integration_e2e(ShellType::Bash, "/bin/bash", "Bash E2E test");
}

/// Full E2E test for ZSH: Install shell integration, start session, verify prompt function output
///
/// This is critical since ZSH is the default shell on macOS.
/// Tests: daemon → PTY → env vars → zsh sources integration → prompt function → output
#[test]
fn test_shell_integration_prompt_e2e_zsh() {
    run_shell_integration_e2e(ShellType::Zsh, "/bin/zsh", "Zsh E2E test");
}

fn create_test_tempdir() -> TempDir {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/test-artifacts");
    std::fs::create_dir_all(&root).expect("Failed to create test artifact directory");
    Builder::new()
        .prefix("pty-tests")
        .tempdir_in(root)
        .expect("Failed to create scoped temp dir")
}

/// Find the todo binary in target directory
fn find_todo_binary() -> Option<PathBuf> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    // Try debug build first
    let debug_path = PathBuf::from(manifest_dir).join("../target/debug/todo");
    if debug_path.exists() {
        return Some(debug_path);
    }

    // Try release build
    let release_path = PathBuf::from(manifest_dir).join("../target/release/todo");
    if release_path.exists() {
        return Some(release_path);
    }

    // Try in current target directory (when run via cargo test)
    let target_path = PathBuf::from(manifest_dir).join("target/debug/todo");
    if target_path.exists() {
        return Some(target_path);
    }

    None
}

fn start_daemon_or_skip(path: &Path) -> Option<DaemonGuard> {
    match start_daemon(path) {
        Ok(guard) => Some(guard),
        Err(err) => {
            if err.is_missing_binary() {
                eprintln!("Skipping test: {}", err);
                None
            } else {
                panic!("Failed to start daemon: {}", err);
            }
        }
    }
}

fn log_shell_version(shell_type: ShellType, shell_path: &str) {
    match Command::new(shell_path).arg("--version").output() {
        Ok(output) => {
            let text = if output.stdout.is_empty() {
                String::from_utf8_lossy(&output.stderr).trim().to_string()
            } else {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            };
            println!(
                "{} version ({}): {}",
                shell_type,
                shell_path,
                if text.is_empty() { "<unknown>" } else { &text }
            );
        }
        Err(err) => {
            eprintln!(
                "Skipping shell version logging for {} ({}): {}",
                shell_type, shell_path, err
            );
        }
    }
}

fn title_function_name(shell_type: ShellType) -> &'static str {
    match shell_type {
        ShellType::Zsh => "_right_now_precmd",
        ShellType::Bash => "_right_now_title",
        ShellType::Fish => "_right_now_title",
    }
}

fn verify_terminal_title(raw_output: &str, task_name: &str, shell_type: ShellType) {
    let sanitized = raw_output.trim_matches(|c| c == '\n' || c == '\r');
    let osc_regex =
        Regex::new(r"^\x1b\]0;#(?P<sid>[^:]+): (?P<task>[^\x07]+)\x07$").expect("valid OSC regex");
    let captures = osc_regex.captures(sanitized).unwrap_or_else(|| {
        panic!(
            "{}: Terminal title missing OSC code. Got: {:?}",
            shell_type, raw_output
        )
    });

    let task = captures
        .name("task")
        .map(|m| m.as_str())
        .unwrap_or_default()
        .trim();
    assert_eq!(
        task, task_name,
        "{}: Terminal title should contain task '{}', got '{}'",
        shell_type, task_name, task
    );

    let session_id = captures
        .name("sid")
        .map(|m| m.as_str())
        .unwrap_or("")
        .trim();
    assert!(
        !session_id.is_empty(),
        "{}: Terminal title should include session id, raw output: {:?}",
        shell_type,
        raw_output
    );
}

#[test]
fn test_daemon_guard_cleans_up_processes() {
    let temp_dir = create_test_tempdir();
    let daemon_dir = temp_dir.path().join("daemon");
    std::fs::create_dir_all(&daemon_dir).expect("Failed to create daemon dir");

    let daemon = match start_daemon_or_skip(&daemon_dir) {
        Some(guard) => guard,
        None => return,
    };

    let pid = daemon.pid();
    assert!(
        is_process_running(pid),
        "Daemon should be running while guard is in scope"
    );

    drop(daemon);

    assert!(
        wait_for_process_exit(pid, Duration::from_secs(2)),
        "Daemon process {} should exit once guard is dropped",
        pid
    );
}
