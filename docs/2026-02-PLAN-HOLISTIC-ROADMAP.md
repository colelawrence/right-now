# Right Now — Holistic Roadmap (Local‑first → Connected)

**Version:** 0.1 (2026-02-05)

**Intent:** Identify the biggest gaps between the current codebase and a cohesive, user-complete product, then propose an execution plan that connects the dots across:

- Core local-first UX (Planner/Tracker/Tray)
- Markdown file fidelity + multi-writer safety
- Terminal session daemon + UI surfaces
- **Connected mode**: logins + “connect to each other” (presence/rooms) without compromising local-first fundamentals

**Audience:** contributors/agents working in this repo.

**How to use this plan**

1. Read **Part 0** (Executive Blueprint) first. It defines the contract, non-goals, and the “shape” of the product.
2. Skim **Part I** (Gap Analysis) to see what’s missing.
3. Implement by **Phases** (Part IV), using the **Quality Gates** as stop-ship criteria.
4. Convert the **Master TODO Inventory** (Part X) into whatever task tracker we’re using (beads/br, GitHub issues, TODO.md) — the tasks are written to be small and verifiable.

> Note on backward compatibility: This repo is internal/experimental; we do **not** need to preserve backwards compatibility for settings/state files or on-disk formats unless a task explicitly asks for it.

---

## PART 0 — Executive Blueprint

### 0.1 Executive summary

Right Now is evolving into a **local-first focus system**:

- A Markdown-backed task file (`TODO.md`) is the source of truth.
- The UI provides two modes:
  - **Planner**: review and edit your tasks
  - **Tracker**: a minimal always-available “what I’m doing right now” bar
- A Pomodoro-like state machine (planning → working → break) is reinforced through audio cues.
- A local daemon (`right-now-daemon`) can run **PTY terminal sessions bound to tasks**, update the TODO file with session badges, and notify you when output “needs attention”.

The “next step” product gap is **coherence**: stitch these parts into complete user flows (onboarding → plan → execute → resume → share), with reliable surfaces for session control, time tracking, and—optionally—**connected mode** (login + presence + simple connection to other people).

### 0.1.1 Non-negotiables (engineering/product contract)

1. **The file is authoritative.** If we write, we must **read fresh from disk first** and write **atomically**.
2. **No silent clobbers.** If the file is invalid/mid-edit, do not “fix” it by overwriting; surface an error and retry on next change.
3. **Local-first works without an account.** Connected mode is additive and opt-in.
4. **We can break persisted formats.** Avoid migrations unless explicitly requested.
5. **One active project at a time** in the UI (unless/until multi-project is a deliberate project).
6. **Connected mode must be privacy-forward.** Default: share minimal status; task text sharing is opt-in.

### 0.2 Mission and non-goals

**Mission**

- Make “the next real thing” (current task + timer + session) always visible and frictionless.
- Keep data in **plain text** (markdown) so it stays portable and editable.
- Treat terminal work as first-class: sessions can detach/resume and reflect status back into the TODO.

**Non-goals (for the next major iteration)**

- Building a full PM tool (no Jira/Linear clone).
- Real-time collaborative editing of the same TODO.md (CRDT/OT) as a v0.x goal.
- Perfect backward compatibility for frontmatter/settings formats.
- Multi-platform parity (Windows/Linux) beyond “doesn’t crash” until explicitly prioritized.

### 0.3 Product surfaces (what needs to exist)

Local-first surfaces:

- **Welcome / Project picker** (recent projects, create/open, error recovery)
- **Planner** (task list + edit/add/reorder)
- **Tracker** (minimal control, current task, timer, session status)
- **Tray menu** (glanceable current task + quick actions)
- **Settings** (sound packs, editor, CLI install, optional connected mode)

Terminal session surfaces:

- **Session controls inline with tasks** (Start/Continue/Stop)
- **Session list** (global view; see running/waiting/stopped)
- **Attention UI** (badge/indicator + deep link routing)

Connected mode surfaces (MVP):

- **Account / Login** (sign in/out)
- **Connect** (create/join a room; invite others)
- **Presence** (see who’s working/break/planning; optional “current task” share)

### 0.4 Layered architecture (keep the core small)

**Ring 1: Local kernel (must stay boring & reliable)**

- Markdown parse/update (lossless where possible)
- ProjectManager (serialized ops, watcher lifecycle, atomic writes)
- Timer logic (pure functions + event bus)

**Ring 2: Local UX/services**

- Windows (planner/tracker sizing/styling)
- Tray integration
- Sound playback
- “Session client” that talks to the daemon

