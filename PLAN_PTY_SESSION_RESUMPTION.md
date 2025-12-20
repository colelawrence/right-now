# PTY Session Cessation & Resumption - Design Exploration

## Core Goal: Attention Management

**The system's purpose is to tell the user when their attention is required.**

Sessions run in the background; when the terminal needs human input or reaches a decision point, the system should surface that clearly—with the right preview text showing *what* needs attention.

## User Requirements (Clarified)

1. **Re-attach to running:** Connect terminal to a background session (like `tmux attach`)
2. **Restart with context:** Preserve working dir/env so stopped sessions can restart in same state
3. **Application-specific patterns:** Different apps have different "attention needed" markers
   - Claude Code: `"✔ Submit"`, `"Enter to select"`, `"❯"` character
   - Build tools: `"Build succeeded"`, `"error:"`
   - Other agents: custom phrases per profile
4. **ANSI-aware parsing:** Detect formatting (bold, colors) to understand focus/emphasis
5. **Preview extraction:** Capture the text around the trigger for meaningful notifications
6. **Architecture decisions:**
   - Ring buffer: in-memory only (accept loss on daemon crash)
   - Attach: per-session socket (cleaner separation)

---

## Current State Summary

### What's Implemented (Phases 3-4 to-date)
- **PTY Lifecycle:** Sessions can start, run, go idle (30s timeout), and stop.
- **Ring Buffer:** 64KB circular buffer stores recent output (in-memory only, not persisted) and snapshots are retained for stopped sessions so summaries can still show final output.
- **State Transitions:** Poll every 5s → Running/Waiting/Stopped based on alive + idle status.
- **Start/Stop Flow:** Markdown badges stay in sync via atomic rewrites; exit codes are captured and persisted.
- **Attach Handshake:** `DaemonRequest::Attach` returns replay bytes plus a per-session Unix socket. The daemon streams PTY output/input through the socket using broadcast events from `PtyRuntime`.
- **CLI Raw Mode:** `todo continue --attach` enters raw mode with `crossterm`, replays the tail, streams live output, forwards stdin (including control bytes) to the PTY, and supports Ctrl-\\ detach.
- **Dynamic Resizing:** CLI watches for SIGWINCH via `signal-hook`, sends `DaemonRequest::Resize`, and daemon calls `portable-pty` resize.
- **Project Fallback:** When no TODO.md is found in ancestor directories, the CLI falls back to the project currently selected in the UI via the shared marker file.
- **Attention Detection:** Complete engine in `session/attention.rs`:
  - `AttentionProfile` + `AttentionTrigger` structs with regex/literal matchers
  - Default profiles: Claude Code (`✔ Submit`, `❯`, `Enter to select`) and build-tools (`error:`, `build succeeded`)
  - `PreviewStrategy` with `LastLines(n)` and `Surround { before, after }` variants
  - Real-time detection via `spawn_attention_monitor()` subscribing to `PtyEvent::Output`
  - Debouncing: duplicate previews are suppressed
  - `Session.last_attention` persisted; `DaemonNotification::Attention` broadcast to subscribers
  - CLI displays attention info in `todo continue` summaries

### Key Gaps
1. **Notification delivery:** No terminal escape codes (BEL, OSC) or sound playback when attention triggers fire.
2. **ANSI parsing:** Patterns match raw bytes including escape sequences; stripping them would improve accuracy.
3. **Cross-chunk detection:** Patterns split across PTY output chunks may be missed (polling fallback not implemented).
4. **Restart scaffolding:** We do not yet capture `cwd`, env snapshots, or shell commands beyond the initial spawn, so restart flows cannot be automated.
5. **Multi-client attach:** No policy for concurrent attach clients (currently allowed implicitly via broadcast).

---

## Design: Re-attach to Running Sessions (Status: Streaming MVP shipped)

### UX Flow
```
$ todo continue 42 --attach
[Replaying last 4KB of output...]
...previous output here...
[Attached to session 42. Press Ctrl+D to detach.]
```

