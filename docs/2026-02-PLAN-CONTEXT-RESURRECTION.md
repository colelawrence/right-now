# Context Resurrection - Large-scale plan (Right Now)

- **Status:** Draft
- **Version:** v0.1
- **Last updated:** 2026-02-06
- **Audience:** Right Now maintainers (Tauri/React + Rust daemon + CLI).

> Document note: This plan contains **API sketches** and **schema sketches** that communicate intent.
> Treat them as design targets, not copy/paste-ready code.

## How to use this plan

1. Read **PART 0** end-to-end (it locks contracts, key decisions, and quality gates).
2. If you disagree with any ADR in **§0.4**, resolve it *before* implementation.
3. Implement in phases (see **PART IV**) and track progress in **PART X: Master TODO Inventory**.

---

# PART 0 - Executive Blueprint (high-leverage layer)

## 0.1 Executive summary

**Context Resurrection (CR)** reduces "return-to-work" reorientation from ~15-30 minutes to ~30-60 seconds by showing a **Resurrection Card** that reconstructs:

- what you were working on (task intent)
- what happened in your terminal session (tail + attention state; later AI briefings)
- your note to future self (human context)
- editor breadcrumbs (file list; later full editor restoration)

CR is designed to keep working even if the **UI was closed**, as long as the **daemon/CLI sessions were running**.

## 0.1.1 Non-negotiables (engineering contract)

1. **Local-first:** snapshots and briefings stay on-device.
2. **Transparent storage:** data is discoverable + deletable by the user.
3. **No clobber:** external edits to TODO.md must never be lost due to CR.
4. **Stable task identity:** CR state must survive task renames.
5. **Works without UI running:** daemon can capture enough context to resurrect later.
6. **Fast path is deterministic:** v1 works without AI; AI is strictly additive.
7. **Non-blocking UX:** CR never traps the user behind a modal they can't dismiss.
8. **Sensitive data awareness:** terminal captures must not inadvertently store secrets (see §0.13).

## 0.2 Mission and non-goals

### Mission

Make task re-entry effortless: *"I open Right Now and instantly know what to do next."*

### Non-goals (explicitly out of scope for v1)

- Not a git client or "what changed" diff view.
- Not a backup / disaster recovery system.
- Not full terminal replay/recording.
- Not a full IDE inside Right Now.
- Not cross-device sync.

## 0.3 Primary user scenarios (v1)

1. **Monday morning return:** "What was I doing on Friday?"
2. **Meeting interruption:** quick re-entry after an hour away.
3. **Task switching:** bouncing between two active tasks.
4. **CLI-only sessions:** sessions run while UI is closed; open app later and resurrect.

## 0.4 Key decisions locked early (ADRs)

### ADR-001 - Stable Task IDs in markdown

- **Decision:** Add a stable task id token to each task line.
- **Format:** `[abc.derived-label]` (3 lowercase letters + dot + derived label; 4-letter prefix allowed as collision fallback)
  - Example: `- [ ] Fix API timeout bug [qdz.fix-api-timeout-bug]`
- **Placement:** End-of-task-line **before** session badge (if present).
  - `- [ ] Fix API timeout bug [qdz.fix-api-timeout-bug] [Running](todos://session/42)`
- **Why:** Renames must not sever history.
- **Consequences:** TS + Rust parsers and writers must preserve the token.
- **Tests:** Round-trip parsing + update routines must keep IDs stable.

### ADR-002 - Explicit active-task pointer

- **Decision:** Persist an explicit active task id in frontmatter: `right_now.active_task_id`.
- **Why:** "first incomplete task" inference is not reliable for resurrection.
- **Tests:** External edits + watcher reload must not erase the pointer accidentally.

### ADR-003 - Daemon is the capture authority (for CR storage)

- **Decision:** The daemon owns the on-disk **Context Resurrection store**.
- **Why:** CR must work with UI closed; daemon already owns PTY sessions and can capture on transitions.
- **Consequence:** UI needs a thin client to query + request capture/note updates.

### ADR-004 - Storage format: file-based snapshots (not SQLite)

- **Decision:** Store snapshots as JSON + optional compressed terminal tail files under `~/.right-now/`.
- **Why:** Deletable, transparent, low ceremony.
- **Consequence:** Cross-snapshot search may require a lightweight index later.

