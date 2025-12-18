# Attention Detection & Terminal Notifications

This document explains how the daemon monitors PTY sessions for events that need user attention and delivers notifications.

## Architecture

```
PTY output → attention.rs (pattern detection) → notify.rs (alerts)
```

The daemon's output watcher task calls `detect_attention()` on each PTY output chunk. When a pattern matches and the debouncer allows it, `notify_attention()` emits terminal escape codes and plays a sound.

## Attention Detection

**Source:** `src-tauri/src/session/attention.rs`

### Profiles & Triggers

Attention profiles group related triggers. Each trigger has:
- A **matcher** (literal string or regex)
- An **attention type** (Error, Completed, DecisionPoint, InputRequired)
- A **preview strategy** for extracting context

Default profiles:

| Profile | Pattern | Type | Use Case |
|---------|---------|------|----------|
| `claude-code` | `✔ Submit` | DecisionPoint | Claude waiting for approval |
| `claude-code` | `Enter to select` | InputRequired | Menu selection |
| `claude-code` | `❯` | InputRequired | Prompt waiting |
| `build-tools` | `build (succeeded\|complete\|passed)` | Completed | Build finished |
| `build-tools` | `(error\|failed\|failure):` | Error | Build failed |

### Preview Strategies

When a trigger matches, a preview is extracted for the notification message:

- **LastLines(n)** — Returns the last N lines of the output buffer
- **Surround { before, after }** — Returns bytes around the match position

## Notification Delivery

**Source:** `src-tauri/src/session/notify.rs`

### Terminal Escape Codes

Multiple escape sequences are emitted to support various terminal emulators:

| Sequence | Terminal | Format |
|----------|----------|--------|
| BEL (`\x07`) | Universal | Audible/visual bell |
| OSC 9 | iTerm2 | `ESC ] 9 ; message BEL` |
| OSC 777 | Konsole/VTE/GNOME | `ESC ] 777 ; notify ; title ; message BEL` |
| OSC 99 | kitty | `ESC ] 99 ; i=1:d=0:p=body ; message ST` |

### Sound Playback

System sounds are played based on attention type:

| Attention Type | macOS | Linux |
|----------------|-------|-------|
| Error | `/System/Library/Sounds/Basso.aiff` | `dialog-error.oga` |
| Completed | `/System/Library/Sounds/Glass.aiff` | `complete.oga` |
| DecisionPoint / InputRequired | `/System/Library/Sounds/Funk.aiff` | `message-new-instant.oga` |

- macOS: Uses `afplay` at 50% volume
- Linux: Tries `paplay` (PulseAudio), falls back to `aplay` (ALSA)

### Debouncing

`NotificationDebouncer` prevents notification spam with a 5-second cooldown per session. The first notification fires immediately; subsequent ones within the window are suppressed.

## Adding New Triggers

To detect new patterns:

1. Add a trigger to an existing profile in `DEFAULT_PROFILES`, or create a new profile
2. Choose the appropriate `AttentionType` for sound selection
3. Pick a `PreviewStrategy` that captures useful context

Example:
```rust
AttentionTrigger::regex(
    r"(?i)tests? (passed|succeeded)",
    AttentionType::Completed,
    PreviewStrategy::LastLines(3),
)
```

## Testing

Run attention-related tests:
```bash
cargo test -p right-now --lib attention
cargo test -p right-now --lib notify
```
