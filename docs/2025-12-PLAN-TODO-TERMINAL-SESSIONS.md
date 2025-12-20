# Global TODO Session Management - Implementation Plan

## Implementation Progress

### ✅ Phase 1: Foundation & Persistence (COMPLETE)
**Files created:**
- `src-tauri/src/session/mod.rs` - Module entry point
- `src-tauri/src/session/protocol.rs` (259 lines) - Session protocol structs, message types, serialization
- `src-tauri/src/session/persistence.rs` (242 lines) - Session registry with file locking, atomic writes
- `src-tauri/src/session/config.rs` (219 lines) - Environment config, socket/PID path helpers
- `src-tauri/src/bin/right-now-daemon.rs` (343 lines) - Daemon binary skeleton with socket listener
- `src-tauri/src/bin/todo.rs` (395 lines) - CLI binary with start/stop/list/continue commands

**Key implementation notes:**
- Uses Unix domain sockets at `~/.right-now/daemon.sock`
- Sessions persisted to `~/.right-now/sessions.json` with `flock` file locking
- Atomic writes via temp file + rename pattern
- Environment override via `RIGHT_NOW_DAEMON_DIR` for testing

### ✅ Phase 2: Markdown Parser & Shared Utilities (COMPLETE)
**Files modified/created:**
- `src/lib/ProjectStateEditor.ts` - Added session badge parsing (`SESSION_BADGE_RE`, `TaskSessionStatus`, `formatSessionBadge()`)
- `src-tauri/src/session/markdown.rs` (593 lines) - Rust markdown parser mirroring TypeScript implementation
- `src/lib/__tests__/ProjectStateEditor.test.ts` - Extended with 48 tests for session badges

**Test coverage:**
- 43 Rust tests for markdown parsing (various formats, special characters, edge cases)
- 48 TypeScript tests for session badge round-trips
- Both parsers agree on badge format: `[Running|Stopped|Waiting](todos://session/<id>)`

**Bug fixes during implementation:**
- Fixed stringify to preserve original task prefix (was hardcoding `- [...]` instead of using `block.prefix`)
- Fixed `p: undefined` in react-markdown components prop causing "Element type is invalid" error

**Testing infrastructure added:**
- `npm run test` script added to package.json (runs `bun test`)
- `npm run typecheck` script added (runs `tsc --noEmit`)
- `src/__tests__/imports.test.ts` - Import validation tests to catch undefined exports early
- Total: 54 TypeScript tests, 43 Rust tests (97 total)

### ✅ Phase 3: Daemon Session Flow (COMPLETE)
**Goal:** Promote `src-tauri/src/bin/right-now-daemon.rs` from a registry stub into the process that actually launches PTYs, rewrites Markdown, and keeps `sessions.json` + the TODO file in sync.

**Files created/modified:**
- `src-tauri/src/session/runtime.rs` (418 lines) - PTY runtime wrapper using `portable-pty`
- `src-tauri/src/bin/right-now-daemon.rs` (775 lines) - Full daemon with PTY lifecycle, markdown updates, output watchers

**Implementation completed:**
- Created `PtyRuntime` struct that wraps `portable-pty`:
  - `spawn(session_id, shell)` - Spawns PTY child process with default or custom shell
  - `send_input(data)` - Async input to PTY via mpsc channel
  - `recv_event()` - Async event receiver for output, idle, exit events
  - `stop()` - Graceful shutdown with signal propagation
  - Ring buffer (64KB) for recent output storage
  - Activity tracking with 30-second idle timeout
- Updated `DaemonState` to hold `HashMap<SessionId, PtyRuntime>` instead of placeholder
- Start handler implementation:
  - Reads TODO file, parses via `parse_body()`, validates task exists
  - Spawns PTY with provided shell or default `$SHELL`
  - Updates markdown with `[Running](todos://session/<id>)` badge
  - Atomic write via temp file + rename
  - Broadcasts `SessionUpdated` notification
- Stop handler implementation:
  - Stops PTY process via `pty.stop()`
  - Updates markdown badge to `[Stopped]`
  - Persists registry and broadcasts update
- Output watcher task (`watch_pty_output`):
  - Polls every 5 seconds for idle/exit status
  - Transitions Running -> Waiting after 30s idle
  - Handles exit by updating to Stopped status
  - Updates both registry and markdown on each transition