### ADR-005 - v1 briefings are non-AI

- **Decision:** v1 uses deterministic "briefing" content: terminal tail + last attention summary + exit status.
- **Why:** Avoid packaging/model complexity until the UX proves valuable.

## 0.5 Architecture (layered)

### 0.5.1 Three-ring decomposition

1. **Kernel (daemon CR core):** capture + storage + retention + query.
2. **UI surfaces (Tauri/React):** Resurrection Card presentation + triggers + note UX.
3. **Integrations (optional extras):** VS Code extension, local LLM briefings.

### 0.5.2 Module / package layout (target)

Rust (daemon):
- `src-tauri/src/session/` (existing) - PTY + attention + attach
- `src-tauri/src/context_resurrection/` (new)
  - `store.rs` - paths + file IO + retention
  - `models.rs` - snapshot schema
  - `capture.rs` - capture routines (from session runtime + timers when available)
  - `query.rs` - list/latest/get APIs

TypeScript (UI):
- `src/lib/context-resurrection/`
  - `types.ts`
  - `client.ts` - daemon RPC wrapper
  - `selectors.ts` - derive "what to show" for the card

Markdown parsing:
- Extend `src/lib/ProjectStateEditor.ts`
- Extend `src-tauri/src/session/markdown.rs`

### 0.5.3 Intra-module boundaries (Rust daemon)

**context_resurrection module internal contracts:**

| File | Responsibility | Consumes | Exposes |
|------|---------------|----------|---------|
| `models.rs` | Schema types only | — | `ContextSnapshotV1`, `CaptureReason`, `SnapshotId` |
| `store.rs` | Disk I/O, paths, atomic writes, retention | `models.rs` | `SnapshotStore` struct: `write()`, `read()`, `list()`, `delete()`, `prune()` |
| `capture.rs` | Build snapshot from runtime state + sanitize | `models.rs`, `store.rs`, `session::*` (via trait) | `CaptureService`: `capture_now()` |
| `query.rs` | RPC handlers for CR requests | `store.rs` | `handle_cr_request()` |

**session → context_resurrection interface:**

`capture.rs` must not reach into `session/` internals directly. Define a trait in `context_resurrection/`:

```rust
// In context_resurrection/capture.rs or a separate session_provider.rs
pub trait SessionProvider: Send + Sync {
    fn get_session_state(&self, session_id: u64) -> Option<SessionSnapshot>;
}

pub struct SessionSnapshot {
    pub status: SessionStatus,
    pub exit_code: Option<i32>,
    pub last_attention: Option<AttentionSummary>,
    pub tail: String, // unsanitized; capture.rs sanitizes
}
```

`session/` module implements `SessionProvider`. This inverts the dependency: CR depends on an abstraction, not on session internals.

### 0.5.4 TypeScript client boundaries

| File | Responsibility | Consumes | Exposes |
|------|---------------|----------|---------|
| `types.ts` | Request/response types, `ContextSnapshotV1` | — | All CR-related types |
| `client.ts` | Daemon IPC transport only | `types.ts` | `CrClient`: `latest()`, `list()`, `get()`, `captureNow()`, `deleteTask()`, `deleteProject()` |
| `selectors.ts` | Derive UI display state from snapshots | `types.ts` | `selectCardData(snapshot): CardDisplayData`, `shouldShowCard(snapshot, lastActivity): boolean` |

**Dependency rule:** React components import from `selectors.ts`, never directly from `client.ts` for data shaping.

## 0.6 Core invariants

1. **Task ID token is preserved** by all markdown edits (UI and daemon).
2. **Session badge updates never alter task IDs**.
3. **Snapshot capture is idempotent** per (task_id, reason, approx timestamp window).
4. **Retention is enforced** (default: last 5 per task; prune old completed tasks).
5. **UI can boot without daemon**, but CR features degrade gracefully (no crash, clear messaging).

## 0.7 Quality gates (stop-ship)

### Data correctness

- Task ID parsing/writing is round-trip safe (TS + Rust agree).
- External edits to TODO.md do not remove IDs or active pointer.
- Snapshot store never corrupts; partial writes are not observable (atomic write pattern).

### UX correctness

