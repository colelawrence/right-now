# Right Now — Holistic Roadmap (Local‑first + Terminal Sessions)

**Version:** 0.2 (2026-02-05)

**Intent:** Identify the biggest gaps between the current codebase and a cohesive, user-complete product, then propose an execution plan that connects the dots across:

- Local-first UX (Welcome / Planner / Tracker / Tray)
- Markdown file fidelity + multi-writer safety
- Terminal session daemon + UI surfaces (start/continue/attention)
- Packaging hardening (ship the right sidecars; stable release builds)

**Audience:** contributors/agents working in this repo.

**How to use this plan**

1. Read **Part 0** (Executive Blueprint) first. It defines the contract, non-goals, and the “shape” of the product.
2. Skim **Part I** (Gap Analysis) to see what’s missing.
3. Implement by **Phases** (Part IV), using the **Quality Gates** as stop-ship criteria.
4. Convert the **Master TODO Inventory** (Part X) into whatever tracker we’re using.

> Note on backward compatibility: this repo is internal/experimental; we do **not** need to preserve backwards compatibility for settings/state files or on-disk formats unless a task explicitly asks for it.

---

## PART 0 — Executive Blueprint

### 0.1 Executive summary

Right Now is evolving into a **local-first focus system**:

- A Markdown task file (`TODO.md`) is the source of truth.
- The UI provides two modes:
  - **Planner**: a readable, glanceable lens on the TODO file
  - **Tracker**: a minimal always-available “what I’m doing right now” bar
- A Pomodoro-like state machine (planning → working → break) is reinforced through audio cues.
- A local daemon (`right-now-daemon`) can run **PTY terminal sessions bound to tasks**, update the TODO file with session badges, and notify you when output “needs attention”.

The next step product gap is **coherence**: stitch these parts into complete user flows (open → plan → execute → resume), with reliable session controls and better persisted timer state.

### 0.1.1 Non-negotiables (engineering/product contract)

1. **The file is authoritative.** If we write, we must **read fresh from disk first** and write **atomically**.
2. **No silent clobbers.** If the file is invalid/mid-edit, do not “fix” it by overwriting; surface an error and retry on next change.
3. **Planner is intentionally not a full editor.** The app should not become a markdown IDE. Most editing happens in the user’s preferred editor.
   - We *do* allow a small set of “Now” actions: complete task, state transitions, session control.
4. **We can break persisted formats.** Avoid migrations unless explicitly requested.
5. **One active project at a time** in the UI (unless/until multi-project becomes deliberate).

### 0.2 Mission and non-goals

**Mission**

- Make “the next real thing” (current task + timer + session) always visible and frictionless.
- Keep data in **plain text** (markdown) so it stays portable and editable.
- Treat terminal work as first-class: sessions can detach/resume and reflect status back into the TODO.

**Non-goals (for the next major iteration)**

- Building a full task editor (create/delete/inline markdown authoring) inside the app.
- Real-time collaboration / login / shared presence rooms.
- Perfect backward compatibility for frontmatter/settings formats.
- Multi-platform parity beyond what we explicitly prioritize.

### 0.3 Product surfaces (what needs to exist)

Core local-first surfaces:

- **Welcome / Project picker** (recent projects, create/open, error recovery)
- **System walkthrough** (make the model explicit: “edit in your editor; app reacts”)
- **Planner** (readable list + minimal actions)
- **Tracker** (minimal control; current task; timer; session status)
- **Tray menu** (glanceable current task + quick actions)
- **Settings** (sound packs, editor integration, CLI status)

Terminal session surfaces:

- **Session controls inline with tasks** (Start/Continue/Stop)
- **Session list** (global view; see running/waiting/stopped + attention)
- **Deep link routing** (`todos://session/<id>` focuses the app and lands in the right place)

### 0.4 Layered architecture (keep the core small)

**Ring 1: Local kernel (must stay boring & reliable)**

- Markdown parse/update (lossless where possible)
- ProjectManager (serialized ops, watcher lifecycle, atomic writes)
- Timer logic (pure functions + event bus)

**Ring 2: Local UX/services**

