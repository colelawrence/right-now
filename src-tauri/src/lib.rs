use rc_zip_sync::ReadZip;
use rodio::{Decoder, OutputStream, OutputStreamHandle};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, Mutex};
use tauri::State;

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

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

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
#[allow(dead_code)]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg(target_os = "macos")]
mod macos_title_bar;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn toggle_mini(window: tauri::Window, mini: bool) {
    #[cfg(target_os = "macos")]
    macos_title_bar::hide_window_buttons_each(&window, mini, mini, mini);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_nspanel::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            use tauri::Manager;
            // Initialize audio state - now won't panic if audio init fails
            let (audio_state, _audio_output) = AudioState::new();
            std::mem::forget(_audio_output); // put me in jail

            // Log audio initialization status
            if audio_state.stream_handle.is_none() {
                eprintln!("Warning: Audio device not available - sound effects will be disabled");
            }

            app.manage(audio_state);

            // let window = app.get_webview_window("main").unwrap();
            // #[cfg(debug_assertions)] // only include this code on debug builds
            // window.open_devtools();

            // FUTURE: Manage the NS Panel properly
            // #[cfg(target_os = "macos")]
            // {
            //     use tauri_nspanel::WebviewWindowExt;
            //     let window = app.get_webview_window("main").unwrap();
            //     let panel = window.to_panel().unwrap();
            //     panel.set_released_when_closed(true);
            //     panel.set_floating_panel(true);
            //     app.manage(panel);
            //     println!("Created panel");
            // }

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
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            play_sound,
            stop_sound,
            list_sound_variations,
            toggle_mini
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