**Ring 3: Optional connected mode**

- Auth + token storage
- Presence/rooms client (WebSocket)
- Connection UI

### 0.5 Invariants to keep testable

- Any file mutation uses **read-fresh → update → atomic write**.
- Watcher reloads are **debounced** and **coalesced**, and cannot “teleport” to a prior project.
- If parsing fails, the UI keeps the last good model and shows a non-destructive error state.
- Session badges round-trip correctly in both TS and Rust parsers.

### 0.6 Decisions to lock early (ADRs)

These are churn magnets; decide explicitly before large implementation:

1. **Connected mode scope**: presence-only MVP vs shared projects vs shared sessions.
2. **Auth provider**: Supabase/Clerk/Firebase vs custom backend.
3. **Token storage**: OS keychain vs encrypted store vs plaintext (prototype only).
4. **Project identity**: how we identify “the same project” across users (frontmatter UUID, git remote, manual room).
5. **Privacy defaults**: what is shared by default (state only) vs opt-in (task text, headings).

### 0.7 Quality gates (stop-ship)

Local-first gates:

- **No clobber** regression tests: external edits + daemon badge updates + UI edits can interleave without losing data.
- **Project switch safety**: watchers and reloads cannot revive old projects.
- **Crash safety**: atomic writes; no partial file writes.

Connected mode gates:

- **No network without opt-in.**
- **Logout deletes tokens.**
- **Presence updates are rate-limited** and do not leak task names unless the user opts in.

### 0.8 Definition of Done (two milestones)

**DoD: Local v0.2 (cohesive solo flow)**

- Welcome screen shows recent projects; can open/create without a file dialog every time.
- Tasks can be added/edited/reordered from the UI while preserving markdown structure.
- Work/break timing survives app restart (persisted state transitions).
- Session badges are visible; Start/Continue/Stop flows are usable from the UI.

**DoD: Connected MVP v0.3 (login + connect + presence)**

- Users can sign in/out.
- Users can create/join a room and see other participants.
- Presence shows planning/working/break + time remaining; optional sharing of current task text.

---

## PART I — Gap analysis (current app vs cohesive product)

### 1.1 What works today (high-confidence)

- Markdown-backed tasks with headings and details.
- Completion toggling persists to disk.
- Planner + Tracker UI modes.
- Pomodoro state machine + timer + warning sound events.
- Tray shows current task context and a few next tasks.
- Local daemon + CLI session system exists (PTY, detach/attach, attention detection, badge updates).
- File watcher infrastructure exists (directory watch, coalesced reload, serialized ops).

### 1.2 Gaps (local-first UX)

**Welcome / onboarding**

- No recent-project list UI (only a basic “Open Project” button).
- Startup always prompts via file dialog (`openProject(lastProject)` still opens a dialog).
- “Open TODO file” vs “Open folder” mismatch: current dialog is folder-only.

**Planner**

- No create/edit/delete/reorder tasks.
- No inline editing of headings.
- No clear “current task” selection (it’s implicitly first incomplete).

**Tracker**

- No session controls in tracker mode.
- No “edit current task” flow.

**Settings**

- Sound packs exist but have little user-facing management.
- No editor selection / CLI install UI (some pieces exist in Rust menu).

**State persistence**

- WorkState/stateTransitions are ephemeral only; restart loses timer state.
- Time tracking (10s increments → frontmatter/task annotation) is planned but not implemented.

### 1.3 Gaps (terminal sessions as a first-class feature)

- UI has no session list, no session controls, no attach UX.
- `todos://session/<id>` deep links are handled, but there’s no consumer that “routes” to a session surface.
- Error states (daemon not running, socket unavailable, duplicate sessions) are not surfaced in UI.
- Packaging needs to ensure the daemon/CLI are consistently available in release builds.

### 1.4 Gaps (connected mode)

- No auth or user identity.
- No server/backend or protocol.
- No privacy model.
- No UI surfaces for login/connect/rooms.
- No secure credential storage.

---

## PART II — Target user flows (what “cohesive” looks like)

### 2.1 Local-first flow (solo)

1. Launch app → **Welcome**
2. Select a recent project or “Open…”
3. **Planner**: add/edit/reorder tasks; optionally choose the current task
4. Press “Start” → app switches to **Tracker**
5. During work:
   - timer counts down
   - tray shows current task
   - warnings/sounds fire
6. Complete task from tracker or planner → next task becomes current
7. Break/resume/end session
8. Quit app → reopen later and **resume state** (timer continues from persisted timestamps)

### 2.2 Terminal session flow (solo)

