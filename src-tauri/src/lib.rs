use rc_zip_sync::ReadZip;
use rodio::{Decoder, OutputStream, OutputStreamHandle};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::State;

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

// Session management module (shared between daemon, CLI, and main app)
pub mod session;

// CLI paths configuration (shared between app, shim, and daemon)
pub mod cli_paths;

// Context resurrection module (snapshot capture + storage + query)
pub mod context_resurrection;

// Test harness module (only compiled with test-harness feature)
#[cfg(feature = "test-harness")]
pub mod test_harness;

use crate::session::config::Config;

// Global state to hold our audio output stream
#[derive(Clone)]
struct AudioState {
    stream_handle: Arc<Option<OutputStreamHandle>>,
    active_sinks: Arc<Mutex<HashMap<u32, rodio::Sink>>>,
}

// Keep the output stream alive in a separate struct
struct AudioOutput {
    _stream: Option<OutputStream>,
}

impl AudioState {
    fn new() -> (Self, AudioOutput) {
        // Try to initialize audio, but don't fail if we can't
        let (stream, handle) = match OutputStream::try_default() {
            Ok((stream, handle)) => (Some(stream), Some(handle)),
            Err(e) => {
                eprintln!("Failed to initialize audio device: {}", e);
                (None, None)
            }
        };

        (
            AudioState {
                stream_handle: Arc::new(handle),
                active_sinks: Arc::new(Mutex::new(HashMap::new())),
            },
            AudioOutput { _stream: stream },
        )
    }

    fn get_stream_handle(&self) -> Result<&OutputStreamHandle, String> {
        (*self.stream_handle).as_ref().ok_or_else(|| {
            "No audio device available. Please check your system's audio settings.".to_string()
        })
    }
}

fn find_matching_sound_files(
    zip_archive: &rc_zip_sync::ArchiveHandle<'_, std::fs::File>,
    name: &str,
) -> Vec<String> {
    // Look for both exact matches (name.mp3) and variations (name.*.mp3)
    let exact_pattern = format!("{}.mp3", name);
    let wildcard_pattern = format!("{}.", name);

    zip_archive
        .entries()
        .filter(|entry| {
            let entry_name = entry.name.to_lowercase();
            entry_name == exact_pattern
                || (entry_name.starts_with(&wildcard_pattern) && entry_name.ends_with(".mp3"))
        })
        .map(|entry| entry.name.to_string())
        .collect()
}

#[tauri::command]
async fn play_sound(
    state: State<'_, AudioState>,
    sound_pack_path: String,
    name: String,
    invocation: u32,
) -> Result<(), String> {
    // First check if we have a valid audio device
    let stream_handle = state.get_stream_handle()?;

    // Open the ZIP file
    let file =
        File::open(&sound_pack_path).map_err(|e| format!("Failed to open sound pack: {}", e))?;

    // Read the ZIP archive
    let archive = file
        .read_zip()
        .map_err(|e| format!("Failed to read ZIP archive: {}", e))?;

    // Find all matching sound files
    let matching_files = find_matching_sound_files(&archive, &name);

    if matching_files.is_empty() {
        return Err(format!("No matching sound files found for '{}'", name));
    }

    // Select a file based on the invocation number
    let selected_file = &matching_files[invocation as usize % matching_files.len()];

    // Get the entry from the archive
    let entry = archive
        .by_name(selected_file)
        .ok_or_else(|| format!("Failed to find entry '{}'", selected_file))?;

    // Read the entry's bytes
    let bytes = entry
        .bytes()
        .map_err(|e| format!("Failed to read sound file bytes: {}", e))?;

    // Create a decoder from the bytes
    let source = Decoder::new(BufReader::new(std::io::Cursor::new(bytes)))
        .map_err(|e| format!("Failed to decode audio: {}", e))?;

    // Create a new sink for this sound
    let sink = rodio::Sink::try_new(stream_handle)
        .map_err(|e| format!("Failed to create audio sink: {}", e))?;

    // Start playing
    sink.append(source);

    // Store the sink so we can control it later if needed
    state
        .active_sinks
        .lock()
        .map_err(|e| format!("Failed to acquire lock: {}", e))?
        .insert(invocation, sink);

    Ok(())
}

#[tauri::command]
fn set_current_project_path(path: Option<String>) -> Result<(), String> {
    let config = Config::from_env();
    match path {
        Some(p) if !p.trim().is_empty() => config
            .write_current_project(p.trim())
            .map_err(|e| format!("Failed to record project path: {}", e)),
        _ => config
            .clear_current_project()
            .map_err(|e| format!("Failed to clear project path: {}", e)),
    }
}

#[tauri::command]
async fn stop_sound(state: State<'_, AudioState>, invocation: u32) -> Result<(), String> {
    if let Some(sink) = state
        .active_sinks
        .lock()
        .map_err(|e| format!("Failed to acquire lock: {}", e))?
        .remove(&invocation)
    {
        sink.stop();
    }
    Ok(())
}