**Testing:**
- 5 integration tests in daemon binary:
  - `test_start_session_updates_markdown` - Verifies markdown gets session badge
  - `test_stop_session_updates_markdown` - Verifies badge changes to Stopped
  - `test_start_nonexistent_task_fails` - Error handling for missing tasks
  - `test_start_duplicate_session_fails` - Prevents duplicate sessions
  - `test_list_sessions` - Lists all active sessions
- 2 unit tests in runtime module:
  - `test_ring_buffer` - Ring buffer capacity and tail retrieval
  - `test_spawn_echo` - PTY spawn with echo command

**Total test count:** 53 Rust tests (48 lib + 5 daemon)

**Follow-ups identified during review (ALL RESOLVED):**
- ~~Badge rewrites still rely on prefix matching~~ → Fixed: `update_task_session_in_content` now uses exact case-insensitive match instead of prefix matching
- ~~Persisted sessions survive daemon restarts~~ → Fixed: Added `reconcile_stale_sessions()` at daemon startup that marks all Running/Waiting sessions as Stopped and updates markdown badges
- ~~Markdown updates clobber concurrent edits~~ → Fixed: Created `update_markdown_badge()` helper that reads fresh content immediately before writing; all badge update paths now use this
- ~~SessionRegistry::find_by_task_key uses prefix matching~~ → Fixed: Changed to exact case-insensitive match so tasks with similar names (e.g., "Build feature" vs "Build feature - backend") can both have sessions
- ~~update_markdown_badge doesn't detect failed updates~~ → Fixed: Added `UpdateResult` struct with `task_found` field; `update_markdown_badge` now returns error if task not found in markdown
- ~~New defect surfaced: PTY channel deadlock~~ → Fixed: Changed `blocking_send` to `try_send` in both reader and wait tasks (`runtime.rs:209,214,274,284`). Ring buffer and activity timestamp are always updated regardless of event channel consumption, so dropping events when channel is full is safe.
- ~~Exit codes are never persisted~~ → Fixed: `PtyRuntime` now caches the child exit code, `watch_pty_output` propagates it into `Session.exit_code`, and stop requests reset it.
- ~~todo continue opens two daemon connections~~ → Fixed: `DaemonRequest::Continue` accepts `tail_bytes` and returns the ring-buffer replay inline, eliminating the extra Tail RPC.
- ~~Tail output unavailable after PTY exit~~ → Fixed: completed sessions retain their final ring-buffer snapshot in-memory so later Tail/Continue responses can show the final output even after handles are dropped.

### ✅ Phase 4: CLI Attach & Attention UX (COMPLETE)
**Goal (see `PLAN_PTY_SESSION_RESUMPTION.md`):** Deliver trustworthy session resumption, output replay, and "attention needed" cues so users can treat `todo continue` like `tmux attach`.

#### What's shipped

**Attach system (Phase 4A):**
- **Attach protocol + sockets** — `DaemonRequest::Attach` returns `DaemonResponse::AttachReady` with a per-session Unix socket under `~/.right-now/attach-<id>.sock`. The daemon keeps a listener alive per running session, streams PTY output through broadcast receivers, and sends detach/exit notices before closing the socket.
- **Live streaming** — Attach connections receive live PTY output and can send input (including control bytes). The runtime switched to a broadcast event channel so multiple observers can subscribe without starving the PTY reader.
- **Replay support** — Attach responses include an optional tail buffer. The CLI prints metadata first, then dumps the exact raw bytes (with a `[Replaying last N bytes]` banner) before switching to live streaming.
- **Raw-mode CLI** — `todo continue --attach <id>` enables raw mode via `crossterm`, forwards stdin to the attach socket, and mirrors PTY output to stdout. Ctrl-\\ cleanly detaches, Ctrl-C and other control codes are forwarded to the PTY instead of killing the CLI.
- **Dynamic resizing** — The CLI watches for SIGWINCH via `signal-hook`, sends `DaemonRequest::Resize`, and the daemon resizes the PTY via `portable-pty` so attached shells follow the user's terminal dimensions instead of staying fixed at 80×24.
- **Daemon safeguards** — Attach is rejected for stopped sessions, exit codes propagate into `Session`, stale ring buffers stay available for non-interactive `continue`, and we keep storing completed tails so metadata views stay useful.

