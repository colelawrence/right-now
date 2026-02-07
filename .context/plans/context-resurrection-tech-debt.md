# Context Resurrection - Tech Debt Paydown Plan

> Learnings relevant to future gates should be written back to respective gates, so future collaborators can benefit.

## Goal & motivation

Pay down tech debt introduced during the Context Resurrection (CR) epic so the feature is:

- less brittle (esp. daemon tests + IPC)
- easier to evolve (protocol/types, UI wiring)
- less surprising (storage location semantics, error semantics)

## Scope

In scope:

- Protocol contract hardening (Rust ↔ TS) and reducing "stringly-typed" error handling.
- Test reliability improvements (remove timing flake patterns).
- Storage location semantics + documentation alignment (esp. Linux/XDG vs `~/.right-now`).
- UI refactors that reduce `App.tsx` complexity without changing UX.

Out of scope (unless pulled in as a dependency):

- New CR product features (AI briefings, timeline UI, etc.).
- Cross-platform implementation (Windows pipes, etc.).
- Backwards compatibility/migrations for on-disk snapshot formats (ok to break; fail fast with reset instructions).

## Codebase context (primary touchpoints)

| Area | Files |
| --- | --- |
| Daemon protocol | `src-tauri/src/session/protocol.rs`, `src-tauri/src/session/daemon_client.rs`, `src-tauri/src/bin/right-now-daemon.rs` |
| CR snapshot model/store/query | `src-tauri/src/context_resurrection/models.rs`, `store.rs`, `query.rs`, `capture.rs` |
| Tauri bridge | `src-tauri/src/lib.rs` (`cr_request`) |
| TS CR client + helpers | `src/lib/context-resurrection/{types,client,tauri,load,note,forget,selectors}.ts` |
| UI wiring | `src/App.tsx`, `src/components/{ResurrectionCard,TaskList}.tsx` |
| Task ID generator (affects CR join key) | `src/lib/ProjectStateEditor.ts` |
| Docs | `docs/2026-02-PLAN-CONTEXT-RESURRECTION.md`, `docs/context-resurrection.md` |

## Controversial forks / decisions needed

### Linux storage directory semantics

Today `Config::base_dir()` prefers `XDG_RUNTIME_DIR/right-now` on Linux, which is **not** guaranteed to persist across reboots.

Options:

1) **Keep as-is** (runtime dir can be ephemeral) and update docs/UI copy accordingly.
2) **Split runtime vs persistent dirs**:
   - socket/pid in `XDG_RUNTIME_DIR/right-now`
   - persistent data (sessions.json, CR snapshots) in `~/.right-now/` *or* XDG state/data dirs
3) **Full XDG compliance**:
   - persistent data under `XDG_STATE_HOME`/`XDG_DATA_HOME`
   - runtime socket under `XDG_RUNTIME_DIR`

**Decision: option (2)**. It matches "daemon socket belongs in runtime dir" while keeping snapshots durable.

This decision is **converged** and should be implemented in Gate "Storage semantics + docs alignment".

---

## Gate execution order

Gates should be executed in the following order (dependencies shown):

1. **Debt inventory + tracking conversion** — no dependencies; creates tracking beads for remaining gates.
2. **Structured daemon errors** — no dependencies; foundational for protocol changes.
3. **Protocol/type drift guardrails** — depends on gate 2 (error codes must exist).
4. **Storage semantics + docs alignment** — no code dependencies; can run in parallel with gates 2-3 if desired.
5. **UI wiring refactor** — depends on gates 2-3 (typed errors needed for controller error handling).
6. **Daemon test reliability hardening** — no code dependencies; can run anytime after gate 1.

Gates 2+3 and gate 4 can proceed in parallel. Gate 5 should wait for 2+3 to land to avoid merge conflicts.

## Tracking (beads created)

| Gate | Beads |
| --- | --- |
| Structured daemon errors | bd-q85.23 |
| Protocol/type drift guardrails | bd-q85.24, bd-q85.25 |
| Storage semantics + docs alignment | bd-q85.26 |
| UI wiring refactor | bd-q85.27 |
| Daemon test reliability hardening | bd-q85.28 |