### Implementation (done + next steps)
1. **Raw mode + replay (DONE)** — CLI enables `crossterm` raw mode, prints a replay banner, and switches to live streaming once the tail is flushed.
2. **Attach handshake (DONE)** — `DaemonRequest::Attach { session_id, tail_bytes }` returns `{ session, tail, socket_path }`. The daemon keeps a per-session Unix listener alive until the PTY exits.
3. **Bidirectional streaming (DONE)** — `PtyRuntime` exposes a broadcast-based event stream so attach connections get output/exit notifications, and stdin is forwarded through the attach socket.
4. **Resizing (DONE) / multi-attaches (TODO)** — The CLI now watches for SIGWINCH, sends a `DaemonRequest::Resize`, and the daemon resizes the PTY via `portable-pty`. We still need to decide whether multiple simultaneous attach clients should be supported or guarded.
5. **Signals & UX (TODO)** — Forward SIGWINCH/SIGTERM, standardize detach shortcuts/messages, and emit exit banners (`[process exited with code X]`) on both the socket and the control channel so UI clients stay informed.

---

## Design: Restart with Context

### The Challenge
When a PTY stops, the shell process dies. We can't resume it—but we CAN restart a new shell with the same context if we capture it beforehand.

### What Context to Capture
| Context | How to Capture | When to Capture |
|---------|----------------|-----------------|
| Working directory | Query via `/proc/{pid}/cwd` (Linux) or `lsof` (macOS) | Periodically while running |
| Environment vars | Query via `/proc/{pid}/environ` or passed at spawn | At spawn time (inherited) |
| Shell command | Already stored in `Session.shell` | At spawn time |
| Exit code | Already captured via `PtyEvent::Exited` | On exit |

### Implementation
1. **Extend `Session` struct** with `last_known_cwd: Option<PathBuf>`
2. **Periodically capture cwd** (every 5s in watch task) while session is running
3. **On restart:** spawn new PTY with `cd {last_known_cwd} && {original_command}`
4. **Replay ring buffer** to show what happened before restart

### Limitation
- Can't restore in-progress process state (e.g., a half-complete `npm install`)
- User must understand restart ≠ resume

---

## Design: Attention Detection System

### Core Concept: Application Profiles

Different applications have different "attention needed" markers. Instead of one hardcoded pattern list, we need **profiles**:

```rust
struct AttentionProfile {
    name: String,              // "claude-code", "npm-build", etc.
    patterns: Vec<AttentionPattern>,
}

struct AttentionPattern {
    matcher: PatternMatcher,   // regex, literal, or ANSI-aware
    attention_type: AttentionType,
    preview_strategy: PreviewStrategy,
}

enum AttentionType {
    InputRequired,  // User needs to type something
    DecisionPoint,  // User needs to choose/confirm
    Completed,      // Task finished, review results
    Error,          // Something went wrong
}

enum PreviewStrategy {
    LastNLines(usize),        // Show last N lines before match
    MatchContext { before: usize, after: usize },
    BoldTextOnly,             // Extract only bold/emphasized text
}
```

### Example Profiles

**Claude Code Profile:**
```rust
AttentionProfile {
    name: "claude-code",
    patterns: vec![
        AttentionPattern {
            matcher: Literal("✔ Submit"),
            attention_type: DecisionPoint,
            preview_strategy: LastNLines(3),
        },
        AttentionPattern {
            matcher: Literal("Enter to select"),
            attention_type: InputRequired,
            preview_strategy: BoldTextOnly,
        },
        AttentionPattern {
            matcher: Literal("❯"),
            attention_type: InputRequired,
            preview_strategy: MatchContext { before: 2, after: 0 },
        },
    ],
}
```

**Build Tools Profile:**
```rust
AttentionProfile {
    name: "build-tools",
    patterns: vec![
        AttentionPattern {
            matcher: Regex(r"(?i)build (succeeded|complete|passed)"),
            attention_type: Completed,
            preview_strategy: LastNLines(5),
        },
        AttentionPattern {
            matcher: Regex(r"(?i)(error|failed|failure):"),
            attention_type: Error,
            preview_strategy: MatchContext { before: 0, after: 10 },
        },
    ],
}
```

### ANSI Escape Parsing