- Windows (planner/tracker sizing/styling)
- Tray integration
- Sound playback
- Session client that talks to the daemon

**Ring 3: Optional extras (deferred)**

- Cloud sync / connected mode (explicitly out of scope right now)

### 0.5 Invariants to keep testable

- Any file mutation uses **read-fresh → update → atomic write**.
- Watcher reloads are **debounced** and **coalesced**, and cannot “teleport” to a prior project.
- If parsing fails, the UI keeps the last good model and shows a non-destructive error state.
- Session badges round-trip correctly in both TS and Rust parsers.

### 0.6 Decisions to lock early (ADRs)

These are churn magnets; decide explicitly before large implementation:

1. **Task identity contract**: name-only matching vs adding stable IDs (and how visible that is in the markdown).
2. **Reorder semantics**: if we reorder, do we reorder whole sections/headings only, never individual tasks?
3. **Session “attach UX”**: open external terminal vs embed a terminal view in-app (v0.x should start with external).
4. **Durable timer state schema**: what we persist in frontmatter (state + startedAt + endsAt) vs what remains ephemeral.

### 0.7 Quality gates (stop-ship)

Local-first gates:

- **No clobber regression tests:** external edits + daemon badge updates + UI actions can interleave without losing data.
- **Project switch safety:** watchers/reloads cannot revive old projects.
- **Crash safety:** atomic writes; no partial file writes.

Sessions gates:

- UI can control sessions without relying on “edit the markdown badge directly”.
- Deep links reliably focus the app and land on the correct session.
- Daemon-not-running errors produce actionable UX (not silent failures).

### 0.8 Definition of Done (two milestones)

**DoD: Local v0.2 (cohesive solo flow)**

- Welcome screen shows recent projects; can open/create without a file dialog every time.
- The app clearly explains its model (walkthrough/help): “edit in your editor; Right Now reacts”.
- Work/break state can persist across restarts (durable timestamps).
- Sessions are visible (badges rendered) even if control is minimal.

**DoD: Sessions v0.3 (daemon feels like a feature)**

- Task list shows session status and Start/Continue/Stop CTAs.
- Session list surface exists.
- `todos://session/<id>` deep links route to a meaningful action.
- Packaging is stable: release builds ship only intended binaries.

---

## PART I — Gap analysis (current app vs cohesive product)

### 1.1 What works today (high confidence)

- Markdown-backed tasks with headings and details.
- Completion toggling persists to disk.
- Planner + Tracker UI modes.
- Pomodoro state machine + timer + warning sound events.
- Tray shows current task context and a few next tasks.
- Local daemon + CLI session system exists (PTY, detach/attach, attention detection, badge updates).
- File watcher infrastructure exists (directory watch, coalesced reload, serialized ops).

### 1.2 Gaps (onboarding / “how it works”)

- Welcome screen is minimal and doesn’t teach the mental model.
- Startup still drives a file dialog flow.
- Users aren’t explicitly told that the Planner is a **lens** and that editing should happen in their editor.

### 1.3 Gaps (local-first timer/state)

- WorkState/stateTransitions are mostly ephemeral today; restart loses timer state.
- Time tracking is specified in docs but not implemented.

### 1.4 Gaps (sessions as a first-class feature)

- UI has no session list, no session controls, no attach UX.
- Deep links are received/parsed but do not route to a session surface.
- Errors (daemon not running, socket unavailable, duplicates) are not surfaced in UI.

### 1.5 Gaps (packaging / release ergonomics)

- Release bundling can be fragile (accidental test-harness coupling, sidecar discovery issues).
- Need a repeatable smoke checklist/script for release artifacts.

### 1.6 Explicitly out of scope (for now)

- Connected mode / login / “connect to each other”.

---

## PART II — Target user flows (what “cohesive” looks like)

### 2.1 Local-first flow (solo)

1. Launch app → **Welcome**
2. Select a recent project or “Open…”
3. Welcome/Help explains:
   - “Your TODO.md is the source of truth”
   - “Edit tasks in your editor; Right Now will update live”
   - “Use Right Now for focus + sessions + lightweight actions”