---

## Gates

### Gate: Debt inventory + tracking conversion

Deliverables
- A short, prioritized tech-debt list added to this plan (append-only section) with:
  - problem statement
  - impacted files
  - proposed fix
  - expected risk
- Convert each item into a bead (or explicitly justify why not tracked as a bead).

Acceptance
- `br sync --status` shows "In sync" after creating/updating beads.
- No TODO/notes left only in chat.

Meaningful scenarios
- New contributor can open the beads list and understand the paydown order without reading the epic transcript.


### Gate: Structured daemon errors (remove message parsing)

Problem
- TS `CrClient` infers `not_found`/`skipped` by substring-matching `DaemonResponse::Error.message`.

Deliverables
- Rust: extend protocol error shape to include a machine-readable code.
  - e.g. `DaemonResponse::Error { code: DaemonErrorCode, message: String }`
  - codes minimally: `not_found`, `skipped`, `invalid_request`, `store_unavailable`, `internal`, `daemon_unavailable`, `timeout`, `version_mismatch`
  - Define `DaemonErrorCode` as a `#[derive(Serialize, Deserialize)]` enum in `src-tauri/src/session/protocol.rs`.
  - Serialization: use `#[serde(rename_all = "snake_case")]` so JSON values are `"not_found"`, `"skipped"`, etc.
- TS: mirror error codes as a union type in `src/lib/context-resurrection/types.ts`:
  - `type DaemonErrorCode = "not_found" | "skipped" | "invalid_request" | "store_unavailable" | "internal" | "daemon_unavailable" | "timeout" | "version_mismatch"`
  - Add a protocol fixture test (see next gate) to catch Rust/TS code drift.
- Rust: update daemon to return the correct code for:
  - CR latest/get not found
  - capture skipped/dedup/rate-limit
  - store unavailable
- Rust: add request timeout handling in `daemon_client.rs` (Tauri-side IPC client):
  - set read timeout: 5 seconds for IPC requests
  - on IO timeout: return `DaemonResponse::Error { code: timeout, ... }` (do not throw) so TS receives a typed error
  - on connect/start failure: return `DaemonResponse::Error { code: daemon_unavailable, ... }` (do not throw) so TS receives a typed error
  - `store_unavailable` is reserved for “daemon is up but snapshot store cannot be used” (permissions, lock failure, etc.)
- TS: update `CrClient` to map by `code` (no string matching).
- TS: define retry policy in `client.ts`:
  - retryable codes: `timeout`, `daemon_unavailable` (max 1 retry, fixed 2s delay before retry)
  - non-retryable: `not_found`, `skipped`, `invalid_request`, `store_unavailable`, `internal`, `version_mismatch`
- Tests:
  - Rust protocol roundtrip tests updated.
  - TS `context-resurrection-client.test.ts` updated to cover code-based mapping.

Acceptance
- `rg -n "includes\(\"no snapshots\"" src/lib/context-resurrection` returns nothing.
- `bun run test:unit` and `cargo test` green.


### Gate: Protocol/type drift guardrails

Depends on: Gate "Structured daemon errors" (error codes must exist before version negotiation can return typed errors).

Problem
- CR protocol currently uses `serde_json::Value` for snapshots and shares the huge `DaemonRequest`/`DaemonResponse` for `cr_request`.
- No version negotiation—mismatched daemon/client versions fail silently or with confusing errors.

Deliverables
- **Protocol version handshake (concrete design):**
  - Rust protocol (`src-tauri/src/session/protocol.rs`):
    - Add `const PROTOCOL_VERSION: u32 = 1`.
    - Add `DaemonRequest::Handshake { client_version: u32 }`.
    - Add `DaemonResponse::Handshake { protocol_version: u32 }`.
  - Rust clients:
    - `src-tauri/src/session/daemon_client.rs`: after connecting (and before sending the real request), perform a handshake roundtrip on the same stream.
    - `src-tauri/src/bin/todo.rs`: perform handshake once right after opening the `UnixStream`.
  - Version mismatch behavior (client-enforced):
    - If `protocol_version > client_version`: return `DaemonResponse::Error { code: version_mismatch, message: "Daemon is newer than app—please update the app." }`.
    - If `protocol_version < client_version`: return `DaemonResponse::Error { code: version_mismatch, message: "Daemon is outdated—please restart daemon." }`.
  - Start at version 1; bump only on breaking changes (any request/response JSON shape change).
