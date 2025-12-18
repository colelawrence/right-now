//! Terminal notifications and sound alerts for attention events.
//!
//! Emits terminal escape codes (BEL, OSC 9, OSC 777, OSC 99) and plays
//! system sounds when sessions require user attention.
//!
//! See `docs/attention-notifications.md` for architecture overview.

use std::io::{self, Write};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::session::protocol::AttentionType;

/// Minimum time between notifications for the same session
const DEBOUNCE_DURATION: Duration = Duration::from_secs(5);

/// Tracks the last notification time for debouncing
#[derive(Default)]
pub struct NotificationDebouncer {
    last_notify: Option<Instant>,
}

impl NotificationDebouncer {
    pub fn new() -> Self {
        Self { last_notify: None }
    }

    /// Returns true if enough time has passed since the last notification
    pub fn should_notify(&mut self) -> bool {
        let now = Instant::now();
        match self.last_notify {
            Some(last) if now.duration_since(last) < DEBOUNCE_DURATION => false,
            _ => {
                self.last_notify = Some(now);
                true
            }
        }
    }

    /// Resets the debouncer (e.g., when session stops)
    pub fn reset(&mut self) {
        self.last_notify = None;
    }
}

/// Emits terminal notification escape codes to stdout.
///
/// This function writes multiple terminal escape sequences to support
/// various terminal emulators:
/// - BEL (`\x07`) - Universal terminal bell
/// - OSC 9 (iTerm2) - Desktop notification
/// - OSC 777 (Konsole/VTE/Gnome Terminal) - Desktop notification
/// - OSC 99 (kitty) - Desktop notification
pub fn emit_terminal_notifications(title: &str, message: &str) {
    let mut stdout = io::stdout();

    // BEL - universal terminal bell
    let _ = stdout.write_all(b"\x07");

    // OSC 9 - iTerm2 notification
    // Format: ESC ] 9 ; message BEL
    let osc9 = format!("\x1b]9;{}\x07", escape_osc(message));
    let _ = stdout.write_all(osc9.as_bytes());

    // OSC 777 - Konsole/VTE/Gnome Terminal
    // Format: ESC ] 777 ; notify ; title ; message BEL
    let osc777 = format!(
        "\x1b]777;notify;{};{}\x07",
        escape_osc(title),
        escape_osc(message)
    );
    let _ = stdout.write_all(osc777.as_bytes());

    // OSC 99 - kitty notification
    // Format: ESC ] 99 ; i=1:d=0:p=body ; message ST
    // i=1: unique id, d=0: no sound (we play our own), p=body: payload type
    let osc99 = format!(
        "\x1b]99;i=1:d=0:p=title;{}\x1b\\\x1b]99;i=1:d=0:p=body;{}\x1b\\",
        escape_osc(title),
        escape_osc(message)
    );
    let _ = stdout.write_all(osc99.as_bytes());

    let _ = stdout.flush();
}

/// Escapes special characters for OSC sequences
fn escape_osc(s: &str) -> String {
    // OSC sequences are terminated by BEL or ST, so we need to escape those
    s.replace('\x07', "")
        .replace('\x1b', "")
        .replace('\n', " ")
        .replace('\r', "")
}

/// Plays a system notification sound appropriate for the attention type.
///
/// On macOS, uses `afplay` with system sounds.
/// On Linux, uses `paplay` with PulseAudio.
/// Spawns the player in a detached process to avoid blocking.
pub fn play_attention_sound(attention_type: AttentionType) {
    #[cfg(target_os = "macos")]
    play_macos_sound(attention_type);

    #[cfg(target_os = "linux")]
    play_linux_sound(attention_type);
}

#[cfg(target_os = "macos")]
fn play_macos_sound(attention_type: AttentionType) {
    // Select sound based on attention type
    let sound_file = match attention_type {
        AttentionType::Error => "/System/Library/Sounds/Basso.aiff",
        AttentionType::Completed => "/System/Library/Sounds/Glass.aiff",
        AttentionType::DecisionPoint | AttentionType::InputRequired => {
            "/System/Library/Sounds/Funk.aiff"
        }
    };

    // Spawn afplay in background, ignoring errors
    let _ = Command::new("afplay")
        .arg(sound_file)
        .arg("-v")
        .arg("0.5") // 50% volume to not be jarring
        .spawn();
}

#[cfg(target_os = "linux")]
fn play_linux_sound(attention_type: AttentionType) {
    // Try common Linux sound paths
    let sound_candidates = match attention_type {
        AttentionType::Error => vec![
            "/usr/share/sounds/freedesktop/stereo/dialog-error.oga",
            "/usr/share/sounds/gnome/default/alerts/bark.ogg",
        ],
        AttentionType::Completed => vec![
            "/usr/share/sounds/freedesktop/stereo/complete.oga",
            "/usr/share/sounds/gnome/default/alerts/glass.ogg",
        ],
        AttentionType::DecisionPoint | AttentionType::InputRequired => vec![
            "/usr/share/sounds/freedesktop/stereo/message-new-instant.oga",
            "/usr/share/sounds/gnome/default/alerts/drip.ogg",
        ],
    };

    // Find first existing sound file
    let sound_file = sound_candidates
        .into_iter()
        .find(|path| std::path::Path::new(path).exists());

    if let Some(path) = sound_file {
        // Try paplay (PulseAudio) first, then aplay (ALSA)
        if Command::new("paplay").arg(path).spawn().is_err() {
            let _ = Command::new("aplay").arg("-q").arg(path).spawn();
        }
    }
}

/// Sends a notification for an attention event.
///
/// Combines terminal escape codes and sound playback.
/// Should be called after debounce check passes.
pub fn notify_attention(profile: &str, attention_type: AttentionType, preview: &str) {
    let title = format!("right-now: {}", profile);
    let message = truncate_preview(preview, 80);

    emit_terminal_notifications(&title, &message);
    play_attention_sound(attention_type);
}

/// Truncates preview text for notifications
fn truncate_preview(preview: &str, max_len: usize) -> String {
    // Take first line only for notification
    let first_line = preview.lines().next().unwrap_or(preview);
    if first_line.len() <= max_len {
        first_line.to_string()
    } else {
        format!("{}...", &first_line[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debouncer_allows_first_notification() {
        let mut debouncer = NotificationDebouncer::new();
        assert!(debouncer.should_notify());
    }

    #[test]
    fn debouncer_blocks_rapid_notifications() {
        let mut debouncer = NotificationDebouncer::new();
        assert!(debouncer.should_notify());
        assert!(!debouncer.should_notify());
        assert!(!debouncer.should_notify());
    }

    #[test]
    fn debouncer_reset_allows_immediate_notification() {
        let mut debouncer = NotificationDebouncer::new();
        assert!(debouncer.should_notify());
        debouncer.reset();
        assert!(debouncer.should_notify());
    }

    #[test]
    fn escape_osc_removes_control_chars() {
        assert_eq!(escape_osc("hello\x07world"), "helloworld");
        assert_eq!(escape_osc("test\x1b[0m"), "test[0m");
        assert_eq!(escape_osc("line1\nline2"), "line1 line2");
    }

    #[test]
    fn truncate_preview_respects_max_length() {
        let long_text = "a".repeat(100);
        let truncated = truncate_preview(&long_text, 80);
        assert!(truncated.len() <= 80);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn truncate_preview_takes_first_line() {
        let multiline = "first line\nsecond line\nthird line";
        assert_eq!(truncate_preview(multiline, 80), "first line");
    }
}