4. **Planner** shows tasks; user hits Start → app switches to **Tracker**
5. During work:
   - timer counts down
   - tray shows current task
   - warnings/sounds fire
6. User completes the current task (in app) or edits the file in their editor (outside) → app reacts
7. Quit app → reopen later and **resume state**

### 2.2 Terminal session flow (solo)

1. In Planner, tasks show session status (if any).
2. User clicks Start Session (or uses CLI) → daemon updates TODO.md badge.
3. UI shows Running/Waiting/Stopped and any attention indicators.
4. User clicks Continue/Attach → opens external terminal attach flow (v0.x).
5. Attention triggers → UI and/or tray indicates and deep link can jump to the session.

---

## PART III — Technical approach (connecting the dots)

### 3.1 Durable state in frontmatter

Persist enough to restore timer state after restart:

- `current_state`: planning/working/break
- `state_transition.started_at`: timestamp
- `state_transition.ends_at`: timestamp (optional)

Notes:

- We can change schema freely (no migration work required).
- Keep a strict separation:
  - **durable state** lives in the file
  - **ephemeral UI state** (window mode, last warning timestamps) lives in memory/store

### 3.2 Walkthrough / model education

Add a minimal, non-annoying walkthrough that can be dismissed:

- First launch: show a 3–5 step explanation.
- Accessible later via a “Help / How it works” button.

Content should explicitly call out:

- Edit tasks in your editor (VS Code, Vim, etc.)
- Right Now auto-reloads and is safe with external writers
- Sessions: badges appear in the TODO and deep links can open sessions

Persist “seen tour” in `ProjectStore` (breaking changes are acceptable).

### 3.3 Editor ergonomics (without becoming an editor)

Prioritize fast context switches:

- “Open TODO file” (already exists) but make it *prominent*.
- Optionally add:
  - reveal in Finder
  - copy path
  - (later) open at current task line (if we adopt stable task identity)

### 3.4 Reorder semantics (optional, nuanced)

If we implement reorder, prefer **section-level moves**:

- Allow moving an entire heading section up/down, including its tasks and intervening unrecognized markdown.
- Avoid per-task reordering unless we commit to stable task identity and a well-defined preservation rule.

### 3.5 Session service (UI ↔ daemon)

Implement a thin `SessionClient` layer:

- `listSessions()`
- `startSession(taskKey)`
- `stopSession(sessionId)`
- `continueSession(sessionId, { attach?: boolean })`

Bridge daemon events into the app’s EventBus and/or reactive atoms.

### 3.6 Packaging hardening

- Release builds must not require building the test harness.
- Sidecar binaries must be bundled intentionally and discoverable.
- Provide a scripted smoke check for the produced app bundle.

---

## PART IV — Execution phases (with exit criteria)

### Phase 0 — Foundation stabilization

Outcome:
- Multi-writer safety stays correct under bursty file events and session badge rewrites.

Exit criteria:
- E2E test(s) cover: external file edit → watcher reload → no clobber.
- Invalid frontmatter shows a non-destructive error UI and recovers.

### Phase 1 — Welcome + walkthrough

Outcome:
- Opening projects is fast and the mental model is explicit.

Exit criteria:
- Welcome screen shows recent projects.
- Auto-load last active project if it exists.
- Walkthrough/help exists and can be re-opened.

### Phase 2 — Persisted work/break state (and initial time tracking)

Outcome:
- Restarting the app does not lose timer progress.

Exit criteria:
- Work/break state and timestamps persist to file.
- Timer resumes correctly after restart.

### Phase 3 — Sessions in UI (daemon becomes user-visible)

Outcome:
- Sessions are controllable from UI surfaces.

Exit criteria:
- Task list shows session status + Start/Continue/Stop CTAs.
- Session list surface exists.
- Deep link `todos://session/<id>` routes to a meaningful action.

### Phase 4 — Packaging + distribution hardening

Outcome:
- Builds are predictable and smoke-tested.

Exit criteria:
- `bun run tauri build` succeeds on a clean target without building rn-test-harness.
- Release bundle does not contain rn-test-harness.
- Smoke script validates the bundle contents and a basic session workflow.