- **Rust+TS protocol fixture test suite** (for CR request/response tags, similar to task-id parity):
  - Rust: add `src-tauri/tests/protocol_fixtures.rs` (integration test) with golden JSON snapshots for each CR request/response variant.
  - TS: add `src/lib/context-resurrection/protocol-fixtures.test.ts` that parses the same JSON and asserts expected types.
  - Fixture JSON files live in `test/fixtures/protocol/` (repo root) and are read by both Rust and TS test suites.
- **Reduce "any" surfaces** (recommendation: typed payloads):
  - Change `DaemonResponse::CrSnapshot/CrSnapshots` payload from `serde_json::Value` to typed `ContextSnapshotV1` struct.
  - Rationale: Rust already defines `ContextSnapshot` in `models.rs`; exposing it as `Value` loses compile-time checking.
  - TS: update `types.ts` to match the typed shape; fixture tests will catch drift.
- **Size limits at IPC boundary:**
  - Rust daemon: reject incoming requests > 1MB with `invalid_request` error code (use `DaemonErrorCode` from prior gate).
  - Rust client (`daemon_client.rs`): reject responses > 10MB with `internal` error code.
- **Snapshot list pagination (defer complexity, enforce limits):**
  - Enforce `limit` semantics on `DaemonRequest::CrList` / TS `cr_list`:
    - if `limit` missing: default 100
    - if `limit` > 500: clamp to 500
    - if `limit` <= 0: return `invalid_request`
  - Response returns at most `limit` most-recent snapshots for the requested task.
  - Future: if pagination needed, add cursor-based pagination—not in this gate's scope.
- **Tighten the Tauri bridge** (recommendation: exhaustive filter tests):
  - Keep current `cr_request` filter approach but add exhaustive tests in `src-tauri/src/lib.rs` tests module that only CR-prefixed variants are accepted.
  - Rationale: introducing a separate `CrDaemonRequest` type is high churn; filter + tests achieves same safety.

Acceptance
- A single failing fixture test catches mismatched tag/type changes before UI runtime.
- Protocol version mismatch produces a clear, actionable error message (not a parse failure).
- `cargo test` and `bun run test:unit` both run the fixture tests.


### Gate: Storage semantics + docs alignment

Problem
- Storage path semantics are currently easy to misunderstand (Linux `XDG_RUNTIME_DIR` vs `~/.right-now`). Docs should match reality.
- Unix socket permissions not explicitly set—any local user may connect.

Interface boundary: Storage path resolution is **Rust-internal only**. TS never queries or constructs file paths—it accesses CR data exclusively through daemon IPC. This gate does not add new Tauri commands for path inspection.

Deliverables
- Implement option 2 (split runtime vs persistent dirs):
  - Introduce `Config::runtime_dir()` + `Config::state_dir()` methods in `src-tauri/src/session/config.rs`.
  - `runtime_dir`: socket + pid files
    - Linux: `$XDG_RUNTIME_DIR/right-now` if set, else fallback to `state_dir`
    - macOS: `~/.right-now` (same as `state_dir`)
  - `state_dir`: persistent data (sessions.json + CR snapshots)
    - macOS + Linux: `~/.right-now` (fallback `/tmp/right-now` if home unavailable)
  - Env override: `RIGHT_NOW_DAEMON_DIR` overrides **both** runtime_dir and state_dir (everything lives under the override).
  - Ensure sessions.json + CR snapshots always go to `state_dir`.