- Resurrection Card appears within **< 200ms** after project load (for local snapshots) for typical projects.
- Card is dismissible; does not block editing/working.
- "Resume" is reliable: attaches to session if running, otherwise offers restart/continue.

### Operational

- Works when UI was closed: open UI, see last daemon-captured snapshot.
- Snapshot retention prevents unbounded growth.

## 0.8 Public UX surface (v1)

### Resurrection Card (v1)

Shown when:
- App opens and there is a recent snapshot for the active task (or most recent task) and the last activity is older than a threshold (default: 60 minutes), OR
- User clicks a "Resume from snapshot" affordance on a task.

Card shows (best-effort):
- Task title
- "Last active" (from snapshot timestamp)
- Terminal section:
  - Session id + status
  - Last attention preview (if any)
  - Tail excerpt (e.g. last 20 lines)
- Your note to future self (if present)
- Editor breadcrumbs (list of files) - optional
- Actions:
  - **Resume work** (attach if running; otherwise `continue` view)
  - **Add note** (quick inline input)
  - **View details** (raw tail, snapshot history)
  - **Dismiss**

### Task affordances (v1)

- A task with snapshots shows a subtle "has context" indicator.
- If a task has a running/waiting session badge, use that as the primary "resume" entry.

## 0.9 Protocol/API surface (target)

Extend `DaemonRequest`/`DaemonResponse` for CR:

```rust
enum DaemonRequest {
  // ... existing

  CrLatest { project_path: String, task_id: Option<String> },
  CrList { project_path: String, task_id: Option<String>, limit: Option<usize> },
  CrGet { snapshot_id: String },

  // Capture entry points
  CrCaptureNow {
    project_path: String,
    task_id: String,
    reason: CaptureReason, // enum; see §1.2 for variants
    user_note: Option<String>,
    // editor_state: Option<...> (future)
  },

  // Deletion entry points (per §0.13.3)
  CrDeleteTask { project_path: String, task_id: String },
  CrDeleteProject { project_path: String },

  // Optional helpers
  SetActiveTask { project_path: String, task_id: String },
}
```

### 0.9.1 TypeScript client contract (must mirror Rust)

`src/lib/context-resurrection/types.ts` must export matching types:

```ts
// Request types (sent by UI)
type CrLatestRequest = { type: "CrLatest"; project_path: string; task_id?: string }
type CrListRequest = { type: "CrList"; project_path: string; task_id?: string; limit?: number }
type CrGetRequest = { type: "CrGet"; snapshot_id: string }
type CrCaptureNowRequest = {
  type: "CrCaptureNow"
  project_path: string
  task_id: string
  reason: CaptureReason
  user_note?: string
}
type CrDeleteTaskRequest = { type: "CrDeleteTask"; project_path: string; task_id: string }
type CrDeleteProjectRequest = { type: "CrDeleteProject"; project_path: string }

// Response types (received by UI)
type CrLatestResponse = { snapshot: ContextSnapshotV1 | null }
type CrListResponse = { snapshots: ContextSnapshotV1[] }
type CrGetResponse = { snapshot: ContextSnapshotV1 | null }
type CrCaptureResponse = { snapshot_id: string }
type CrDeleteResponse = { deleted_count: number }
```

**Contract enforcement:** Add a shared JSON Schema or integration test that asserts TS and Rust types serialize identically.

**Error contract (daemon unavailable):**

```ts
type CrError =
  | { type: "daemon_unavailable" }
  | { type: "not_found"; snapshot_id?: string }
  | { type: "io_error"; message: string }

// CrClient methods return Result-style:
type CrResult<T> = { ok: true; value: T } | { ok: false; error: CrError }
```

UI must handle `daemon_unavailable` gracefully: show "Daemon not running" state, do not crash. CR features simply become unavailable until daemon connects.

Notes:
- `SetActiveTask` is provided for future CLI-based task switching; the daemon reads but does not write frontmatter (per §0.12.1). UI remains the primary writer for `right_now.active_task_id`.
- Snapshot payloads should keep terminal tail sizes bounded; store large tails as files.

## 0.10 Definition of done (milestone v1)

A "v1 CR" is done when:

- Task lines contain stable IDs in the specified format.
- Frontmatter contains explicit `right_now.active_task_id`.
- Daemon captures snapshots on at least:
  - session stop
  - session transitions (running ↔ waiting)
  - periodic idle timeout (configurable)
- Terminal tail sanitization runs before every capture (per §0.13.1).
- Snapshot files have correct permissions (`0600`/`0700` per §0.13.2).
- UI can show a Resurrection Card for the active task, sourced from daemon snapshots.
- User can add a note to a snapshot via UI (stored in snapshot store).
- User can delete snapshots per-task and per-project (per §0.13.3).
- Snapshot retention is implemented and tested.

## 0.11 Performance budgets

- Snapshot write: p95 < 50ms (metadata-only) on local disk.
- Snapshot tail size: default max 10KB raw text for UI display; larger stored compressed.
- UI card compute: < 10ms per render for typical snapshot.
- `CrList` query: p95 < 100ms for projects with up to 100 tasks × 5 snapshots each (500 snapshot files). For larger projects, callers should provide a `limit` parameter to bound scan time.

## 0.11.1 Logging and observability (CR module)

**Logging levels:**
- `error`: Store initialization failure, atomic write failure, permission errors.
- `warn`: Duplicate task ID prefixes detected, orphan temp file cleanup limit hit, sanitization pattern matched (first occurrence per session only).
- `info`: Capture completed (task_id, reason, snapshot_id).
- `debug`: Capture skipped (rate limit/dedup), missing tail_path on read, pruning actions.

**Counters (for future metrics integration):**
- `cr_captures_total{reason}`: Successful captures by reason.
- `cr_captures_dropped_ratelimit`: Captures dropped due to rate limit.
- `cr_sanitization_redactions`: Lines redacted by sanitization.

**Owner:** Daemon team adds structured logging in `store.rs` and `capture.rs`.

## 0.12 Risks / unknowns

- **Multi-writer coordination:** UI + daemon both editing TODO.md fields (active task pointer + badges) can race. **Mitigation:** see §0.12.1.
- **Task ID rollout:** Introducing IDs requires careful "auto-assign" flow without surprising users.
- **Terminal semantics:** Extracting "last command" reliably is hard; v1 should not promise it.
- **Daemon availability:** UI needs good fallback when daemon isn't running. **Mitigation:** UI shows empty state with "Daemon not running" message; core task management remains functional.
- **Storage initialization failure:** If `~/.right-now/context-resurrection/` cannot be created (disk full, permissions), CR features must fail gracefully. **Mitigation:** see §0.12.2.

### 0.12.1 Multi-writer coordination (architectural decision)

**Problem:** Both UI (Tauri frontend) and daemon write to `TODO.md` (UI: active task pointer in frontmatter, task edits; daemon: session badges). Concurrent writes can cause data loss.

**Decision for v1:** Daemon is the sole writer for badge fields; UI is the sole writer for all other fields.

- **Badge updates:** Daemon updates badges based on session lifecycle/state transitions and writes atomically. UI never writes badges.
- **Active task pointer:** UI writes frontmatter directly. Daemon reads but never writes `right_now.active_task_id`.
- **Task content edits:** UI only. Daemon never modifies task text or IDs.

**Consequence:** If user edits TODO.md externally while daemon is updating a badge, file watcher reload may see a stale badge. This is acceptable for v1 (badges are ephemeral; next session event corrects it).

**Future:** Consider a daemon-owned lock file or single-writer architecture if conflicts become problematic.

### 0.12.2 Storage initialization failure

**Problem:** `~/.right-now/context-resurrection/` may fail to create (disk full, permissions denied, home dir unset).

**Behavior:**
1. On daemon startup, attempt to create base directory with `0700` permissions.
2. If creation fails, log an `error` and set internal flag `cr_store_available = false`.
3. While `cr_store_available = false`:
   - All capture requests are silently dropped (log at debug level).
   - All query requests return empty results (not errors).
   - UI receives `daemon_unavailable`-equivalent responses for CR endpoints.
4. Daemon does not crash; non-CR functionality continues.
5. On successful capture (later), retry directory creation once. If it succeeds, set `cr_store_available = true`.

**Consequence:** CR degrades gracefully; user sees "no context available" rather than crashes.