1. In Planner, current task has a “Start session” CTA
2. Starting creates a daemon session and updates TODO.md badge
3. UI shows session status (Running/Waiting/Stopped)
4. “Continue” attaches (either:
   - open in external terminal, or
   - embedded terminal view in-app later)
5. Attention triggers → UI shows an indicator; clicking focuses the relevant session.

### 2.3 Connected flow (MVP: presence rooms)

1. User opens Settings → “Connected mode” → **Sign in**
2. User creates a **Room** (or joins via invite link/code)
3. Room shows participants + their statuses:
   - planning/working/break
   - time remaining
   - optional: current task text (if enabled)
4. User can “Start together” (optional): room proposes a synchronized start time.

**Privacy defaults**

- Default share: only (planning/working/break) + coarse timer state.
- Opt-in share: current task name / heading.

---

## PART III — Technical approach (connecting the dots)

### 3.1 Local data model & persistence

- Expand frontmatter to persist:
  - `current_state` (planning/working/break)
  - `state_transition.started_at` / `ends_at`
  - optional: `project_id` (UUID)
- Keep a strict separation:
  - **durable state** lives in the file
  - **ephemeral UI state** (window mode, last warning timestamps) lives in memory/store

> We can change the frontmatter schema freely (internal repo; no migration required).

### 3.2 Session service (UI ↔ daemon)

Implement a thin client layer:

- `src/lib/sessions/client.ts` — talks to the daemon socket via Tauri commands (preferred) or by invoking bundled CLI (fallback).
- Expose operations:
  - `listSessions()`
  - `startSession(taskKey)`
  - `stopSession(sessionId)`
  - `continueSession(sessionId, { attach?: boolean })`
  - `attachSession(sessionId)`
- Subscribe to daemon notifications and bridge them into the app’s EventBus.

### 3.3 Connected mode architecture (recommended MVP)

**Recommendation:** start with a presence-only backend using a managed auth provider.

- Backend options (pick one):
  - **Supabase**: auth + realtime channels (fastest path)
  - **Clerk + custom WebSocket service**
  - **Custom (Axum + Postgres + WS)** for full control

**Client design**

- `AuthService`:
  - sign-in/out
  - store tokens securely
  - expose `currentUser`
- `PresenceClient`:
  - connect/join room
  - publish presence updates
  - receive presence updates

**Token storage**

- Add a Rust-side credential store using OS keychain (Keychain on macOS).
- Never store access/refresh tokens in plaintext `ProjectStore.json`.

**Project identity**

- Add `project_id: <uuid>` in frontmatter for projects that opt into connected mode.
- Room IDs can be:
  - “manual room” created by user, or
  - derived from `project_id` for a shared project experience.

### 3.4 Presence update strategy

- Broadcast on:
  - state changes (planning/working/break)
  - task completion
  - coarse timer updates (e.g., every 15s or 30s, not every tick)
- Payload (privacy-safe default):
  - user id, display name
  - work state
  - endsAt (or remaining seconds)
  - optional: current task summary (only if enabled)

---

## PART IV — Execution phases (with exit criteria)

### Phase 0 — Foundation stabilization (finish the “core tech” loop)

**Outcome:** reliability for multi-writer and watcher flows; clear error surfaces.

Exit criteria:

- E2E test(s) cover: external file edit → watcher reload → no clobber; daemon badge update + UI write interleaving.
- “Invalid markdown/frontmatter” shows a non-destructive error UI and recovers.

### Phase 1 — Welcome screen + project switching

**Outcome:** opening/creating projects is a first-class UX, not a file dialog.

Exit criteria:

- Welcome screen shows recent projects.
- “Open last project automatically” path exists (with fallback to chooser).
- “Open folder” and “Open file” behaviors are correct.

### Phase 2 — Task CRUD + reorder + edit

**Outcome:** app can be used without external editor.

Exit criteria:

- Add/edit/delete tasks.
- Reorder tasks within a section; preserve markdown structure.
- Edit headings.

### Phase 3 — Persisted state + time tracking

**Outcome:** timer sessions survive restart; time tracking becomes real.

Exit criteria:

- Work/break state and timestamps persist to file.
- Time tracking stored (initially frontmatter-only is OK); updates are rate-limited.

### Phase 4 — Sessions in UI (bridge the daemon)

**Outcome:** terminal session system is visible and usable from the UI.

Exit criteria:

- TaskList shows session badges and CTAs.
- Session list surface exists.
- Deep link `todos://session/<id>` routes to “continue/attach”.