- IPC socket security (Unix):
  - Create socket directory (`runtime_dir`) with `0700` permissions before binding.
  - Bind socket, then set socket file permissions to `0600` (owner read/write only).
  - Implementation location: daemon bind code in `src-tauri/src/bin/right-now-daemon.rs`.
  - Document the security model in `docs/context-resurrection.md`.
- Storage file permissions + locking:
  - Create snapshot directory (`state_dir/context-resurrection/snapshots/`) with `0700` permissions.
  - Create snapshot files with `0600` permissions.
  - Add a file lock (`flock` on a `store.lock` file) in `store.rs` to prevent corruption from concurrent daemon starts.
  - Locking strategy: exclusive non-blocking lock on store init; if lock fails, daemon exits with clear error: "Another daemon instance is running."
- Concurrent client connections:
  - Daemon must handle multiple simultaneous client connections (one app instance per project).
  - IPC handler uses per-connection state; no global mutable state outside the locked store.
- Update `docs/context-resurrection.md` with exact platform behavior + override env vars.
- Add a small Rust unit test for config path selection on Linux behind cfg-test helpers (or a table-driven test over env vars).
- Operability: document reset procedure in `docs/context-resurrection.md`:
  - Clear instructions: "To reset CR state: stop daemon, delete `~/.right-now/context-resurrection/` (or `state_dir/context-resurrection/`), restart daemon."
  - Include env var to enable debug logging for IPC (`RIGHT_NOW_DEBUG=1` → stderr trace).

Acceptance
- Docs match code; paths are not aspirational.
- `ls -la` on socket and snapshot files shows owner-only permissions.
- Manual: set `RIGHT_NOW_DAEMON_DIR` and confirm snapshots are written under the override.
- Manual: `RIGHT_NOW_DEBUG=1` produces IPC request/response traces on stderr.


### Gate: UI wiring refactor (reduce `App.tsx` complexity)

Problem
- `App.tsx` owns a lot of CR state + side-effects; harder to reason about concurrency/cancellation and to test.

Deliverables
- Extract CR state management into a dedicated module:
  - `src/lib/context-resurrection/controller.ts` (pure orchestrator, no React imports)
  - `src/lib/context-resurrection/use-cr-controller.ts` (thin React hook wrapper)
  - Dependency direction: hook → controller → client/selectors. Controller must not import React.
  - keep existing pure helpers (`selectors`, `load`, `note`, `forget`)
- Make cancellation/overlap behavior explicit:
  - Controller tracks in-flight request per task ID.
  - On new request for same task: cancel previous (AbortController), last-call-wins.
  - On task switch: cancel all in-flight requests for previous task.
  - Unit-test these scenarios without React (mock client, assert cancel called).
- Keep UI behavior unchanged:
  - pinned/dismissed semantics
  - per-task indicators
  - resume + forget flows

Acceptance
- `App.tsx` loses CR-specific branching complexity (measurable: smaller CR section; fewer CR states in component scope).
- Unit tests cover controller orchestration paths.


### Gate: Daemon test reliability hardening

Problem
- Attention/CR daemon tests have shown timing flakiness (subscription vs output emission).

Deliverables
- Introduce a small async test helper (Rust) for "eventually" assertions with timeouts:
  - Signature: `async fn assert_eventually<F, T>(predicate: F, timeout: Duration, poll_interval: Duration) -> T`
  - On timeout: panic with descriptive message including last predicate result and elapsed time.
  - Default timeout: 5s, poll interval: 50ms (configurable per-call).
  - Location: `src-tauri/src/test_utils.rs` (new module, `#[cfg(test)]`).
- Replace sleeps/polls in daemon tests with the helper (including `test_attention_detection_records_summary`).
- Add regression test for the exact race that caused flake (subscribe late ⇒ missed attention) to ensure future refactors keep ordering safe.

Acceptance
- `cargo test --bin right-now-daemon` passes reliably when run repeatedly (local loop: 20x).

---

## Verification (global)

- `bun run typecheck`
- `bun run test:unit`
- `cd src-tauri && cargo test`
- `br sync --status` → In sync