**Owner:** Daemon team implements availability flag in `store.rs`.

## 0.13 Sensitive data handling

### 0.13.1 Terminal tail sanitization

Terminal output frequently contains secrets (API keys, passwords, tokens, connection strings). CR must avoid persisting these.

**v1 approach (heuristic scrubbing):**

1. Before storing `tail_inline` or writing `tail_path`, run a sanitizer that:
   - Redacts lines matching common secret patterns (e.g., `API_KEY=`, `password:`, `Bearer `, `-----BEGIN`).
   - Replaces matched content with `[REDACTED]`.
2. Sanitizer runs in daemon (`capture.rs`) before any disk write.
3. Sanitization is best-effort; users should be informed that sensitive data *may* still leak.

**Patterns to redact (initial set):**
- Environment variable assignments for known secret names (`API_KEY`, `SECRET`, `TOKEN`, `PASSWORD`, `PRIVATE_KEY`).
- Bearer tokens (`Bearer [A-Za-z0-9._-]+`).
- PEM blocks (`-----BEGIN.*PRIVATE KEY-----`).
- AWS-style keys (`AKIA[A-Z0-9]{16}`).

**Owner:** Daemon team implements `sanitize_terminal_output()` in `capture.rs`.

### 0.13.2 Storage permissions

Snapshot files must not be world-readable:
- Daemon sets file mode `0600` (owner read/write only) on all files under `~/.right-now/context-resurrection/`.
- Directory mode `0700` for `~/.right-now/` and subdirectories.

**Owner:** Daemon team ensures `atomic_write` helper applies correct permissions.

### 0.13.3 User deletion controls

Users must be able to delete CR data:
- **Per-task:** "Forget this task's context" action deletes `~/.right-now/context-resurrection/snapshots/<project-hash>/<task-id>/`.
- **Per-project:** "Forget project context" deletes `~/.right-now/context-resurrection/snapshots/<project-hash>/`.
- **Global:** User can delete `~/.right-now/context-resurrection/` entirely.

UI provides the first two via explicit actions; the third is documented for power users.

---

# PART I - Design details / specs

## 1.1 Task ID parsing/writing rules

### Syntax

- Token: `[abc.derived-label]`
  - `abc` = 3 lowercase letters (or 4 as collision fallback), **must be unique within the file**.
  - `derived-label` = slug derived from task text at creation time; never automatically changes.

### 1.1.1 Cross-language parsing contract (TS ↔ Rust parity)

**Critical invariant:** TS and Rust parsers must extract identical task IDs from the same input and produce identical output when writing.

**Canonical regex (both languages must use equivalent):**
```
\[([a-z]{3,4})\.([a-z0-9-]+)\]
```
- Group 1: prefix (3 or 4 lowercase letters)
- Group 2: derived-label (lowercase alphanumeric + hyphens)

**Placement within task line (ordered):**
1. Checkbox: `- [ ]` or `- [x]`
2. Task text
3. Task ID token: `[abc.derived-label]`
4. Session badge (optional): `[Running](todos://session/42)`
5. End of line

**Contract enforcement:**
- Add a shared test fixture file: `test/fixtures/task-id-parsing.md`
- TS test: parse fixture, assert extracted IDs match expected JSON
- Rust test: parse same fixture, assert extracted IDs match same expected JSON
- Both tests run in CI; failure = contract broken

### Generation rules

- On first time the UI sees a task without an ID, it can offer:
  - "Assign IDs to tasks" (one-time action), or
  - silently assign when user interacts with the task (recommended to avoid mass diff churn).

**Collision avoidance (required):**
1. Generator maintains a set of existing 3-letter prefixes in the file.
2. New prefix is generated randomly; if collision, regenerate (up to 10 attempts).
3. If 10 attempts fail (file has >17,000 tasks or extreme bad luck), extend to 4 letters for that ID only.
4. On file parse, if duplicate prefixes are detected, log a warning but do not auto-fix (user may have manually edited).

**Owner:** TS `ensureTaskId()` implements collision check; Rust parser validates uniqueness on load.

### Preservation rules

- Renaming a task must keep its ID token unchanged.
- Session badge updates must not remove or reorder the ID token.

## 1.2 Snapshot schema (v1)

