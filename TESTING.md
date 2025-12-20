# Testing Philosophy & Tools

## Philosophy

Testing in Right Now prioritizes **authenticity over isolation**. The goal is to enable AI agents (and humans) working in headless environments to understand and verify application behavior *as it actually runs*.

### Core Principle: Mock at the Edges, Not the Core

| Layer | Mock? | Rationale |
|-------|-------|-----------|
| PTY/Terminal sessions | **No** | Core value. Runs headless fine. Real behavior matters. |
| File system operations | **No** | Use temp directories instead. Real I/O, isolated location. |
| Tauri IPC | **No** | Real commands, real state. This IS the app. |
| Window chrome/decorations | Yes | Edge concern. Doesn't affect logic. |
| System sounds | Yes | Edge concern. Disabled in test mode. |
| Native dialogs | Yes | Can't automate. Use direct file paths instead. |

### Why This Matters for AI

When an AI agent needs to understand "what happens when a user completes a task", the answer should come from running the actual code path—not from reading mock implementations that may drift from reality. Tests that exercise real PTY sessions and real file parsing give accurate mental models.

## Test Architecture

```
┌─────────────────┐     Unix Socket      ┌─────────────────────┐
│  Bun Test       │◄───────────────────►│  Test Harness       │
│  Runner         │                      │  (real Tauri app)   │
└─────────────────┘                      └─────────────────────┘
                                                   │
                                          Real IPC │ Real PTY
                                                   ▼
                                         ┌─────────────────────┐
                                         │  Rust Backend       │
                                         │  + Session Daemon   │
                                         └─────────────────────┘
```

The test harness runs as a real Tauri application with a special window that displays all views simultaneously. External test runners communicate via Unix socket to:
- Load fixtures into temp directories
- Trigger state changes
- Assert on actual application state

### Important Implementation Details

**Temp Directory Location**: Test temp directories are created under `~/rightnow-test/` (NOT system temp like `/var/folders/...`). This is required because Tauri's fs:scope permissions don't reliably work with system temp directories. The `$HOME/**` scope in capabilities covers `~/rightnow-test/`.

**Socket Location**: The socket is at `$TMPDIR/rightnow-test-harness.sock` (on macOS, typically `/var/folders/.../T/rightnow-test-harness.sock`).

**Port Conflicts**: The test harness uses port 1421 for Vite. Always kill stale processes before running tests:
```bash
pkill -f "rn-desktop-2" && pkill -f "vite" && lsof -i :1421 -t | xargs kill -9
```

## Running Tests

```bash
# Unit tests (fast, isolated)
bun test src/**/*.test.ts

# Start test harness in dev mode (interactive)
bun run tauri:test

# Build test harness binary
bun run tauri:test:build

# Run E2E tests against harness
bun run test:e2e
```

## Test Harness Protocol

The harness listens on `$TMPDIR/rightnow-test-harness.sock`. Messages are newline-delimited JSON.

**Requests:**
```typescript
// Lifecycle
{ type: "ping" }
{ type: "shutdown" }

// Fixture Management
{ type: "create_temp_dir", label?: string }
{ type: "load_fixture", name: string, temp_dir: string }
{ type: "cleanup_all" }

// State Management
{ type: "open_project", path: string }
{ type: "get_state" }
{ type: "complete_task", task_name: string }
{ type: "change_state", state: "planning" | "working" | "break" }

// Clock Control (deterministic time)
{ type: "advance_clock", ms: number }      // Advance TestClock by ms
{ type: "set_clock_time", timestamp: number } // Jump to specific timestamp
{ type: "get_clock_time" }                 // Query current clock time

// Event History (event-driven testing)
{ type: "get_event_history" }              // Get all emitted events
{ type: "clear_event_history" }            // Reset event log for test isolation
```

**Responses:**
```typescript
{ type: "pong" }
{ type: "temp_dir_created", path: string }
{ type: "fixture_loaded", path: string }
{ type: "state", state: { /* LoadedProjectState */ } }
{ type: "ok", data?: any }                 // Generic success, data varies by command
{ type: "error", message: string }
```

## Fixtures

Located in `src-tauri/test-fixtures/`:

| Fixture | Purpose |
|---------|---------|
| `minimal.md` | Basic project with 2 tasks |
| `complex.md` | Multiple sections, completed tasks, notes |
| `with-sessions.md` | Project referencing terminal sessions |
| `empty.md` | Edge case: no tasks |

## Writing Tests