#[tauri::command]
async fn list_sound_variations(
    sound_pack_path: String,
    name: String,
) -> Result<Vec<String>, String> {
    // Open the ZIP file
    let file =
        File::open(&sound_pack_path).map_err(|e| format!("Failed to open sound pack: {}", e))?;

    // Read the ZIP archive
    let archive = file
        .read_zip()
        .map_err(|e| format!("Failed to read ZIP archive: {}", e))?;

    // Find and return all matching sound files
    Ok(find_matching_sound_files(&archive, &name))
}

#[cfg(target_os = "macos")]
mod macos_title_bar;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn toggle_mini_os_specific_styling(window: tauri::Window, mini: bool) {
    #[cfg(target_os = "macos")]
    macos_title_bar::hide_window_buttons_each(&window, mini, mini, mini);
}

// ============================================================================
// Session/Daemon Commands (Unix only for now)
// ============================================================================

#[cfg(unix)]
#[tauri::command]
fn session_list(project_path: Option<String>) -> Result<Vec<session::protocol::Session>, String> {
    use session::daemon_client::{response_to_result, send_request};
    use session::protocol::{DaemonRequest, DaemonResponse};

    let request = DaemonRequest::List { project_path };

    let response = send_request(request).map_err(|e| e.to_string())?;

    response_to_result(response, |r| {
        if let DaemonResponse::SessionList { sessions } = r {
            Some(sessions)
        } else {
            None
        }
    })
}

#[cfg(unix)]
#[tauri::command]
fn session_start(
    task_key: String,
    task_id: Option<String>,
    project_path: String,
    shell: Option<Vec<String>>,
) -> Result<session::protocol::Session, String> {
    use session::daemon_client::{response_to_result, send_request};
    use session::protocol::{DaemonRequest, DaemonResponse};

    let request = DaemonRequest::Start {
        task_key,
        task_id,
        project_path,
        shell,
    };

    let response = send_request(request).map_err(|e| e.to_string())?;

    response_to_result(response, |r| {
        if let DaemonResponse::SessionStarted { session } = r {
            Some(session)
        } else {
            None
        }
    })
}

#[cfg(unix)]
#[tauri::command]
fn session_stop(session_id: u64) -> Result<session::protocol::Session, String> {
    use session::daemon_client::{response_to_result, send_request};
    use session::protocol::{DaemonRequest, DaemonResponse};

    let request = DaemonRequest::Stop { session_id };

    let response = send_request(request).map_err(|e| e.to_string())?;

    response_to_result(response, |r| {
        if let DaemonResponse::SessionStopped { session } = r {
            Some(session)
        } else {
            None
        }
    })
}

#[cfg(unix)]
#[tauri::command]
fn session_continue(
    session_id: u64,
    tail_bytes: Option<usize>,
) -> Result<serde_json::Value, String> {
    use session::daemon_client::{response_to_result, send_request};
    use session::protocol::{DaemonRequest, DaemonResponse};

    let request = DaemonRequest::Continue {
        session_id,
        tail_bytes,
    };

    let response = send_request(request).map_err(|e| e.to_string())?;

    response_to_result(response, |r| {
        if let DaemonResponse::SessionContinued { session, tail } = r {
            Some(serde_json::json!({
                "session": session,
                "tail": tail
            }))
        } else {
            None
        }
    })
}

#[cfg(unix)]
#[tauri::command]
fn cr_request(
    request: session::protocol::DaemonRequest,
) -> Result<session::protocol::DaemonResponse, String> {
    use session::daemon_client::send_request;
    use session::protocol::DaemonRequest;

    match &request {
        DaemonRequest::CrLatest { .. }
        | DaemonRequest::CrList { .. }
        | DaemonRequest::CrGet { .. }
        | DaemonRequest::CrCaptureNow { .. }
        | DaemonRequest::CrDeleteTask { .. }
        | DaemonRequest::CrDeleteProject { .. } => {}
        _ => {
            return Err("Unsupported request type for cr_request".to_string());
        }
    }

    send_request(request).map_err(|e| e.to_string())
}

// Stub commands for non-Unix platforms
#[cfg(not(unix))]
#[tauri::command]
fn session_list(_project_path: Option<String>) -> Result<Vec<()>, String> {
    Err("Session management not yet supported on this platform".to_string())
}

#[cfg(not(unix))]
#[tauri::command]
fn session_start(
    _task_key: String,
    _project_path: String,
    _shell: Option<Vec<String>>,
) -> Result<(), String> {
    Err("Session management not yet supported on this platform".to_string())
}