Terminal output includes escape sequences for formatting:
- `\x1b[1m` = bold start, `\x1b[0m` = reset
- `\x1b[32m` = green, `\x1b[31m` = red
- Cursor movement, clear screen, etc.

**Approach:**
1. Parse ANSI sequences using `ansi_term` or `vte` crate
2. Build structured representation: `Vec<Span>` with text + formatting
3. Pattern matching can query: "is this text bold?" or "what's the current color?"

### State Machine

```
                    ┌─────────────────────────┐
                    │        Running          │
                    │   (no attention needed) │
                    └───────────┬─────────────┘
                                │
                    ┌───────────┼───────────┐
                    │           │           │
                    ▼           ▼           ▼
             ┌──────────┐ ┌──────────┐ ┌──────────┐
             │ Awaiting │ │ Decision │ │  Error   │
             │  Input   │ │  Point   │ │          │
             └────┬─────┘ └────┬─────┘ └────┬─────┘
                  │            │            │
                  └────────────┼────────────┘
                               ▼
                    ┌─────────────────────────┐
                    │   NeedsAttention        │
                    │   (+ preview text)      │
                    │   (+ attention type)    │
                    └───────────┬─────────────┘
                                │
                    ┌───────────┼───────────┐
                    ▼           │           ▼
              User attaches     │      Process exits
                    │           │           │
                    ▼           │           ▼
                 Running    ────┼────→  Stopped
```

### Pattern Matching Strategy

**Recommendation: Hybrid approach**

1. **Real-time streaming** (low latency for input prompts):
   - Subscribe to `PtyEvent::Output`
   - Run patterns on each chunk
   - Instant detection of `"❯"` or similar

2. **Polling backup** (for cross-chunk patterns):
   - Every 5s, check ring buffer
   - Catches patterns split across chunks

### Files to Modify

- `src-tauri/src/session/attention.rs` (NEW) — AttentionProfile, pattern matching, ANSI parsing
- `src-tauri/src/session/runtime.rs` — integrate attention detector, emit AttentionEvents
- `src-tauri/src/session/protocol.rs` — add AttentionType, NeedsAttention status, preview field
- `src-tauri/src/bin/right-now-daemon.rs` — broadcast attention events to UI/CLI

---

## Implementation Phases

### Phase 4A: PTY Attach (Priority: High) — ✅ COMPLETE

**Files:**
- `src-tauri/src/bin/todo.rs` — `--attach` flag, raw mode, SIGWINCH handling, resize requests
- `src-tauri/src/session/protocol.rs` — `DaemonRequest::Attach`, `Resize`, `DaemonResponse::AttachReady`, `SessionResized`
- `src-tauri/src/bin/right-now-daemon.rs` — per-session socket, I/O streaming, resize handler
- `src-tauri/src/session/runtime.rs` — broadcast event channel, `resize()` method

**Completed:**
- [x] Add `--attach` flag to `todo continue` (`todo.rs:199-206, 293-337`)
- [x] Enter raw mode in CLI using `crossterm` (`todo.rs:729-742`)
- [x] Spawn per-session Unix socket on attach (`~/.right-now/attach-{id}.sock`) (`right-now-daemon.rs:183-249`)
- [x] Implement bidirectional I/O: STDIN → PTY, PTY → STDOUT (`right-now-daemon.rs:251-326`, `todo.rs:529-663`)
- [x] Replay ring buffer on attach with `[Replaying last N bytes]` banner (`todo.rs:665-687`)
- [x] Handle Ctrl-\\ detach (`todo.rs:597-610`)
- [x] SIGWINCH handling via `signal-hook` + `ResizeWatcher` (`todo.rs:775-820`)
- [x] `DaemonRequest::Resize` and daemon handler (`protocol.rs:166-175`, `right-now-daemon.rs:636-657`)
- [x] Integration test `test_attach_streams_output` (`right-now-daemon.rs:1284-1383`)
- [x] Integration test `test_resize_request_succeeds` (`right-now-daemon.rs:1385-1425`)

### Phase 4B: Attention Detection (Priority: High) — ✅ COMPLETE