**Attention detection (Phase 4B):**
- **Attention engine** — `session/attention.rs` provides `AttentionProfile` + `AttentionTrigger` structs with regex/literal matchers and `PreviewStrategy` (LastLines, Surround).
- **Default profiles** — Claude Code (`✔ Submit`, `❯`, `Enter to select`) and build-tools (`error:`, `build succeeded`) patterns hardcoded.
- **Real-time detection** — `spawn_attention_monitor()` subscribes to `PtyEvent::Output` and runs patterns on each chunk.
- **Debouncing** — Duplicate previews are suppressed to avoid notification spam.
- **Persistence + broadcast** — `Session.last_attention` is persisted; `DaemonNotification::Attention` is broadcast to subscribers.
- **CLI integration** — `todo continue` displays attention info in session summaries.

**Tests:**
- `test_attach_streams_output` — attach sockets echo input/output
- `test_resize_request_succeeds` — resize requests propagate to PTY
- `attention.rs` unit tests — literal/regex detection, preview rendering

**Terminal notifications (Phase 4E):**
- **Notification module** — `session/notify.rs` provides terminal notification codes and sound playback.
- **Terminal escape codes** — Emits BEL (`\x07`), OSC 9 (iTerm2), OSC 777 (Konsole/VTE/Gnome), OSC 99 (kitty) when attention triggers fire.
- **Sound playback** — Plays system sounds via `afplay` (macOS) or `paplay`/`aplay` (Linux) with attention-type-specific sounds (error, completion, input required).
- **Time-based debouncing** — `NotificationDebouncer` prevents notification spam with 5-second cooldown per session, in addition to preview deduplication.
- **Tests** — 6 unit tests in `notify.rs` covering debouncer logic, OSC escaping, and preview truncation.

#### Deferred to Phase 4C (Context Capture)
1. **Restart scaffolding** — Persist `shell_command`, `last_known_cwd`, and `env_snapshot` in the registry to enable restart flows.

#### Lower priority polish (can be done anytime)
- Multi-client attach semantics (gate to one client or allow concurrent via broadcast)
- ANSI escape stripping for cleaner pattern matching (`strip-ansi-escapes` crate)
- Cross-chunk pattern detection via polling fallback (ring buffer scan every 5s)
- `--json` flag for attach metadata output

#### Testing coverage
- `test_attach_streams_output` covers attach socket I/O round-trip
- `test_resize_request_succeeds` covers resize propagation
- `attention.rs` unit tests cover pattern detection and preview rendering
- Future: CLI smoke tests with `assert_cmd` for `--attach` flows

#### Next steps (see `PLAN_PTY_SESSION_RESUMPTION.md` for details)

| Priority | Phase | Task | Effort |
|----------|-------|------|--------|
| ✅ Done | 4E | Terminal notifications (BEL, OSC) + sound | — |
| 🔴 Immediate | 4C | Context capture for restart (cwd, env) | 4h |
| 🟡 Short-term | 5 | Deep link OS registration | 6h |
| 🟢 Medium-term | 6 | Frontend session integration | 12h |

### 🔲 Phase 5: Deep Link Plumbing (PARTIAL)
**Implemented:**
- `src/lib/links/handler.ts` - Link type detection and handling
- `src/lib/links/types.ts` - LinkType definitions including `todos://` protocol
- `src/components/markdown/` - Markdown rendering with clickable links
- Relative path resolution for file links

**Not yet implemented:**
- `@tauri-apps/plugin-deep-link` integration
- OS-level `todos://` scheme registration
- Rust deep link handler
- React deep link bridge (currently just dispatches DOM events)

**Goal:** Promote the existing in-app `todos://` handling to a real OS-level protocol so clicking a deep link launches the Tauri app, routes through Rust, and ultimately focuses/attaches to the correct session.

**Next actions:**
- Add `@tauri-apps/plugin-deep-link` to `package.json` and `tauri-plugin-deep-link` to `src-tauri/Cargo.toml`, then declare the plugin + schemes inside `tauri.conf.json`. Initialize it in `src-tauri/src/lib.rs`, buffer incoming URLs until the window is ready, and emit `tauri::Event`s that the front-end can subscribe to.
- Create a proper bridge in TypeScript (`src/lib/deeplinks.ts`): subscribe to the plugin (via `@tauri-apps/api/event`), parse `todos://session/<id>`, and invoke the soon-to-be-built session service (`focusSession`, `attachSession`). Remove the temporary DOM `CustomEvent`.
- Publish platform-specific registration steps (macOS `open -Ra`, Linux `.desktop` + `xdg-settings`, Windows registry) as part of the release checklist so QA can verify OS plumbing.