```ts
type SnapshotId = string // e.g. "2026-02-06T13:12:33Z_qdz.fix-api-timeout-bug"

type CaptureReason =
  | "session_stopped"
  | "session_waiting"
  | "session_running"
  | "idle_timeout"
  | "manual"

type ContextSnapshotV1 = {
  id: SnapshotId
  version: 1

  project_path: string // absolute TODO.md path
  task_id: string
  task_title_at_capture: string

  captured_at: string // ISO8601
  capture_reason: CaptureReason

  // Terminal context (best effort)
  terminal?: {
    session_id: number
    status: "Running" | "Waiting" | "Stopped"
    exit_code?: number
    last_attention?: {
      attention_type: "input_required" | "decision_point" | "completed" | "error"
      preview: string
      triggered_at: string
    }

    // Either inline tail or file reference (sanitized per §0.13.1)
    tail_inline?: string
    tail_path?: string
  }

  user_note?: string

  // Reserved for later
  editor?: unknown
}
```

## 1.3 Storage layout

Under daemon base dir (currently `~/.right-now/` on macOS):

```
~/.right-now/
  context-resurrection/
    snapshots/
      <project-hash>/
        <task-id>/
          <snapshot-id>.json
          <snapshot-id>.term.gz (optional)
    index/
      <project-hash>.json (optional, for future cross-snapshot search)
```

**Project hash:** SHA-256 of the canonical absolute path to `TODO.md`, truncated to 16 hex chars. This avoids leaking path structure while maintaining uniqueness.

### 1.3.1 Atomic write protocol

To prevent partial/corrupt snapshots observable by readers:

1. Write content to a temp file in the same directory: `<snapshot-id>.json.tmp.<pid>`.
2. `fsync` the temp file.
3. Rename temp file to final name (atomic on POSIX).
4. Set file mode `0600` before or immediately after rename.

**Consequence:** Readers that see a `.json` file can trust it is complete.

**Startup cleanup:** On daemon startup, delete stale `.tmp.*` files older than 1 hour. Cleanup is bounded: scan at most 1000 files per project-hash directory to avoid blocking startup. Log a warning if limit is hit; remaining orphans cleaned on next startup.

**Missing tail_path handling:** On snapshot read, if `tail_path` is set but file does not exist, treat `tail_path` as `null` and log a debug-level warning. Do not fail the read.

### 1.3.2 Retention and pruning

- Pruning happens per (project, task-id).
- Default retention: last 5 snapshots per task.
- **Pruning must not race with capture:** pruning acquires a lightweight lock (flock on a `.lock` file in the task-id directory) before deleting. Capture acquires the same lock before writing.
- Completed tasks: snapshots are retained for 7 days after last capture, then pruned entirely.

### 1.3.3 Concurrent capture coordination

Multiple capture triggers can fire in quick succession (e.g., session_stopped + idle_timeout race, or rapid state transitions). To prevent duplicate/corrupt writes:

1. **Per-task capture lock:** All captures acquire the same flock used by pruning (`.lock` file in task-id directory) before writing. Lock acquisition timeout: 500ms. If timeout expires, drop the capture request and log at `warn` level.
2. **Deduplication window:** After a successful capture, skip subsequent captures for the same (task_id, reason) within a 5-second window. Tracked in-memory by `CaptureService`.
3. **Rate limit:** At most 1 capture per task per 2 seconds, regardless of reason. Requests during cooldown are dropped (not queued). Log at debug level when dropped.

**Owner:** Daemon team implements dedup + rate limit in `capture.rs`.

## 1.4 Capture triggers (daemon)

Minimum daemon triggers for v1:

- On session status change:
  - `Running → Waiting`
  - `Waiting → Running`
  - any → `Stopped`
- On idle polling loop (existing 5s loop): if a session has been idle for N minutes (default 10), capture a snapshot.

Optional (later):
- system sleep notification hook
- task switch capture (requires active-task pointer integration)

## 1.5 Resurrection selection algorithm (UI)

When project loads:

1. Determine active task:
   - Use `right_now.active_task_id` if present and points to an existing task.
   - Else choose "most recently captured snapshot" task.
2. Query daemon: `CrLatest { project_path, task_id }`
3. Show Resurrection Card if:
   - snapshot exists AND
   - time since snapshot > threshold OR user explicitly invoked "resume".