### Phase 5 — Packaging + distribution hardening

**Outcome:** release builds are consistent (daemon/cli included, no accidental test harness).

Exit criteria:

- Release bundle includes required binaries and resources.
- Deep links work in release.
- Install/uninstall CLI shim is predictable.

### Phase 6 — Connected mode MVP (login + rooms + presence)

**Outcome:** users can login and connect to other users in a room.

Exit criteria:

- Login works end-to-end.
- Create/join room works.
- Presence shows other users and updates on state changes.

### Phase 7 — Collaboration expansions (post-MVP)

Candidates:

- “Start together” countdown and synchronization.
- Light messaging (“nudge”, “on break”) inside rooms.
- Optional: share session attention events (privacy gated).

---

## PART X — Master TODO inventory (task specs)

> These are written in a “small bead” style: Context → Done means → Verification → RGR.

### A) Welcome + projects

#### A1. Welcome screen lists recent projects and opens without a dialog

Context:
- Today the app always launches a file dialog. This blocks “quick start” and makes the app feel unfinished.

Done means:
- Welcome screen shows recent projects from `ProjectStore`.
- Clicking a recent project loads it directly.
- “Open…” still exists for browsing.

Verification:
- Manual: launch app, see recent list, click one, project loads.
- Tests: add a unit/integration test around `ProjectStore.addRecentProject` + a mocked ProjectManager load.

Red-Green-Refactor plan:
- Red: add a test that expects recent list rendering when store has entries.
- Green: implement minimal UI.
- Refactor: extract `Welcome` component and keep App.tsx slim.

#### A2. Support opening a TODO file (not just folders)

Context:
- The dialog is configured as folder-only.

Done means:
- “Open…” allows either folder or markdown file selection.

Verification:
- Manual: select a `TODO.md` file directly.

---

### B) Task editing

#### B1. Add-task UI (append + preserve formatting)

Context:
- Without task creation, the app is not usable as a primary surface.

Done means:
- Planner has an input to add a new task.
- Task is inserted in markdown without breaking existing sections.

Verification:
- Unit: ProjectStateEditor update preserves unrelated content.
- E2E: add task → file contains `- [ ] New task`.

#### B2. Edit task title + details

Done means:
- Clicking a task allows editing title and details.
- Enter saves; Esc cancels.

Verification:
- Manual: edit in UI; reopen file in editor; formatting preserved.

#### B3. Reorder tasks within a heading

Done means:
- Drag/drop or buttons reorder tasks.
- Markdown structure (headings/unrecognized blocks) remains stable.

---

### C) Persisted state + time tracking

#### C1. Persist `workState` + transition timestamps in frontmatter

Context:
- Restart currently loses timer state.

Done means:
- Starting work/break updates frontmatter state.
- On launch, ProjectManager restores state from file.

Verification:
- Manual: start work, quit app, reopen, state is restored.
- Unit: parse/update frontmatter round-trip tests.

#### C2. Rate-limited time accumulation (prototype)

Done means:
- Track work time locally (e.g., every 10s) and flush to frontmatter every N seconds/minutes.

Verification:
- Unit: deterministic TestClock drives accumulation.

---

### D) Session UI integration

#### D1. Implement `SessionClient` and show session badges in TaskList

Done means:
- TaskList renders Running/Waiting/Stopped with CTAs.
- Clicking Start/Continue/Stop calls the daemon.

Verification:
- Manual: Start session from UI; see badge change.

#### D2. Deep link routing to session continue/attach

Done means:
- `open 'todos://session/<id>'` focuses app and routes to the session action.

Verification:
- Manual smoke on macOS.

---

### E) Connected mode MVP

#### E1. Add Account screen + login/logout skeleton

Context:
- We need a first-class surface for identity.

Done means:
- UI includes Account/Connected section.
- Login starts an auth flow; logout clears tokens.

Verification:
- Manual: sign in/out; see UI state update.

#### E2. Presence rooms (create/join) + participant list

Done means:
- Create room → invite code/link.
- Join room → see participants.

Verification:
- Manual: run two app instances (or two machines) and see each other.

#### E3. Presence updates from local state

Done means:
- work/break changes publish presence.
- participant list updates within a few seconds.

---

## Appendix — References

- Terminal sessions plan: `docs/2025-12-PLAN-TODO-TERMINAL-SESSIONS.md`
- Event-driven plan: `docs/PLAN-EVENT-DRIVEN-ARCHITECTURE.md`
- Time tracking spec: `docs/time-tracking-feature.md`
- Markdown storage spec: `docs/markdown-based-project-files.md`
