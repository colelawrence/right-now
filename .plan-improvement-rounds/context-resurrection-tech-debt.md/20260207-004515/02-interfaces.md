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

Recommendation: option (2). It matches "daemon socket belongs in runtime dir" while keeping snapshots durable.

Execution should pause after Gate "Storage semantics" if this decision is still unsettled.

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
  - codes minimally: `not_found`, `skipped`, `invalid_request`, `store_unavailable`, `internal`, `timeout`
  - Define `DaemonErrorCode` as a `#[derive(Serialize, Deserialize)]` enum in `src-tauri/src/session/protocol.rs`.
  - Serialization: use `#[serde(rename_all = "snake_case")]` so JSON values are `"not_found"`, `"skipped"`, etc.
- TS: mirror error codes as a union type in `src/lib/context-resurrection/types.ts`:
  - `type DaemonErrorCode = "not_found" | "skipped" | "invalid_request" | "store_unavailable" | "internal" | "timeout"`
  - Add a protocol fixture test (see next gate) to catch Rust/TS code drift.
- Rust: update daemon to return the correct code for:
  - CR latest/get not found
  - capture skipped/dedup/rate-limit
  - store unavailable
- Rust: add request timeout handling in `daemon_client.rs`:
  - default timeout (e.g. 5s) for IPC requests
  - surface `timeout` error code to TS client
- TS: update `CrClient` to map by `code` (no string matching).
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
- **Protocol version handshake:**
  - Rust: add `protocol_version: u32` field to `DaemonResponse::Handshake` (or introduce one if missing).
  - TS: check version on first connect; if version > expected, surface error to user: "Daemon is newer than app—please update the app." If version < expected, surface: "Daemon is outdated—please restart daemon."
  - Start at version 1; bump only on breaking changes.
  - Breaking change = any change to request/response JSON shape that would cause parse failure on the other side.
- **Rust+TS protocol fixture test suite** (for CR request/response tags, similar to task-id parity):
  - Rust: add `tests/protocol_fixtures.rs` with golden JSON snapshots for each CR request/response variant.
  - TS: add `src/lib/context-resurrection/protocol-fixtures.test.ts` that parses the same JSON and asserts expected types.
  - CI: fixture JSON files live in `test/fixtures/protocol/` and are read by both test suites.
- **Reduce "any" surfaces** (recommendation: typed payloads):
  - Change `DaemonResponse::CrSnapshot/CrSnapshots` payload from `serde_json::Value` to typed `ContextSnapshotV1` struct.
  - Rationale: Rust already defines `ContextSnapshot` in `models.rs`; exposing it as `Value` loses compile-time checking.
  - TS: update `types.ts` to match the typed shape; fixture tests will catch drift.
- **Size limits at IPC boundary:**
  - Rust daemon: reject incoming requests > 1MB with `invalid_request` error code (use `DaemonErrorCode` from prior gate).
  - Rust client (`daemon_client.rs`): reject responses > 10MB with `internal` error code.
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
- Implement chosen decision (see fork above).
  - If splitting dirs: introduce `Config::runtime_dir()` + `Config::state_dir()` methods in `src-tauri/src/session/config.rs`.
  - Ensure CR snapshots always go to the persistent dir (`state_dir`).
- IPC socket security (Unix):
  - Set socket file permissions to `0600` (owner read/write only) in `daemon_client.rs` or daemon bind code.
  - Verify the socket directory has appropriate permissions (`0700`).
  - Document the security model in `docs/context-resurrection.md`.
- Storage file permissions:
  - Ensure snapshot files are created with `0600` permissions.
  - Add a file lock (e.g. `flock`) in `store.rs` to prevent corruption from concurrent daemon starts.
- Update `docs/context-resurrection.md` with exact platform behavior + override env vars.
- Add a small Rust unit test for config path selection on Linux behind cfg-test helpers (or a table-driven test over env vars).

Acceptance
- Docs match code; paths are not aspirational.
- `ls -la` on socket and snapshot files shows owner-only permissions.
- Manual: set `RIGHT_NOW_DAEMON_DIR` and confirm snapshots are written under the override.


### Gate: UI wiring refactor (reduce `App.tsx` complexity)

Problem
- `App.tsx` owns a lot of CR state + side-effects; harder to reason about concurrency/cancellation and to test.

Deliverables
- Extract CR state management into a dedicated module:
  - `src/lib/context-resurrection/controller.ts` (pure orchestrator, no React imports)
  - `src/lib/context-resurrection/use-cr-controller.ts` (thin React hook wrapper)
  - Dependency direction: hook → controller → client/selectors. Controller must not import React.
  - keep existing pure helpers (`selectors`, `load`, `note`, `forget`)
- Make cancellation/overlap behavior explicit (e.g., last-call-wins) and unit-test it without React.
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
- Introduce a small async test helper (Rust) for "eventually" assertions with timeouts.
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