**Files:**
- `src-tauri/src/session/attention.rs` — profiles, pattern matching, preview strategies
- `src-tauri/src/session/protocol.rs` — `AttentionType`, `AttentionSummary`, `DaemonNotification::Attention`
- `src-tauri/src/bin/right-now-daemon.rs` — `spawn_attention_monitor()`, `record_attention()`
- `src-tauri/src/bin/todo.rs` — displays `last_attention` in summaries

**Completed:**
- [x] Create `AttentionProfile` and `AttentionTrigger` structs (`attention.rs:58-97`)
- [x] Implement literal and regex matchers with case-insensitive option (`attention.rs:70-97`)
- [x] Hardcode Claude Code profile (`"✔ Submit"`, `"❯"`, `"Enter to select"`) (`attention.rs:9-34`)
- [x] Hardcode build tools profile (success/error patterns) (`attention.rs:35-53`)
- [x] Hook real-time pattern check into `PtyEvent::Output` via `spawn_attention_monitor()` (`right-now-daemon.rs:328-368`)
- [x] Implement preview extraction strategies (`LastLines`, `Surround`) (`attention.rs:99-131`)
- [x] Broadcast `DaemonNotification::Attention` with type + preview (`right-now-daemon.rs:383-389`)
- [x] Add `Session.last_attention` field and persist to registry (`protocol.rs:99-101`)
- [x] Debounce duplicate attention previews (`right-now-daemon.rs:344-350`)
- [x] Unit tests for detection logic (`attention.rs:170-197`)

**Deferred (low priority):**
- [ ] Add ANSI escape stripping (use `strip-ansi-escapes` crate) for cleaner pattern matching
- [ ] Add polling fallback for cross-chunk patterns (ring buffer scan every 5s)

### Phase 4C: Context Capture for Restart (Priority: Medium)
Enable restarting stopped sessions in same working directory.

**Files:**
- `src-tauri/src/session/protocol.rs` — add `last_known_cwd` to Session
- `src-tauri/src/bin/right-now-daemon.rs` — cwd capture in watch task
- `src-tauri/src/bin/todo.rs` — add `--restart` flag

**Tasks:**
- [ ] Add `last_known_cwd: Option<PathBuf>` to Session struct
- [ ] Capture cwd periodically via `/proc/{pid}/cwd` (Linux) or `lsof -p` (macOS)
- [ ] Implement `todo start --restart <session-id>`
- [ ] Spawn new PTY with `cd {cwd} && {original_command}`
- [ ] Show disclaimer: "Restarting session (process state not preserved)"

### Phase 4D: Environment Detection System (Priority: High)
Continuously detect what application/environment the session is running.

**Heuristics (in priority order):**
1. **Terminal title sequences** — OSC escape codes set window title (e.g., `\x1b]0;Claude Code\x07`)
2. **Process introspection** — command name, arguments
3. **Output markers** — specific patterns unique to each environment
4. **Recent activity** — if we saw Claude Code prompts recently, prioritize that

**Architecture:**
```rust
struct EnvironmentDetector {
    detected_env: Option<Environment>,
    confidence: f32,
    recent_markers: VecDeque<(Instant, MarkerType)>,
}

enum Environment {
    ClaudeCode,
    NpmBuild,
    CargoTest,
    GenericShell,
    Unknown,
}
```

**Tasks:**
- [ ] Create `EnvironmentDetector` struct
- [ ] Parse OSC title sequences from output stream
- [ ] Maintain sliding window of recent markers
- [ ] Score environments by marker frequency + recency
- [ ] When environment changes, log transition
- [ ] Prioritize attention patterns for detected environment

### Phase 4E: Notification System (Priority: High) — NEXT UP
Alert user when attention is needed via terminal escape codes and sound.

**Goal:** Close the "attention management" loop. The detection engine already fires `DaemonNotification::Attention`—now we need to *deliver* it to the user in a way that works even when they're not looking at the terminal.

**Approach:**
Emit terminal escape codes through the PTY output stream so the terminal emulator itself can notify. This ensures clicking the notification focuses the *terminal window*, not our Tauri app. Additionally, play a system sound for audible alerts.

#### Escape Codes to Emit

