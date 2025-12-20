# Test Harness Architecture & Implementation Plan

> **Guideline:** Interesting artifacts and learnings must be written back to this document.

## Overview

This document describes the incremental implementation of a deterministic test harness for the Right Now application. The harness enables headless E2E testing by running a real Tauri application with controlled time and observable events.

### Design Principles

- **Mock at the edges, not the core**: PTY sessions, file I/O, and Tauri IPC run for real
- **Deterministic time**: TestClock replaces `Date.now()` for reproducible tests
- **Observable side effects**: EventBus captures sounds, notifications, state changes
- **Fixture-based setup**: Markdown project files loaded from `test-fixtures/`

---

## Phase 1: Unix Socket Infrastructure ✅

### Objectives
Establish bidirectional communication between external test runners (Bun) and the Tauri application via Unix domain socket.

### Scope
- Rust-side socket server listening at `$TMPDIR/rightnow-test-harness.sock`
- JSON-over-newline protocol for request/response
- Conditional compilation via `test-harness` feature flag

### Dependencies
- None (foundational)

### Tasks
| Task | Acceptance Criteria |
|------|---------------------|
| Create socket server module | Server binds to socket path on startup |
| Implement message framing | Newline-delimited JSON parsing works |
| Add ping/pong handshake | `{"type":"ping"}` returns `{"type":"pong"}` |
| Handle shutdown gracefully | Socket file cleaned up on exit |
| Add feature flag gating | Code only compiles with `--features test-harness` |

### Verification
- **Test: Socket lifecycle** - Server starts, accepts connection, responds to ping, shuts down cleanly
- **Test: Protocol parsing** - Malformed JSON returns error response, doesn't crash
- **Test: Concurrent connections** - Multiple test runners can connect sequentially
- **Coverage requirement**: All request types have corresponding response handlers
- **Pass criteria**: `bun run test:e2e` can establish connection and receive pong

### Learnings
- Socket must be created after Tauri app initialization completes
- Using `$TMPDIR` avoids permission issues across platforms

---

## Phase 2: Test Command Protocol ✅

### Objectives
Define the complete request/response protocol for all test operations.

### Scope
- Request type enum with all supported commands
- Response type enum with success/error variants
- Type definitions shared between Rust and TypeScript

### Dependencies
- Phase 1 (socket infrastructure)

### Tasks
| Task | Acceptance Criteria |
|------|---------------------|
| Define TestRequest enum | All command variants documented |
| Define TestResponse enum | Success and error cases covered |
| Implement request routing | Each request type dispatched to handler |
| Add request timeout handling | Hung requests don't block forever |
| Create TypeScript mirror types | `runner.ts` types match Rust exactly |

### Verification
- **Test: Unknown command** - Unrecognized type returns structured error
- **Test: Missing fields** - Incomplete requests return validation error
- **Test: Type safety** - TypeScript compilation fails if types drift
- **Coverage requirement**: Every TestRequest variant has a test
- **Pass criteria**: Protocol documented in TESTING.md, types compile clean

### Learnings
- Keep protocol simple: one request, one response, no streaming
- `request_id` field enables async command correlation if needed later

---

## Phase 3: Fixture Management ✅

### Objectives
Enable loading test project files into isolated temp directories.

### Scope
- Copy fixtures from `src-tauri/test-fixtures/` to temp locations
- Track created temp directories for cleanup
- Support labeled temp dirs for debugging

### Dependencies
- Phase 2 (protocol defines fixture commands)

### Tasks
| Task | Acceptance Criteria |
|------|---------------------|
| Create fixture directory | `test-fixtures/*.md` files exist |
| Implement `create_temp_dir` | Returns path under `~/rightnow-test/` |
| Implement `load_fixture` | Copies named fixture to temp dir |
| Implement `list_fixtures` | Returns available fixture names |
| Implement `cleanup_all` | Removes all created temp dirs |
| Add `cleanup_temp_dir` | Removes specific temp dir |

### Verification
- **Test: Fixture isolation** - Each test gets independent copy
- **Test: Cleanup completeness** - No temp dirs remain after test suite
- **Test: Missing fixture** - Loading nonexistent fixture returns error
- **Test: Path traversal** - `../` in fixture name rejected
- **Coverage requirement**: All fixture commands tested
- **Pass criteria**: Fixtures load, tests run isolated, cleanup works

### Learnings
- Use `~/rightnow-test/` not system temp - Tauri fs:scope doesn't cover `/var/folders`
- Label temp dirs with test names for easier debugging

---

## Phase 4: State Management ✅

### Objectives
Enable tests to query and modify application state through the test bridge.

### Scope
- Frontend `window.__TEST_BRIDGE__` API exposed in test mode
- Rust forwards commands to frontend via Tauri events
- State queries return serialized LoadedProjectState

### Dependencies
- Phase 3 (fixtures provide project files to load)