#[cfg(not(unix))]
#[tauri::command]
fn session_stop(_session_id: u64) -> Result<(), String> {
    Err("Session management not yet supported on this platform".to_string())
}

#[cfg(not(unix))]
#[tauri::command]
fn session_continue(_session_id: u64, _tail_bytes: Option<usize>) -> Result<(), String> {
    Err("Session management not yet supported on this platform".to_string())
}

#[cfg(not(unix))]
#[tauri::command]
fn cr_request(
    _request: session::protocol::DaemonRequest,
) -> Result<session::protocol::DaemonResponse, String> {
    Err("Context Resurrection not yet supported on this platform".to_string())
}

// ============================================================================
// CLI Shim Commands
// ============================================================================

#[derive(serde::Serialize)]
struct CliShimInfo {
    name: String,
    installed: bool,
    path: Option<String>,
    install_dir: Option<String>,
}

#[tauri::command]
fn get_cli_shim_info() -> CliShimInfo {
    let status = cli_paths::ShimStatus::check();
    let install_dir = cli_paths::shim_install_dir().map(|p| p.to_string_lossy().to_string());

    match status {
        cli_paths::ShimStatus::Installed { path, name } => CliShimInfo {
            name,
            installed: true,
            path: Some(path.to_string_lossy().to_string()),
            install_dir,
        },
        cli_paths::ShimStatus::NotInstalled { name } => CliShimInfo {
            name,
            installed: false,
            path: None,
            install_dir,
        },
        cli_paths::ShimStatus::DirectoryMissing => CliShimInfo {
            name: cli_paths::DEFAULT_CLI_NAME.to_string(),
            installed: false,
            path: None,
            install_dir,
        },
    }
}

#[tauri::command]
fn set_cli_name(name: String) -> Result<String, String> {
    cli_paths::validate_cli_name(&name)?;
    cli_paths::set_cli_name(&name).map_err(|e| e.to_string())?;
    Ok(format!("CLI name set to '{}'", name))
}

#[tauri::command]
fn install_cli_shim(name: Option<String>) -> Result<String, String> {
    let cli_name = name.unwrap_or_else(cli_paths::current_cli_name);
    cli_paths::validate_cli_name(&cli_name)?;

    let path = cli_paths::install_shim_as(&cli_name).map_err(|e| e.to_string())?;
    Ok(format!("'{}' installed to {}", cli_name, path.display()))
}

#[tauri::command]
fn uninstall_cli_shim() -> Result<String, String> {
    let name = cli_paths::current_cli_name();
    cli_paths::uninstall_shim().map_err(|e| e.to_string())?;
    Ok(format!("'{}' CLI uninstalled", name))
}

const MENU_ID_CLI_SHIM: &str = "cli_shim_toggle";

fn get_shim_menu_text() -> String {
    let status = cli_paths::ShimStatus::check();
    let name = status.cli_name();
    if status.is_installed() {
        format!("Uninstall '{}' CLI...", name)
    } else {
        format!("Install '{}' CLI...", name)
    }
}

fn setup_app_menu(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{AboutMetadata, SubmenuBuilder};

    let shim_menu_item =
        MenuItemBuilder::with_id(MENU_ID_CLI_SHIM, get_shim_menu_text()).build(app)?;

    // Build standard app menu (macOS "AppName" menu, or first menu on other platforms)
    let app_menu = SubmenuBuilder::new(app, "Right Now")
        .about(Some(AboutMetadata::default()))
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    // Build Edit menu with standard items
    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    // Build Window menu
    let window_menu = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .separator()
        .close_window()
        .build()?;

    // Build Tools menu with our custom item
    let tools_menu = SubmenuBuilder::new(app, "Tools")
        .item(&shim_menu_item)
        .build()?;

    let menu = MenuBuilder::new(app)
        .item(&app_menu)
        .item(&edit_menu)
        .item(&window_menu)
        .item(&tools_menu)
        .build()?;

    app.set_menu(menu)?;

    Ok(())
}

fn handle_menu_event(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    use tauri_plugin_dialog::DialogExt;

    match event.id().0.as_str() {
        MENU_ID_CLI_SHIM => {
            let status = cli_paths::ShimStatus::check();
            let result = if status.is_installed() {
                cli_paths::uninstall_shim().map(|_| "CLI uninstalled successfully".to_string())
            } else {
                cli_paths::install_shim().map(|path| format!("CLI installed to {}", path.display()))
            };

            // Update the menu item text
            if let Some(menu) = app.menu() {
                if let Some(item) = menu.get(MENU_ID_CLI_SHIM) {
                    if let Some(menu_item) = item.as_menuitem() {
                        let _ = menu_item.set_text(get_shim_menu_text());
                    }
                }
            }

            // Show result dialog
            match result {
                Ok(msg) => {
                    let status = cli_paths::ShimStatus::check();
                    let name = status.cli_name();
                    let path_hint = if cfg!(windows) {
                        "%USERPROFILE%\\bin"
                    } else {
                        "~/.local/bin"
                    };
                    let detail = if status.is_installed() {
                        format!(
                            "{}\n\nYou can now use '{}' from your terminal.\n\nMake sure {} is in your PATH.",
                            msg, name, path_hint
                        )
                    } else {
                        msg
                    };
                    app.dialog()
                        .message(detail)
                        .title("CLI Tool")
                        .blocking_show();
                }
                Err(e) => {
                    app.dialog()
                        .message(format!("Failed: {}", e))
                        .title("Error")
                        .blocking_show();
                }
            }
        }
        _ => {}
    }
}