## 1.6 UX for notes (v1)

- Add Note is an inline textarea in the card.
- Saving a note sends `CrCaptureNow` with `reason: "manual"` and `user_note` set. This creates a new snapshot with the note attached (simpler than updating existing snapshots).

---

# PART IV - Execution phases (with exit criteria)

## Phase 0 - Markdown identity foundation

**Outcome:** stable task IDs + active task pointer exist and are safe.

Exit criteria:
- TS parser/writer supports task IDs and preserves them.
- Rust parser/writer supports task IDs and preserves them.
- Cross-language parity test passes: shared fixture file parsed by both TS and Rust produces identical results (per §1.1.1).
- Tests cover:
  - round-trip
  - session badge update preserves task ID
  - moving heading sections preserves IDs

## Phase 1 - Daemon snapshot store + capture

**Outcome:** daemon writes snapshots and can serve latest/list/get.

Exit criteria:
- Snapshot store directory created + atomic writes (per §1.3.1).
- File/directory permissions enforced (`0600`/`0700` per §0.13.2).
- Terminal tail sanitization implemented (per §0.13.1).
- Capture on session transitions works.
- Retention/pruning implemented with flock coordination (per §1.3.2).
- Concurrent capture coordination implemented (dedup + rate limit per §1.3.3).
- Storage initialization failure handling implemented (per §0.12.2).
- Unit tests for store + retention + sanitization + concurrent capture.

## Phase 2 - UI Resurrection Card (deterministic, no AI)

**Outcome:** user can see "where they left off" with terminal tail and attention summary.

Exit criteria:
- Card shows on project open (when eligible) and on explicit action.
- Dismiss + Details works.
- Resume action attaches (existing CLI attach path) or provides a clear next step.

## Phase 3 - Notes to future self

**Outcome:** user notes are captured and displayed prominently.

Exit criteria:
- UI can save note by capturing a new snapshot with `reason: "manual"` (per §1.6).
- Notes persist and are deletable.

## Phase 4 - Editor breadcrumbs (lightweight)

**Outcome:** file list restore is "credible but basic".

Exit criteria:
- UI can store a small file list (manual add or heuristics).
- "Open files" action uses `code --goto` / platform open (best effort).

## Phase 5 - AI briefings (post-v1, optional)

**Outcome:** pluggable briefings provider; local model or BYO-key later.

**Note:** Per ADR-005, v1 uses deterministic briefings only. This phase is post-v1 and optional.

Exit criteria:
- Provider interface exists.
- Off by default; can be enabled.
- Clear disclosure about data flow.

---

# PART X - Master TODO inventory

## X.1 Markdown task IDs + active task pointer

- [ ] Decide exact parsing grammar for `[abc.derived-label]` token (slug rules, allowed chars)
- [ ] TS: extend `ProjectStateEditor.parseBody` to extract task IDs
- [ ] TS: extend `ProjectStateEditor.update` to preserve task IDs on writes
- [ ] TS: add helper `ensureTaskId(task)` with collision avoidance per §1.1:
  - [ ] collect existing 3-letter prefixes
  - [ ] retry up to 10 times on collision
  - [ ] fall back to 4-letter prefix if exhausted
- [ ] Rust: extend `session/markdown.rs` task parsing to extract task IDs
- [ ] Rust: validate task ID prefix uniqueness on parse; log warning on duplicates
- [ ] Rust: ensure badge update preserves task IDs
- [ ] Add tests in TS and Rust for ID preservation under:
  - [ ] badge updates
  - [ ] heading moves
  - [ ] task rename
- [ ] Add tests for collision avoidance (mock random generator to force collisions)
- [ ] Add cross-language parity test fixture (`test/fixtures/task-id-parsing.md`) per §1.1.1:
  - [ ] Create fixture with edge cases (IDs with hyphens, 3 vs 4-letter prefixes, badges present/absent)
  - [ ] Add TS test: parse fixture, compare to expected JSON
  - [ ] Add Rust test: parse same fixture, compare to same expected JSON

## X.2 Daemon CR module