**Testing & docs:**
- Extend the existing `links` utilities tests (or add `src/lib/__tests__/deeplinks.test.ts`) to assert that malformed URLs fail gracefully and valid ones trigger the right session command.
- Document a manual smoke test: run `open 'todos://session/42'` (mac) / `xdg-open todos://session/42` (linux) / `start todos://session/42` (windows later) and confirm the app routes to that session.

### 🔲 Phase 6: Frontend Integration (NOT STARTED)
**Goal:** Surface live session data inside React + the tray while still treating TODO.md as the single source of truth.

**Implementation to-dos:**
- Build `src/lib/sessions.ts`: a tiny client that talks to the daemon via a new Tauri command (Rust can just proxy `session::protocol`) or, during early development, shells out to the bundled `todo` binary. Expose hooks/atoms such as `useSessions()`, `useSessionByTask(task)`, and actions like `startSession`, `stopSession`, `attachSession`.
- Teach `ProjectManager` (src/lib/project.ts) to reconcile parsed `task.sessionStatus` with push updates coming from the daemon so the UI updates immediately instead of waiting for the file watcher.
- Update `TaskList.tsx` to render session badges (component that shows Running/Waiting/Stopped colors), expose CTA buttons (Start, Continue, Stop), and fire commands through the session service rather than editing Markdown directly.
- Refresh the tray integration (`src/lib/windows.ts`) so it displays `[Running]` or `[Waiting]` next to tasks, offers quick actions (Continue/Stop), and keeps the path to the current TODO file for the "Edit" shortcut.
- Add lightweight notifications or status toasts when the daemon returns errors (e.g., “session already running”) so UI users know to resolve duplicates.

**Testing:**
- Add Vitest/React Testing Library coverage for the session hook and badge component to ensure we render the right CTA/state transitions.
- Provide a mocked-daemon integration test that feeds synthetic `DaemonNotification::SessionUpdated` events through the bridge and asserts the tray/task list update without writing to disk.

### 🔲 Phase 7: Packaging & Postinstall (NOT STARTED)
**Goal:** Ship both binaries + the GUI in a bundle that installs cleanly, registers deep links, and provides a `todo` CLI on PATH.

**Implementation to-dos:**
- Update `tauri.conf.json` so the bundle includes the compiled `todo` + `right-now-daemon` binaries (list them under `bundle.resources`) and make `todo.rs` spawn the bundled daemon instead of calling `cargo run`.
- Extend `script/postinstall-setup.mts` (or add a new installer helper) to place a `todo` symlink in `/usr/local/bin` or `~/.local/bin` on mac/Linux, emit guidance for Windows (scoop/choco/manual PATH), and respect opt-out flags for users who do not want PATH modifications.
- Write release docs covering environment variables (`RIGHT_NOW_DAEMON_DIR`, `RIGHT_NOW_SHELL`), deep-link verification, and manual recovery steps if the daemon socket gets wedged.
- Notarize/sign macOS builds, produce at least one Linux artifact (AppImage or .deb), and track Windows requirements even if support comes later.

**Testing & verification:**
- Add CI coverage that runs `bun test`, `npm run typecheck`, `cargo test`, and `cargo tauri build` for macOS + Linux so regressions get caught before packaging.
- Script a smoke test that installs the built app into a temp dir, links `todo`, runs `todo start/stop` against a sample TODO file, and proves the bundled daemon handles the flow without developer tools present.

---

## Overview
We will add a daemon-driven terminal session workflow that keeps TODO Markdown files as the single source of truth. The daemon (not the Tauri UI) edits TODO files atomically whenever a session changes state, and every other surface—including the React UI, tray, and CLI—reacts to file changes or daemon push events. CLI tooling (`todo`) boots/communicates with the daemon, sessions are monitored via PTYs, and deep links (`todos://session/<id>`) let any surface jump directly to an active session. macOS/Linux have priority, but we will call out the seams that need Windows implementations later.

