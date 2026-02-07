# Context Resurrection (CR)

Context Resurrection captures lightweight “where you left off” snapshots for a task so the UI can show a Resurrection Card on project open.

This is a **local, on-disk** feature.

## What CR captures

Each snapshot is a JSON file (schema `ContextSnapshotV1`) containing (best-effort):

- Project path (absolute path to `TODO.md`)
- Task identity
  - `task_id` (stable ID token from the task line, e.g. `abc.fix-api-timeout-bug`)
  - `task_title_at_capture` (task title text at capture time)
- Capture metadata
  - `captured_at` (ISO timestamp)
  - `capture_reason` (`session_stopped`, `session_waiting`, `session_running`, `idle_timeout`, `manual`)
- Terminal context (optional)
  - `session_id`, `status`, `exit_code`
  - `last_attention` (type + preview + timestamp)
  - `tail_inline` or `tail_path` (sanitized terminal tail)
- User note (optional)
  - `user_note` (“note to future self”)

## Where it is stored

By default the daemon uses `~/.right-now/` as its data directory (macOS + Linux). You can override this with:

- `RIGHT_NOW_DAEMON_DIR=/custom/path`

CR data lives under:

- `~/.right-now/context-resurrection/`
  - `snapshots/<project-hash>/<task-id>/<snapshot-id>.json`
  - `snapshots/<project-hash>/<task-id>/.lock` (flock lock file)

In v1, terminal tail is stored inline in the snapshot JSON (`terminal.tail_inline`).

`terminal.tail_path` is reserved for future use (large tails may be stored as separate files later).

Notes:

- Snapshots are written atomically (temp file + rename).
- Permissions are locked down (`0700` directories, `0600` files).

## Sanitization and privacy

Terminal output sanitization is **best-effort**.

The daemon attempts to redact common secret shapes (e.g. AWS access keys, bearer tokens, API-key style assignments, PEM private keys). This reduces accidental leakage, but it is not a security boundary.

Rules of thumb:

- Do not paste secrets into your terminal.
- Assume anything printed to the terminal *might* be captured in sanitized form.
- If you accidentally printed a secret, use the deletion controls below.

## Deletion controls

CR supports deletion at three levels:

### 1) Forget this task’s context

Deletes all snapshots for the current task under:

- `.../snapshots/<project-hash>/<task-id>/`

UI: Resurrection Card → **Forget task**

### 2) Forget project context

Deletes all snapshots for the project under:

- `.../snapshots/<project-hash>/`

UI: Resurrection Card → **Forget project**

### 3) Global deletion (manual)

To delete everything:

- Remove `~/.right-now/context-resurrection/` entirely.

## Smoke checklist (release readiness)

1. **Task ID assignment**
   - Create a new task without an ID.
   - Click ▶ Start session.
   - Confirm the task line now includes `[abc.derived-label]`.

2. **Snapshot capture**
   - Start a session and stop it.
   - Confirm a snapshot file exists under `~/.right-now/context-resurrection/snapshots/...`.

3. **Resurrection Card eligibility**
   - Reopen the app with an older snapshot.
   - Confirm the card appears when eligible.

4. **Manual note capture**
   - In the card, type a note and click **Save note**.
   - Confirm a new snapshot is created and the note is shown.

5. **Resume behavior**
   - Click **Resume work**.
   - If the session is running/waiting: it should attach/continue.
   - If stopped: it should start a new session.

6. **Deletion**
   - Click **Forget task** and confirm snapshot files are removed.
   - Click **Forget project** and confirm the project snapshot directory is removed.

7. **Daemon unavailable**
   - Stop the daemon.
   - Open the app and confirm CR features degrade gracefully (no crash; indicators/actions disabled).