/// Create the base Tauri builder with all standard plugins
fn create_base_builder() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(tauri_nspanel::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_deep_link::init())
}

/// Standard app setup (shared between normal and test harness modes)
fn setup_app(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::Manager;

    // Initialize audio state - now won't panic if audio init fails
    let (audio_state, _audio_output) = AudioState::new();
    std::mem::forget(_audio_output); // put me in jail

    // Log audio initialization status
    if audio_state.stream_handle.is_none() {
        eprintln!("Warning: Audio device not available - sound effects will be disabled");
    }

    app.manage(audio_state);

    // Write CLI paths so the shim can find our binaries
    if let Err(e) = cli_paths::CliPaths::write_from_current_exe() {
        eprintln!("Warning: Failed to write CLI paths: {}", e);
    }

    // Setup application menu with CLI shim install option
    setup_app_menu(app)?;

    #[cfg(desktop)]
    app.handle()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .map_err(|e| {
            println!("Error initializing autostart plugin: {}", e);
            e
        })?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    create_base_builder()
        .setup(|app| setup_app(app))
        .on_menu_event(handle_menu_event)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            play_sound,
            stop_sound,
            list_sound_variations,
            toggle_mini_os_specific_styling,
            set_current_project_path,
            get_cli_shim_info,
            set_cli_name,
            install_cli_shim,
            uninstall_cli_shim,
            session_list,
            session_start,
            session_stop,
            session_continue,
            cr_request
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Create a test harness builder (context must be passed from the binary)
/// This version includes additional commands for test control and starts a Unix socket server
#[cfg(feature = "test-harness")]
pub fn create_test_harness_builder() -> tauri::Builder<tauri::Wry> {
    use std::sync::Arc;

    // Create test harness state
    let harness_state = Arc::new(test_harness::TestHarnessState::new());

    // Clone for the setup closure
    let harness_state_for_setup = Arc::clone(&harness_state);

    create_base_builder()
        .setup(move |app| {
            use tauri::Manager;

            // Standard app setup (but skip audio for faster tests)
            // Note: We don't initialize audio in test mode to avoid noise

            // Write CLI paths so the shim can find our binaries
            if let Err(e) = cli_paths::CliPaths::write_from_current_exe() {
                eprintln!("Warning: Failed to write CLI paths: {}", e);
            }

            // Manage the test harness state
            let harness_state = Arc::clone(&harness_state_for_setup);
            app.manage(harness_state.clone());

            // Set the app handle so the harness can emit events
            let app_handle = app.handle().clone();
            let harness_for_handle = harness_state.clone();
            tauri::async_runtime::spawn(async move {
                harness_for_handle.set_app_handle(app_handle).await;
            });

            // Start the Unix socket server for test control
            let socket_path = test_harness::default_socket_path();
            let state_for_server = harness_state;

            // We need a tokio runtime for the server
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
                rt.block_on(async {
                    match test_harness::start_server(socket_path, state_for_server).await {
                        Ok(mut shutdown_rx) => {
                            // Wait for shutdown signal
                            let _ = shutdown_rx.recv().await;
                        }
                        Err(e) => {
                            eprintln!("Failed to start test harness server: {}", e);
                        }
                    }
                });
            });

            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_autostart::init(
                    tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                    None,
                ))
                .map_err(|e| {
                    println!("Error initializing autostart plugin: {}", e);
                    e
                })?;

            Ok(())
        })
        .on_menu_event(handle_menu_event)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // Standard commands (but play_sound is a no-op in test mode)
            play_sound,
            stop_sound,
            list_sound_variations,
            toggle_mini_os_specific_styling,
            set_current_project_path,
            get_cli_shim_info,
            set_cli_name,
            install_cli_shim,
            uninstall_cli_shim,
            session_list,
            session_start,
            session_stop,
            session_continue,
            cr_request,
            // Test harness commands
            test_harness::test_create_temp_dir,
            test_harness::test_load_fixture,
            test_harness::test_list_fixtures,
            test_harness::test_cleanup_all,
            test_harness::test_get_socket_path,
            test_harness::test_respond,
            test_harness::test_set_app_handle
        ])
}