The UI already uses a `FileWatcher` (`src/lib/project.ts`) that reloads the Markdown file when it changes, so keeping all Markdown mutations inside the daemon keeps the UI in sync automatically. The UI (and potentially automated flows) can still *request* updates by sending instructions to the daemon instead of editing files directly.

```
todo CLI <-> Unix socket <-> right-now-daemon ----(atomic write)----> TODO.md
                                                            |
                                               FileWatcher reload (ProjectManager)
                                                            |
                                                      React UI / Tray
```

## Architecture Snapshot

| Component | Responsibility | Mac/Linux | Windows Notes |
|-----------|----------------|-----------|---------------|
| `right-now-daemon` (`src-tauri/src/bin/right-now-daemon.rs`) | Own session registry, spawn PTYs via `portable-pty`, update Markdown, expose socket protocol, persist session metadata to `$APPDATA/right-now/sessions.json`. | Use Unix sockets in `$HOME/.right-now/daemon.sock`. | Later swap socket backend to named pipes and WinPTY; leave adapter traits in place now. |
| `todo` CLI (`src-tauri/src/bin/todo.rs`) | User-facing commands (`start`, `continue`, `list`, `stop`). Ensures daemon is running, streams daemon replies. | Connect via Unix socket. | Later: named pipes, Windows-specific binary install path. |
| React UI (`src/main.tsx`, `src/components/TaskList.tsx`, `src/lib/sessions.ts`) | Displays badges/deep links, issues daemon commands via bridge module, reacts to file reloads. | Uses existing watcher + new session service. | Same code; needs Windows socket path once daemon supports it. |
| Deep-link plumbing (`src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs`, `src/lib/deeplinks.ts`) | Register `todos` scheme, propagate URLs to React, map to session actions. | `@tauri-apps/plugin-deep-link` + Rust plugin init. | Works once Tauri plugin supports Windows (already does). |
| Store (`src/lib/store.ts`) | Continues tracking recent projects/timing. Sessions no longer live here, but helper APIs (e.g., `rememberSessionPreferences`) can be added later. | n/a | n/a |

## Markdown Session Encoding

Tasks gain an optional trailing session badge immediately before any details block:

```
- [ ] Implement reports [Running](todos://session/42)
```

* Rules:
  * `todos://session/<id>` deep link is the canonical source of the numeric session id. We rely on IDs in Markdown rather than human names for reliability.
  * `status` is capitalized (`Running`, `Waiting`, `Stopped`). The daemon is responsible for updating the word to match the session state.
  * Additional Markdown content (links, emphasis) can still exist in `task.name` before the badge. If the user includes a bracketed link that is not `todos://`, we treat it as part of the name.
  * Details/notes (`task.details`) stay on lines below and the daemon never mutates them.

* Parser updates (`src/lib/ProjectStateEditor.ts`):
  * Extend `TASK_RE` capture groups to isolate the badge via a targeted regex like `(?:\s+\[(Running|Stopped|Waiting)\]\((todos://session/\d+)\))?`.
  * Augment `ProjectMarkdown` with `sessionStatus?: { status: "Running" | "Stopped" | "Waiting"; sessionId: number }`.
  * Ensure stringify logic reconstructs the badge if `sessionStatus` is set; otherwise leave the line unchanged. Preserve user whitespace by storing the original single-space separator where possible.

* CLI/app task addressing:
  * For user-friendly targeting, `todo start "build pipeline"` resolves tasks by matching the initial case-insensitive words of `task.name`. If multiple matches exist we return them all and require the user to specify the deep link ID.
  * Because IDs are persisted in Markdown, all surfaces can reconnect even after restarts as long as the daemon reads the file to rebuild its registry.

## Daemon Responsibilities

1. **Lifecycle**
   * Launched on demand: CLI tries to connect to the socket; on failure, it spawns the daemon binary from the app bundle (`tauri::api::process::Command`). The daemon exits when no sessions and no clients remain for N minutes (configurable).
   * PID + socket path recorded under `$HOME/.right-now/{daemon.pid, daemon.sock}` (mac) or `%APPDATA%\\Right Now\\` on Windows (future). The CLI uses those files for discovery.