| Terminal | Escape Code | Notes |
|----------|-------------|-------|
| All | `\x07` (BEL) | Audio bell; many terminals flash or badge |
| iTerm2 | `\x1b]9;{message}\x07` | Native macOS notification |
| Konsole/VTE | `\x1b]777;notify;{title};{body}\x07` | Linux notification |
| kitty | `\x1b]99;i=1:d=0;{message}\x1b\\` | Cross-platform |
| tmux | `\x1bPtmux;\x1b\x1b]9;{message}\x07\x1b\\` | Passthrough wrapper |

**Implementation Strategy:**
1. Write escape codes to the PTY master (so attach clients see them)
2. Also emit via the broadcast channel (so non-attached UI can react)
3. Play sound asynchronously (don't block PTY I/O)

#### Files to Create/Modify

```
src-tauri/src/session/notify.rs (NEW)
├── emit_terminal_notification(message: &str) -> Vec<u8>
├── play_attention_sound() -> Result<()>
└── NotificationConfig { sound_enabled, escape_codes_enabled }

src-tauri/src/bin/right-now-daemon.rs
├── spawn_attention_monitor() — call notify after record_attention()
└── DaemonState.notification_config

src-tauri/src/session/mod.rs
└── pub mod notify;
```

#### Implementation Details

**`notify.rs` skeleton:**
```rust
use std::process::Command;

/// Build escape sequence bytes for terminal notifications
pub fn terminal_notification_bytes(title: &str, body: &str) -> Vec<u8> {
    let mut bytes = Vec::new();

    // BEL - universal attention signal
    bytes.push(0x07);

    // iTerm2 (OSC 9)
    bytes.extend_from_slice(format!("\x1b]9;{}: {}\x07", title, body).as_bytes());

    // Konsole/VTE (OSC 777)
    bytes.extend_from_slice(
        format!("\x1b]777;notify;{};{}\x07", title, body).as_bytes()
    );

    // kitty (OSC 99)
    bytes.extend_from_slice(
        format!("\x1b]99;i=1:d=0;{}: {}\x1b\\", title, body).as_bytes()
    );

    bytes
}

/// Play system attention sound (non-blocking)
pub fn play_attention_sound() {
    #[cfg(target_os = "macos")]
    {
        // Use system sound - "Ping" is subtle but audible
        let _ = Command::new("afplay")
            .arg("/System/Library/Sounds/Ping.aiff")
            .spawn();
    }

    #[cfg(target_os = "linux")]
    {
        // paplay for PulseAudio systems
        let _ = Command::new("paplay")
            .arg("/usr/share/sounds/freedesktop/stereo/message.oga")
            .spawn();
    }
}
```

**Integration in `spawn_attention_monitor()`:**
```rust
// After record_attention():
if state.notification_config.enabled {
    // Write escape codes to PTY so attached terminals see them
    if let Some(runtime) = handles.get(&session_id) {
        let notif_bytes = notify::terminal_notification_bytes(
            &matched.profile,
            &matched.preview,
        );
        let _ = runtime.input_sender().try_send(notif_bytes);
    }

    // Play sound (async, don't block)
    if state.notification_config.sound_enabled {
        notify::play_attention_sound();
    }
}
```

#### Debouncing Strategy

The current `last_preview` check prevents duplicate notifications for the same content. Add time-based debouncing:

```rust
struct AttentionDebouncer {
    last_notification: Option<Instant>,
    last_preview: Option<String>,
    min_interval: Duration,  // e.g., 5 seconds
}

impl AttentionDebouncer {
    fn should_notify(&mut self, preview: &str) -> bool {
        let dominated_by_time = self.last_notification
            .map(|t| t.elapsed() < self.min_interval)
            .unwrap_or(false);

        let same_preview = self.last_preview
            .as_ref()
            .map(|p| p == preview)
            .unwrap_or(false);

        if dominated_by_time && same_preview {
            return false;
        }

        self.last_notification = Some(Instant::now());
        self.last_preview = Some(preview.to_string());
        true
    }
}
```

#### Tasks

- [ ] Create `src-tauri/src/session/notify.rs` with escape code builder
- [ ] Add `play_attention_sound()` for macOS/Linux
- [ ] Integrate into `spawn_attention_monitor()` after `record_attention()`
- [ ] Add time-based debouncing (5s minimum between notifications for same session)
- [ ] Add `NotificationConfig` to `DaemonState` (allow disabling sound/escapes)
- [ ] Test with iTerm2, Terminal.app, kitty, and Alacritty
- [ ] Document escape code compatibility in README

#### Testing

```rust
#[test]
fn test_notification_bytes_contains_bel() {
    let bytes = terminal_notification_bytes("test", "hello");
    assert!(bytes.contains(&0x07));
}

#[test]
fn test_notification_bytes_contains_osc9() {
    let bytes = terminal_notification_bytes("test", "hello");
    let s = String::from_utf8_lossy(&bytes);
    assert!(s.contains("\x1b]9;"));
}
```

---

## Recommended Next Steps

### Immediate (Phase 4E: Notifications)
**Effort:** ~2 hours | **Impact:** High (closes attention loop)

The attention detection engine is complete—triggers fire and `DaemonNotification::Attention` is broadcast. The missing piece is *delivering* that attention to the user when they're not looking at the terminal.

1. Create `src-tauri/src/session/notify.rs` (~50 lines)
2. Add `terminal_notification_bytes()` and `play_attention_sound()`
3. Hook into `spawn_attention_monitor()` after `record_attention()`
4. Test with your terminal emulator of choice

### Short-term (Phase 4C: Context Capture)
**Effort:** ~4 hours | **Impact:** Medium (enables restart)

Capture working directory and environment so stopped sessions can be restarted with context.

1. Add `last_known_cwd: Option<PathBuf>` to `Session`
2. Periodically capture cwd via `lsof -p {pid} -Fn | grep ^n` (macOS) in watch task
3. Add `todo start --restart <session-id>` that spawns `cd {cwd} && {cmd}`
4. Store original `shell_command` in Session for replay

### Medium-term (Phase 5: Deep Links)
**Effort:** ~6 hours | **Impact:** Medium (cross-app integration)

Register `todos://` as an OS-level protocol so external apps (Raycast, Alfred, scripts) can open sessions.

1. Add `@tauri-apps/plugin-deep-link` dependency
2. Register scheme in `tauri.conf.json`
3. Handle incoming URLs in Rust, emit events to frontend
4. Parse `todos://session/{id}` and focus/attach

### Longer-term (Phase 6: Frontend Integration)
**Effort:** ~12 hours | **Impact:** High (full UI integration)

Surface session state in React, add tray badges, and provide UI controls.

1. Build `src/lib/sessions.ts` client that talks to daemon
2. Add session badges to `TaskList.tsx`
3. Update tray menu with Running/Waiting indicators
4. Wire Start/Stop/Continue buttons to daemon commands

### Low Priority (Polish)
These can be done anytime as quality-of-life improvements:

| Task | Effort | Notes |
|------|--------|-------|
| ANSI escape stripping | 1h | Add `strip-ansi-escapes` for cleaner pattern matching |
| Cross-chunk detection | 2h | Ring buffer polling every 5s for split patterns |
| Multi-client attach policy | 1h | Gate to one client or document concurrent behavior |
| `--json` for attach | 1h | Emit metadata even during raw streaming |
| Environment detection | 4h | OSC title parsing, process introspection (Phase 4D) |

---

## Decisions Summary

| Question | Decision | Status |
|----------|----------|--------|
| Failure state handling | `AttentionType::Error` variant | ✅ Implemented |
| Ring buffer persistence | In-memory only (accept loss on crash) | ✅ Implemented |
| Attach socket architecture | Per-session socket for cleaner separation | ✅ Implemented |
| Profile selection | Hardcoded defaults; auto-detection deferred | ✅ Implemented |
| Notification delivery | Terminal escape codes (BEL, OSC) + sound | 🔲 Phase 4E |
| Debouncing | Preview dedup done; time-based debounce in 4E | 🔲 Phase 4E |
| Restart flows | Capture cwd/env for `--restart` flag | 🔲 Phase 4C |