---

## PART X — Master TODO inventory (task specs)

> These tasks are written to be small and verifiable (Context → Done means → Verification → RGR).

### A) Welcome + onboarding

#### A1. Welcome screen lists recent projects and opens directly

Context:
- Current Welcome only shows an Open button and the app always drives a file dialog. This blocks fast repeat usage.

Done means:
- Welcome screen lists recent project paths from ProjectStore.
- Clicking a recent project loads it without showing a dialog.
- Still offer an “Open…” button to browse.

Verification:
- Manual: open two projects, relaunch, verify both show in recent list and open correctly.

RGR:
- Red: add a failing test (E2E or component test) expecting recents UI.
- Green: implement minimal Welcome UI + load action.
- Refactor: extract Welcome into its own component and keep App.tsx thin.

#### A2. Walkthrough: explain the model (“edit in your editor; Right Now reacts”)

Done means:
- First-run walkthrough exists (dismissible).
- It clearly explains: file-as-db, external editor workflow, sessions/deep links.
- Walkthrough can be reopened from the UI.

Verification:
- Manual: fresh profile shows walkthrough once; later accessible via a button.

#### A3. Startup: auto-load last active project without prompting

Context:
- main.tsx currently calls openProject(lastProject) which still opens a dialog.

Done means:
- On launch, if lastActiveProject exists and is readable, load it directly.
- If it fails, fall back to Welcome with a clear error.

Verification:
- Manual: set lastActiveProject, relaunch, ensure it loads without dialog. Rename/delete file, relaunch, ensure fallback UI appears.

---

### B) Persisted timer state

#### B1. Persist work/break state + timestamps in frontmatter

Context:
- WorkState/stateTransitions are ephemeral; restart loses timer state.

Done means:
- Frontmatter includes current state + transition timestamps.
- On load, ProjectManager restores timer state from frontmatter.

Verification:
- Manual: start work, quit, reopen, ensure timer resumes correctly.

---

### C) Sessions UI integration

#### C1. Implement SessionClient (daemon RPC bridge)

Done means:
- UI can list/start/stop/continue sessions via a SessionClient.
- Errors are surfaced (at least to console + placeholder toast).

Verification:
- Manual: start/stop works from UI.

#### C2. Render session status + Start/Continue/Stop CTAs in TaskList

Done means:
- Task rows show Running/Waiting/Stopped.
- Buttons call SessionClient (not direct markdown edits).

Verification:
- Manual: start session from UI; see badge/state update.

#### C3. Deep links: route todos://session/<id> to Continue/Attach UX

Done means:
- On macOS: `open "todos://session/42"` focuses the app and routes to session 42.

Verification:
- Manual smoke test on macOS.

#### C4. Add a session list surface (running/waiting/stopped)

Done means:
- UI surface lists sessions with status, task, project path.
- Selecting a session offers Continue/Stop actions.

Verification:
- Manual: manage at least one running + one stopped session.

---

### D) Packaging hardening

#### D1. Exclude rn-test-harness from release builds

Done means:
- Release builds do not depend on rn-test-harness.
- Right Now.app/Contents/MacOS does not contain rn-test-harness.

Verification:
- Delete target/release/rn-test-harness and run: `bun run tauri build`.

#### D2. Bundle daemon + todo CLI intentionally

Done means:
- Release bundle includes required sidecars and they are discoverable at runtime.

Verification:
- Install DMG and run a session start/continue/stop flow without dev tooling.

#### D3. Add scripted post-build smoke test

Done means:
- A script validates the built bundle contents and a minimal smoke flow.

Verification:
- Script fails if a sidecar is missing.

---

## Appendix — References

- Terminal sessions plan: `docs/2025-12-PLAN-TODO-TERMINAL-SESSIONS.md`
- Event-driven plan: `docs/PLAN-EVENT-DRIVEN-ARCHITECTURE.md`
- Time tracking spec (future): `docs/time-tracking-feature.md`
- Markdown storage spec: `docs/markdown-based-project-files.md`