### Tasks
| Task | Acceptance Criteria |
|------|---------------------|
| Create TestBridge interface | TypeScript interface defined |
| Implement `get_state` | Returns current project state or null |
| Implement `open_project` | Loads project file, state becomes available |
| Implement `complete_task` | Marks named task complete |
| Implement `change_state` | Transitions work state (planning/working/break) |
| Implement `reset_state` | Clears current project |
| Wire Rust to frontend | Commands forwarded via `test:command` event |

### Verification
- **Test: State round-trip** - Load fixture, get state, verify structure
- **Test: Task completion** - Complete task, get state, verify marked done
- **Test: State transitions** - Change through all states, verify each
- **Test: No project loaded** - Commands before open_project handled gracefully
- **Coverage requirement**: All state commands have happy path + error tests
- **Pass criteria**: Tests can load projects and manipulate state

### Learnings
- Frontend initialization is async - must poll `get_state` until ready
- Return debug info in `open_project` response for troubleshooting

---

## Phase 5: Deterministic Time (TestClock) ✅

### Objectives
Replace real time with controllable TestClock for reproducible timer tests.

### Scope
- Clock interface with `now()` method
- TestClock implementation with `advance()` and `setTime()`
- Inject clock into all time-dependent code paths

### Dependencies
- Phase 4 (state management for timer state)

### Tasks
| Task | Acceptance Criteria |
|------|---------------------|
| Define Clock interface | `now(): number` method |
| Implement RealClock | Delegates to `Date.now()` |
| Implement TestClock | Controllable time, starts at real time |
| Add `advance_clock` command | Moves time forward by ms |
| Add `set_clock_time` command | Jumps to specific timestamp |
| Add `get_clock_time` command | Returns current clock time |
| Inject clock into ProjectManager | Timer calculations use clock |

### Verification
- **Test: Clock isolation** - TestClock doesn't affect system time
- **Test: Advance accuracy** - `advance(5000)` moves time exactly 5000ms
- **Test: Set time** - `setTime(X)` makes `now()` return X
- **Test: Timer integration** - Work session timer respects TestClock
- **Coverage requirement**: Clock commands tested, timer logic uses injected clock
- **Pass criteria**: Tests can manipulate time without waiting

### Learnings
- Initialize TestClock with real time so relative calculations work
- Timer intervals should check clock, not use `setInterval` durations

---

## Phase 6: Event-Driven Testing (EventBus) ✅

### Objectives
Make application side effects observable through an event bus with history.

### Scope
- EventBus interface with emit/subscribe/history
- AppEvent union type for all event kinds
- Commands to query and clear event history

### Dependencies
- Phase 5 (clock provides timestamps for events)

### Tasks
| Task | Acceptance Criteria |
|------|---------------------|
| Define EventBus interface | emit, subscribe, getHistory, clearHistory |
| Define AppEvent types | sound, state_change, task_completed, warning, timer_tick |
| Implement AppEventBus | Stores history, notifies subscribers |
| Add `get_event_history` command | Returns all events since last clear |
| Add `clear_event_history` command | Resets history for test isolation |
| Emit events from state changes | State transitions emit state_change + sound |
| Emit events from task completion | Task done emits task_completed + sound |

### Verification
- **Test: Event recording** - Events appear in history after emit
- **Test: History isolation** - Clear removes all events
- **Test: State change events** - Transition emits correct from/to
- **Test: Sound events** - Correct sound name for each action
- **Test: Subscriber errors** - One failing subscriber doesn't break others
- **Coverage requirement**: All event types tested, history commands tested
- **Pass criteria**: Tests can assert on events instead of polling state

### Learnings
- Events have flat structure (no `payload` wrapper) - fields at top level
- Type guards (`isSoundEvent`, etc.) make filtering type-safe

---

## Phase 7: Timer Warning & Completion Events 🔲

### Objectives
Emit events when timer approaches end and when timer completes.

### Scope
- Warning events at configurable threshold (default 1 minute)
- Timer completion events when time reaches zero
- Overtime tracking for work sessions

### Dependencies
- Phase 5 (TestClock for time control)
- Phase 6 (EventBus for event emission)

### Tasks
| Task | Acceptance Criteria |
|------|---------------------|
| Implement timer tick logic | Calculates time remaining from clock |
| Add warning threshold config | Configurable via pomodoro settings |
| Emit `warning` event | Fires once when threshold crossed |
| Emit `timer_tick` event | Periodic updates for UI |
| Handle overtime | Continue tracking after timer ends |
| Add timer completion sound | `session_end` or `break_end` sound emitted |

### Verification
- **Test: Warning fires once** - Crossing threshold emits exactly one warning
- **Test: Warning timing** - Event fires at correct time relative to end
- **Test: Completion event** - Timer reaching zero emits completion
- **Test: Overtime tracking** - `overtime: true` in tick events after end
- **Test: No warning in planning** - Warning only during timed states
- **Coverage requirement**: All timer states tested with clock manipulation
- **Pass criteria**: E2E tests verify warning and completion without real waiting