- [ ] Add `src-tauri/src/context_resurrection/mod.rs`
- [ ] Implement `ContextSnapshotV1` model in `models.rs`
- [ ] Define `SessionProvider` trait in CR module (per §0.5.3); session module implements it
- [ ] Implement `SnapshotStore` in `store.rs`:
  - [ ] paths (with SHA-256 project hash, 16 hex chars)
  - [ ] atomic writes (temp file + fsync + rename pattern per §1.3.1)
  - [ ] file permissions (`0600` for files, `0700` for directories per §0.13.2)
  - [ ] retention/pruning with flock coordination per §1.3.2
  - [ ] list/latest/get
  - [ ] delete (per-task, per-project)
  - [ ] availability flag (`cr_store_available`) with graceful degradation per §0.12.2
  - [ ] startup orphan cleanup (bounded to 1000 files per project-hash per §1.3.1)
  - [ ] missing `tail_path` handling (treat as null, log debug warning per §1.3.1)
- [ ] Implement `CaptureService` in `capture.rs`:
  - [ ] `capture_now()` using `SessionProvider` trait (not session internals)
  - [ ] `sanitize_terminal_output()` per §0.13.1
  - [ ] per-task capture lock (flock) per §1.3.3
  - [ ] deduplication window (5s same task_id+reason) per §1.3.3
  - [ ] rate limit (1 capture per task per 2s) per §1.3.3
- [ ] Add structured logging per §0.11.1 (error/warn/info/debug levels)
- [ ] Implement `handle_cr_request()` in `query.rs`
- [ ] Implement capture hooks in daemon session watcher
- [ ] Add protocol requests/responses for CR (including `CrDeleteTask`, `CrDeleteProject`)
- [ ] Add unit tests for store + retention
- [ ] Add unit tests for terminal sanitization (redacts known secret patterns)
- [ ] Add unit tests for concurrent capture coordination (dedup, rate limit)
- [ ] Implement `SessionProvider` trait in `session/` module (per §0.5.3):
  - [ ] Add `impl SessionProvider for SessionManager` (or equivalent)
  - [ ] Ensure CR module only uses trait, never reaches into session internals

## X.3 UI Resurrection Card

- [ ] Add `src/lib/context-resurrection/types.ts` with:
  - [ ] Request/response types mirroring Rust (per §0.9.1)
  - [ ] `CrError` and `CrResult<T>` types (per §0.9.1 error contract)
- [ ] Add `src/lib/context-resurrection/client.ts` to call daemon:
  - [ ] Implement `CrClient` with methods per §0.5.4
  - [ ] Return `CrResult<T>` to handle `daemon_unavailable` gracefully
- [ ] Add `src/lib/context-resurrection/selectors.ts`:
  - [ ] `selectCardData(snapshot): CardDisplayData`
  - [ ] `shouldShowCard(snapshot, lastActivity): boolean`
- [ ] Add React component `ResurrectionCard`
- [ ] Add eligibility logic on project load
- [ ] Add task list indicator + explicit "Resume from snapshot" action

## X.4 Notes

- [ ] UI: inline note editor in card
- [ ] Daemon: persist note by capturing a new snapshot with `reason: "manual"` (per §1.6)

## X.5 Operational + docs

- [ ] Document storage location and deletion controls (per §0.13.3)
- [ ] UI: Add "Forget this task's context" action (deletes `<project-hash>/<task-id>/`)
- [ ] UI: Add "Forget project context" action (deletes `<project-hash>/`)
- [ ] Document manual deletion of `~/.right-now/context-resurrection/` for power users
- [ ] Add smoke test checklist
- [ ] Add security/privacy section to user-facing docs:
  - [ ] Explain what data is captured (terminal output, file lists, notes)
  - [ ] Explain sanitization is best-effort; advise not to paste secrets into terminals
  - [ ] Explain how to delete data

---

# Appendix A — Open questions (tracked)

- ~~Should daemon accept `SetActiveTask` and update frontmatter, or should UI remain the only writer for frontmatter?~~ **Resolved in §0.12.1:** UI is the sole writer for frontmatter; daemon reads only.
- How to surface CR when daemon is not running? (Graceful empty state vs "start daemon" CTA) — **Partially resolved in §0.12:** empty state with messaging; CTA decision deferred.
- Should task IDs be assigned lazily (on interaction) vs bulk assignment?