2. **Session Registry & Persistence**
   * In-memory map `{sessionId -> Session}` with metadata (task ID, TODO path, status, PTY PID, timestamps).
   * Persisted to `$APPDATA/right-now/sessions.json` on each mutation so CLI/daemon can crash and reconnect. File uses `serde_json` with `advisory-lock` (on Unix: `flock`) to prevent concurrent writers. For Windows, leave TODO to use `CreateFile` locking.

3. **Markdown editing**
   * Whenever a session starts/stops, the daemon parses the TODO file through a shared library module (Rust port mirroring `ProjectStateEditor` rules) and rewrites only the affected task line. Use write-to-temp + `std::fs::rename` to ensure atomicity so the UI watcher sees a single change event.
   * Because the daemon is SoT, the UI never edits session badges directly; it instead invokes daemon commands for start/stop or uses `todo` CLI scaffolding.

4. **PTY management**
   * Use `portable-pty` + `tokio` to spawn shells (default to `$SHELL` or `/bin/zsh`). For Windows later, annotate TODO to swap in `portable-pty`’s Windows backend or `conpty`.
   * Monitor stdout for heuristics (“Build succeeded”, idle timer, exit code) to transition statuses from `Running` -> `Waiting` -> `Stopped`.

5. **IPC**
   * Listen on Unix socket with framed JSON messages:
     ```json
     {"type":"start","taskKey":"Implement reports","projectPath":"/path/TODO.md","shell":["/bin/zsh","-lc","npm run dev"]}
     ```
     Responses echo `sessionId`, status, and terminal attach info.
   * Broadcast incremental updates (e.g., notify all clients when a session’s status changes). The UI session service subscribes to this stream to render badges while still reloading the Markdown file for permanence.

## CLI Binary (`todo`)

Commands:
```
todo start <task words> [--project <path>] [--cmd "<shell command>"]
todo continue <session-id>
todo list [--project <path>]
todo stop <session-id>
```

Implementation notes:
* Binary lives in `src-tauri/src/bin/todo.rs`. It shares protocol structs with the daemon via a `session_protocol` module in `src-tauri/src/session`.
* On connect failure it spawns the daemon binary (also bundled) and retries with exponential backoff.
* Output formatting: `list` groups sessions by TODO file and shows `[42] Implement reports — Running — todos://session/42`.
* For commands triggered from the UI (e.g., “Start Session” button), the frontend can either shell out to `todo` (via `Command`) or talk directly to the daemon using the same protocol via a new Tauri command (preferred to avoid shell quoting issues).

## UI/React Integration

1. **Session service (`src/lib/sessions.ts`)**
   * Wraps daemon protocol in a small TS client using `@tauri-apps/api/tauri` commands. Maintains a Jotai atom keyed by `sessionId`.
   * Exposes helpers like `useSessionForTask(taskKey)` so `TaskList` can show status badges or “Continue” buttons.

2. **Project state sync**
   * ProjectManager already refreshes after file writes. After the daemon updates a task, the UI re-parses the Markdown and sees the new status plus deep link data automatically.

3. **Task list rendering (`src/components/TaskList.tsx`)**
   * Extend UI to show a badge next to each task that has `sessionStatus`. Clicking the badge invokes the deep link (opens CLI attach) or surfaces a context menu.
   * Provide fallback text when multiple tasks share the same starting words (since we rely on that for CLI detection).

4. **Tray updates (`src/lib/windows.ts`)**
   * When `ProjectManager` publishes tasks, include session info so tray menu items can show `[Running]` or `[Waiting]`.

## Deep Link Handling

1. **Configuration (`src-tauri/tauri.conf.json`)**
   * Add `"plugins": {"deep-link": {"desktop": {"schemes": ["todos"]}}}` and include `@tauri-apps/plugin-deep-link` in `devDependencies`.

2. **Rust bootstrap (`src-tauri/src/lib.rs`)**
   * Register `tauri_plugin_deep_link::init()` (mac/Linux now, Windows later) and hook its callback to emit a window event (`todos://session/42`).
   * Expose `#[tauri::command] fn invoke_deep_link(url: String)` so the frontend can simulate deep links (useful for tests).

3. **Frontend listener (`src/lib/deeplinks.ts`)**
   * Subscribe to the plugin, parse `todos://session/<id>`, and route to `sessions.focus(sessionId)` which attaches to the terminal or surfaces the right task.