```typescript
import { runner } from "../harness/setup";

test("completing a task updates the file", async () => {
  // Setup: real temp dir, real fixture file
  const tempDir = await runner.createTempDir();
  const projectPath = await runner.loadFixture("minimal", tempDir);
  await runner.openProject(projectPath);

  // Act: real state transition through real IPC
  await runner.completeTask("First task");

  // Assert: real state from real app
  const state = await runner.getState();
  expect(state.tasks[0].complete).toBeTruthy();
});
```

## Deterministic Testing

The test harness provides two powerful primitives for writing reliable, non-flaky tests:

### TestClock: Control Time

In test mode, `Date.now()` is replaced with a `TestClock` that only advances when you tell it to. This eliminates timing-based flakiness.

```typescript
test("timer warning fires at 5 minutes remaining", async () => {
  await runner.loadFixture("pomodoro-25min");
  await runner.openProject(projectPath);

  // Start a 25-minute work session
  await runner.changeState("working");
  const state = await runner.getState();
  const endsAt = state.stateTransitions.endsAt;

  // Jump to 5 minutes before end
  await runner.setClockTime(endsAt - 5 * 60 * 1000);

  // Advance 1 second to trigger timer check
  await runner.advanceClock(1000);

  // Verify warning event was emitted
  const events = await runner.getEventHistory();
  expect(events).toContainEqual(
    expect.objectContaining({ type: "warning" })
  );
});
```

### EventBus: Verify Side Effects

The EventBus records all emitted events, enabling assertion on behavior rather than just state.

```typescript
test("completing task emits sound event", async () => {
  await runner.openProject(projectPath);
  await runner.clearEventHistory(); // Fresh start

  await runner.completeTask("First task");

  const events = await runner.getEventHistory();
  const soundEvent = events.find(
    (e) => e.type === "sound" && e.sound === "todo_complete"
  );
  expect(soundEvent).toBeDefined();
});
```

### Event Types

The following events are emitted via the EventBus (see `src/lib/events.ts` for type definitions):

| Event Type | Fields | When |
|------------|--------|------|
| `sound` | `{ sound: SoundEventName, reason: string, timestamp }` | Sound should play |
| `warning` | `{ state: WorkState, timeLeft: number, timestamp }` | Timer warning threshold hit |
| `timer_tick` | `{ timeLeft: number, overtime: boolean, timestamp }` | Timer tick for UI updates |
| `state_change` | `{ from: WorkState, to: WorkState, timestamp }` | Work state transition |
| `task_completed` | `{ taskName: string, timestamp }` | Task marked complete |

### Best Practices

1. **Clear event history** at the start of each test for isolation
2. **Use `setClockTime`** to jump to specific moments (e.g., "5 min before end")
3. **Use `advanceClock`** to trigger interval-based logic
4. **Assert on events** when testing side effects (sounds, notifications)
5. **Assert on state** when testing data transformations

## What We Don't Test Here

- **Visual rendering**: Use Playwright/screenshot tests separately if needed
- **Platform-specific window behavior**: Manual testing on each OS
- **Performance**: Separate benchmarking setup

## File Overview

```
test/
├── harness/
│   ├── runner.ts      # Unix socket client, TauriTestRunner class
│   └── setup.ts       # beforeAll/afterAll hooks
└── integration/
    └── *.test.ts      # E2E tests

src-tauri/
├── src/test_harness.rs           # Unix socket server, test commands
├── test-fixtures/*.md            # Project file fixtures
└── tauri.test.conf.json          # Test window configuration

src/
├── main-test.tsx                 # Test harness entry point
├── components/TestHarness.tsx    # Split-view test UI
└── lib/
    ├── test-bridge.ts            # window.__TEST_BRIDGE__ API
    ├── clock.ts                  # Clock interface + TestClock
    ├── events.ts                 # EventBus + AppEvent types
    └── timer-logic.ts            # Pure functions for timer events
```

## Architecture: Event-Driven Testing

```
┌────────────────────────────────────────────────────────────────────┐
│                         Test Harness                                │
├────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────────────┐  │
│  │  TestClock  │────►│  Timer      │────►│  EventBus           │  │
│  │  (control)  │     │  Logic      │     │  (record + emit)    │  │
│  └─────────────┘     └─────────────┘     └─────────────────────┘  │
│        ▲                                           │               │
│        │                                           ▼               │
│  ┌─────────────┐                          ┌─────────────────────┐  │
│  │  Test       │◄─────────────────────────│  Event History      │  │
│  │  Assertions │  getEventHistory()       │  (queryable)        │  │
│  └─────────────┘                          └─────────────────────┘  │
│                                                                     │
└────────────────────────────────────────────────────────────────────┘
```

The separation of concerns enables:
- **TestClock**: Deterministic time, no waiting
- **Timer Logic**: Pure functions, unit testable
- **EventBus**: Observable side effects, assertable history