### Acceptance Criteria (Overall)
- [ ] `advanceClock` to warning threshold triggers warning event
- [ ] `advanceClock` past timer end triggers completion event
- [ ] Overtime continues to emit ticks with `overtime: true`

---

## Phase 8: Sound Player Integration 🔲

### Objectives
Connect EventBus sound events to the actual sound player (with test mode bypass).

### Scope
- Sound player subscribes to EventBus `sound` events
- Test mode flag to disable actual audio playback
- Verify correct sounds play for each action

### Dependencies
- Phase 6 (EventBus emits sound events)

### Tasks
| Task | Acceptance Criteria |
|------|---------------------|
| Create sound player subscriber | Listens to EventBus sound events |
| Add test mode detection | `window.__TEST_MODE__` or similar flag |
| Skip audio in test mode | No actual sounds during E2E tests |
| Map event sounds to files | SoundEventName -> audio file path |
| Handle missing sound files | Graceful fallback, log warning |

### Verification
- **Test: Sound mapping** - Each SoundEventName maps to valid file
- **Test: Test mode silent** - No audio plays during E2E tests
- **Test: Subscriber lifecycle** - Subscribe on init, unsubscribe on cleanup
- **Coverage requirement**: All sound events have mapping tests
- **Pass criteria**: Event history shows sounds without blocking on audio

---

## Phase 9: PTY Session Testing 🔲

### Objectives
Enable testing of terminal session functionality through the harness.

### Scope
- Commands to create, write to, and read from PTY sessions
- Session state queryable through test bridge
- Integration with todo shell shim for command detection

### Dependencies
- Phase 4 (state management)
- Existing PTY infrastructure in `src-tauri/src/session/`

### Tasks
| Task | Acceptance Criteria |
|------|---------------------|
| Add `create_session` command | Spawns new PTY session |
| Add `write_to_session` command | Sends input to session |
| Add `read_session_output` command | Gets buffered output |
| Add `close_session` command | Terminates session cleanly |
| Query session state | Get active sessions and their status |
| Test todo shim integration | `todo` command in session marks task |

### Verification
- **Test: Session lifecycle** - Create, use, close without leaks
- **Test: Output capture** - Commands produce expected output
- **Test: Todo shim** - Running `todo "task"` marks task complete
- **Test: Multiple sessions** - Can manage several concurrent sessions
- **Coverage requirement**: All session commands tested
- **Pass criteria**: PTY sessions testable in headless CI

### Learnings
(To be filled during implementation)

---

## Phase 10: CI Integration 🔲

### Objectives
Run the full E2E test suite in continuous integration.

### Scope
- GitHub Actions workflow for test harness
- Binary caching for faster builds
- Test result reporting

### Dependencies
- All previous phases (complete test suite)

### Tasks
| Task | Acceptance Criteria |
|------|---------------------|
| Create CI workflow file | `.github/workflows/test-harness.yml` |
| Build test harness binary | `cargo build --features test-harness` |
| Cache Rust/Node dependencies | Incremental builds fast |
| Run E2E tests | `bun run test:e2e` in CI |
| Report test results | Failures visible in PR checks |
| Handle display server | Xvfb or similar for headless GUI |

### Verification
- **Test: CI runs** - Workflow triggers on PR
- **Test: Failures block** - Failing tests prevent merge
- **Test: Caching works** - Second run faster than first
- **Coverage requirement**: All E2E tests pass in CI
- **Pass criteria**: Green CI badge on main branch

---

## Test Organization

### Directory Structure
```
test/
├── harness/
│   ├── runner.ts       # TauriTestRunner class
│   └── setup.ts        # beforeAll/afterAll hooks
├── integration/
│   ├── event-driven.test.ts    # EventBus + Clock tests
│   ├── fixtures.test.ts        # Fixture loading tests
│   ├── state.test.ts           # State management tests
│   └── pty.test.ts             # PTY session tests (Phase 9)
└── unit/                       # Pure function tests (in src/)
```

### Naming Conventions
- Unit tests: `*.test.ts` adjacent to source file
- Integration tests: `test/integration/*.test.ts`
- E2E tests: Use test harness, require `bun run test:e2e`

### Running Tests
```bash
# Unit tests only (fast, no harness needed)
bun test src/**/*.test.ts

# Start test harness (separate terminal)
bun run tauri:test

# Run E2E tests (requires harness running)
bun run test:e2e

# All tests
bun test
```

---

## Artifacts & Learnings Log

### 2024-12-20: Event Type Naming
- Event types use underscores not colons: `state_change` not `state:changed`
- Sound enum values are snake_case: `session_start`, `todo_complete`
- No `payload` wrapper on events - fields are at top level

### 2024-12-20: Temp Directory Location
- Tauri `fs:scope` doesn't reliably work with `/var/folders/...`
- Use `~/rightnow-test/` which falls under `$HOME/**` scope

### 2024-12-20: Frontend Readiness
- Socket/Rust side starts before frontend is ready
- Must poll `get_state` until it returns non-null
- Added `waitForFrontendReady()` to runner with timeout

(Add new learnings here as implementation progresses)