4. **CLI integration**
   * `todo list` prints the deep link so other apps (like Raycast) can open sessions. On mac/Linux we can `open todos://session/<id>`; on Windows future work may require `start todos://...`.

## Packaging & Distribution

* Extend `src-tauri/Cargo.toml` with `[[bin]]` entries for both binaries. Use a shared workspace module (`src-tauri/src/session/mod.rs`) for protocol structs to avoid duplication.
* Ship the binaries inside the Tauri bundle (`bundle.resources`) and add a post-installation script (`script/postinstall-setup.mts`) that symlinks `todo` into `~/.local/bin` (Linux) or `/usr/local/bin` (mac). Allow the user to skip linking.
* Document environment variables: `RIGHT_NOW_DAEMON_DIR` to override socket/pid locations (useful for tests) and `RIGHT_NOW_SHELL` to override the default shell.
* When Windows support lands, update installers to add `todo.exe` to `%LOCALAPPDATA%\\Microsoft\\WindowsApps` or instruct users to run `scoop install`.

## Dependencies to Add

**Rust (`src-tauri/Cargo.toml`):**
- `portable-pty` — PTY support (mac/Linux now; confirm Windows backend status).
- `tokio` (full + `rt-multi-thread`, `macros`, `signal`) — async runtime for socket + PTY piping.
- `interprocess` — cross-platform IPC (Unix socket + named pipe abstraction).
- `serde`, `serde_json`, `anyhow`, `thiserror` — protocol/persistence ergonomics.
- `notify` or `tauri-plugin-fs` reuse for file monitoring within daemon if needed (UI already watches, but daemon may watch TODO files to detect manual edits).

**TypeScript (`package.json`):**
- `@tauri-apps/plugin-deep-link` — deep link bridge.
- (optional) `superjson` or similar if we need richer serialization for daemon bridge.

## Implementation Phases

1. **Foundation & Persistence**
   * Create daemon project skeleton with socket listener, JSON protocol structs, and persistence file (no PTY yet).
   * Add environment configuration helpers for socket paths on mac/Linux and stub Windows constants with TODO comments.

2. **Markdown Parser & Shared Utilities**
   * Update `src/lib/ProjectStateEditor.ts` to understand session badges.
   * Add a Rust counterpart module (`src-tauri/src/session/markdown.rs`) to avoid reimplementing parsing logic by hand; keep them in sync via tests against sample Markdown fixtures (`docs/examples/session-markdown.md`).

3. **Daemon Session Flow**
   * Implement `start/stop/list/continue` handlers, PTY launching, output monitoring, status transitions, and Markdown rewriting.
   * Write integration tests (Rust) that run against temp TODO files to ensure badges are idempotent.

4. **CLI UX**
   * Build `todo` binary with nice help text, human-friendly search, and ability to attach to PTY (e.g., spawn `tokio::io::copy` between stdin/stdout and daemon-provided PTY endpoint).
   * Add `todo --json` flag for automation.

5. **Deep Link Plumbing**
   * Add plugin dependency/config, register handler in Rust, and add TS listener. Verify `open todos://session/123` focuses the right session from both CLI and UI.

6. **Frontend Integration**
   * Build `src/lib/sessions.ts`, update `TaskList`, tray menu, and add UI affordances (e.g., start session button).
   * Wire UI actions to send commands to the daemon via a new Tauri command (Rust simply proxies socket messages to avoid reimplementing the protocol in JS).

7. **Packaging & Postinstall**
   * Add binaries to bundle, update postinstall script to expose `todo`, and document manual steps in `README.md`.

## Notes & Future Work

- **Windows parity:** Keep abstractions around sockets/PTYs behind traits so we can plug in Windows transports later without rewriting all logic. Leave `TODO(windows)` comments where platform-specific work remains (socket path resolution, symlink install steps, PTY backend, file locking).
- **Security:** Consider authenticating socket clients (e.g., restrict permissions on the socket file). For MVP, limit access by file permissions inside the user’s config directory.
- **Session metadata in UI store:** If later we need to show session history even when TODO files disappear, we can extend `ProjectStore` with read-only helpers fed by daemon snapshots, but it is out of scope for the initial implementation.
- **Future panel:** After the CLI/daemon flow is solid, we can add an in-app session panel for richer terminal output streaming (`Phase 5.2` in the original plan).
