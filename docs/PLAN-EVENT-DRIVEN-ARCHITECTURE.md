# Event-Driven Architecture Refactoring Plan

> **Interesting artifacts and learnings must be written back to this document.**

## Overview

This document describes an incremental refactoring to make the Right Now application more testable by introducing:
- An injectable Clock interface for time control
- An EventBus for observable application events
- Pure functions for timer logic
- Sound playback as an event subscriber (edge concern)

### Why This Architecture?

The current implementation mixes time-dependent logic with side effects, making it impossible to:
- Test that warning sounds fire at the correct threshold
- Verify timer behavior without waiting real time
- Assert on sound events without playing audio

The new architecture separates **deciding what should happen** from **making it happen**.

### Current State

| Component | Location | Problem |
|-----------|----------|---------|
| Timer warnings | `App.tsx:88-111` | Uses `Date.now()`, real `setInterval`, direct sound calls |
| State changes | `App.tsx:135-182` | One function handles state + sound + window changes |
| Sound playback | `sounds.ts` | Interface exists but no DI, directly calls Tauri |

### Target State

```
Clock (injectable) → TimerService (pure) → EventBus (observable)
                                                ↓
                                    Sound/Window subscribers (edges)
```

---

## Phase 1: Clock Interface

### Objectives
- Introduce an abstraction over `Date.now()` and timer functions
- Enable time control in tests without modifying production behavior
- Establish the pattern for dependency injection in the app

### Scope
- Create `Clock` interface and `realClock` implementation
- Create `TestClock` implementation with `advance()` method
- No changes to existing application logic yet

### Dependencies
- None (foundational phase)

### Task List

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 1.1 | Create `src/lib/clock.ts` with `Clock` interface | Interface defines `now()`, `setTimeout()`, `setInterval()`, `clearTimeout()`, `clearInterval()` |
| 1.2 | Implement `realClock` constant | All methods delegate to `Date.now()` and `window.setTimeout/setInterval` |
| 1.3 | Implement `TestClock` class | Has `advance(ms)` method that moves time forward and fires due timers in correct order |
| 1.4 | Define timer cascade behavior | Timers scheduled during `advance()` that fall within the advancement window fire in the same call |
| 1.5 | Export clock types and implementations | Both `realClock` and `TestClock` are importable |
| 1.6 | Add JSDoc comments | Each method and class has clear documentation |

### Design Decisions

**Timer Cascade Behavior:** When `advance(ms)` is called, timers scheduled during handler execution that fall within the advancement window will fire within the same `advance()` call. This ensures deterministic behavior:

```typescript
clock.setTimeout(() => {
  clock.setTimeout(() => console.log("nested"), 50); // fires at t=150
}, 100);
clock.advance(200); // Both timers fire: first at t=100, nested at t=150
```

**requestAnimationFrame:** Not abstracted in this phase. React's scheduler uses rAF but our timer logic uses `setInterval`. If needed, add `requestAnimationFrame()` to the Clock interface in a future iteration.

### Verification

**Test File:** `src/lib/__tests__/clock.test.ts`

**Test Scenarios:**

1. **TestClock.now() returns controlled time**
   - Create TestClock
   - Verify `now()` returns 0 initially
   - Call `advance(1000)`
   - Verify `now()` returns 1000
   - Pass: Time advances exactly as specified

2. **TestClock.setTimeout fires at correct time**
   - Create TestClock
   - Register setTimeout for 500ms with callback that sets a flag
   - Call `advance(400)` - flag should be false
   - Call `advance(100)` - flag should be true
   - Pass: Callback fires exactly when time reaches threshold

3. **TestClock.setInterval fires repeatedly**
   - Create TestClock
   - Register setInterval for 100ms with callback that increments counter
   - Call `advance(350)`
   - Pass: Counter equals 3 (fired at 100, 200, 300)

4. **TestClock.clearTimeout prevents firing**
   - Create TestClock
   - Register setTimeout for 500ms
   - Call clearTimeout with returned ID
   - Call `advance(1000)`
   - Pass: Callback never fires

5. **TestClock handles multiple timers in correct order**
   - Create TestClock
   - Register setTimeout at 300ms that logs "A"
   - Register setTimeout at 100ms that logs "B"
   - Register setTimeout at 200ms that logs "C"
   - Call `advance(400)`
   - Pass: Log order is ["B", "C", "A"]

6. **realClock.now() returns actual time**
   - Call `realClock.now()` twice with small delay between
   - Pass: Second call returns value >= first call

7. **TestClock handles nested timer scheduling (cascade)**
   - Create TestClock
   - Register setTimeout at 100ms that schedules another setTimeout at +50ms
   - Call `advance(200)`
   - Pass: Both callbacks fire (first at 100ms, nested at 150ms)

8. **TestClock.advance(0) fires timers scheduled for now**
   - Create TestClock
   - Register setTimeout for 0ms
   - Call `advance(0)`
   - Pass: Callback fires immediately

**Coverage Requirements:**
- All `TestClock` methods must have test coverage
- Edge cases: zero-duration timers, `advance(0)` behavior, negative advance (should throw), overlapping timers, nested timer scheduling

---

## Phase 2: EventBus

### Objectives
- Create a centralized event system for application events
- Enable observation of all significant app actions
- Provide test utilities for event assertions

### Scope
- Define `AppEvent` union type for all event kinds
- Create `EventBus` interface and `AppEventBus` implementation
- Add test helpers for event history inspection

### Dependencies
- Phase 1 (Clock) - for timestamp events
- Existing `SoundEventName` enum from `sounds.ts`
- Existing `WorkState` type from `project.ts`

### Task List

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 2.1 | Create `src/lib/events.ts` | File exists with proper exports |
| 2.2 | Define `AppEvent` type union | Includes: `SoundEvent`, `StateChangeEvent`, `WarningEvent`, `TaskCompletedEvent`, `TimerTickEvent` |
| 2.3 | Define each event type with timestamp | Each has `type` discriminator, `timestamp` field, and relevant payload fields |
| 2.4 | Create `EventBus` interface | Has `emit(event)`, `subscribe(handler)`, and `subscribeByType<T>(type, handler)` methods |
| 2.5 | Implement `AppEventBus` class | Maintains subscriber set, calls all subscribers on emit |
| 2.6 | Implement typed subscription | `subscribeByType()` only invokes handler for matching event types |
| 2.7 | Add `getHistory()` method | Returns copy of all emitted events for testing |
| 2.8 | Add `clearHistory()` method | Clears event history for test isolation |
| 2.9 | Ensure unsubscribe works | `subscribe()` returns cleanup function that removes handler |
| 2.10 | Implement error isolation | Subscriber errors are caught and logged, do not prevent other subscribers from receiving events |

### Design Decisions

**Reentrant Emission:** Events emitted by a subscriber during its handler are processed synchronously and delivered to all subscribers (including the emitting subscriber if it matches). This keeps behavior predictable but requires subscribers to guard against infinite loops.

**Event Timestamps:** All events include a `timestamp` field populated by the Clock at emission time. This enables event replay, debugging, and correlation.

**Error Isolation:** If a subscriber throws, the error is caught and logged to `console.error`, but emission continues to remaining subscribers. This prevents one faulty subscriber from breaking the entire event system.

```typescript
// Error isolation behavior
eventBus.subscribe(() => { throw new Error("bad"); });
eventBus.subscribe((e) => console.log(e)); // Still receives events
```

### Verification

**Test File:** `src/lib/__tests__/events.test.ts`

**Test Scenarios:**

1. **EventBus.emit notifies subscribers**
   - Create EventBus
   - Subscribe with handler that captures events
   - Emit a SoundEvent
   - Pass: Handler received exactly one event with correct type

2. **EventBus supports multiple subscribers**
   - Create EventBus with two subscribers
   - Emit one event
   - Pass: Both handlers receive the event

3. **EventBus.subscribe returns working unsubscribe function**
   - Create EventBus and subscribe
   - Call returned unsubscribe function
   - Emit event
   - Pass: Handler does not receive event after unsubscribe

4. **EventBus.getHistory returns all events in order**
   - Emit three events of different types
   - Call getHistory()
   - Pass: Returns array of 3 events in emission order

5. **EventBus.clearHistory resets history**
   - Emit two events
   - Call clearHistory()
   - Emit one more event
   - Call getHistory()
   - Pass: Returns array with only the last event

6. **EventBus.getHistory returns a copy (immutable)**
   - Emit one event
   - Get history and push to returned array
   - Get history again
   - Pass: Second getHistory() still returns only original event

7. **Event type discrimination works**
   - Emit SoundEvent, StateChangeEvent, WarningEvent
   - Subscribe with handler that filters by type
   - Pass: Can correctly identify each event type via `event.type`

8. **subscribeByType only receives matching events**
   - Create EventBus
   - Use `subscribeByType('sound', handler)`
   - Emit SoundEvent, StateChangeEvent, WarningEvent
   - Pass: Handler only called once (for SoundEvent)

9. **All events have timestamps**
   - Emit any event
   - Pass: Event in history has `timestamp` field matching clock time

10. **Reentrant emission delivers to all subscribers**
    - Subscribe handler A that emits a new event when receiving 'sound'
    - Subscribe handler B that captures all events
    - Emit SoundEvent
    - Pass: Handler B receives both original SoundEvent and reentrant event

11. **Subscriber error does not prevent other subscribers**
    - Subscribe handler A that throws
    - Subscribe handler B that captures events
    - Emit event
    - Pass: Handler B still receives event, error is logged

12. **Subscriber error is logged to console**
    - Mock console.error
    - Subscribe handler that throws Error("test")
    - Emit event
    - Pass: console.error called with error containing "test"

**Coverage Requirements:**
- All EventBus methods tested
- All AppEvent types can be emitted and received
- Memory leak check: ensure unsubscribed handlers are garbage-collectable
- Error isolation verified

---

## Phase 3: Timer Logic Extraction

### Objectives
- Extract timer warning logic from React component into pure functions
- Make timer behavior unit-testable without React or DOM
- Prepare for Clock injection

### Scope
- Create pure functions that compute timer events given state and time
- Move warning threshold logic out of `App.tsx`
- Do NOT yet wire into the app (that's Phase 5)

### Dependencies
- Phase 1 (Clock) - uses Clock interface for time
- Phase 2 (EventBus) - emits AppEvents

### Task List

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 3.1 | Create `src/lib/timer-logic.ts` | File exists with exports |
| 3.2 | Define `TimerState` interface | Contains `workState`, `startedAt`, `endsAt`, `lastWarningAt` |
| 3.3 | Define `TimerResult` interface | Contains `events: AppEvent[]` and `nextState: Partial<TimerState>` |
| 3.4 | Implement `computeTimerEvents(state, now)` | Pure function returns `TimerResult` with events and state updates |
| 3.5 | Handle warning threshold detection | Returns warning + sound events when `timeLeft <= WARNING_THRESHOLD_MS` |
| 3.6 | Handle warning deduplication | Only returns warning if `lastWarningAt` is stale or undefined |
| 3.7 | Return `nextState.lastWarningAt` when warning fires | Caller must apply this to maintain deduplication |
| 3.8 | Handle overtime detection | Returns TimerTickEvent with `overtime: true` when past `endsAt` |
| 3.9 | Return empty result for planning state | No timer events in planning mode |
| 3.10 | Map warning state to correct sound | Working → BreakApproaching, Break → BreakEndApproaching |

### Design Decisions

**Pure Function with State Updates:** `computeTimerEvents()` returns both events to emit AND state updates to apply. This keeps the function pure while making state management explicit:

```typescript
interface TimerResult {
  events: AppEvent[];
  nextState: Partial<TimerState>; // e.g., { lastWarningAt: 12345 }
}

// Caller responsibility:
const result = computeTimerEvents(state, clock.now());
result.events.forEach(e => eventBus.emit(e));
Object.assign(timerState, result.nextState); // Apply state updates
```

**Warning Deduplication Window:** Warnings are deduplicated using a 30-second window. If `now - lastWarningAt < 30000`, no new warning is emitted. This prevents warning spam while ensuring repeated notifications for long overtime periods.

### Verification

**Test File:** `src/lib/__tests__/timer-logic.test.ts`

**Test Scenarios:**

1. **Returns empty result in planning state**
   - State with `workState: 'planning'`
   - Pass: `computeTimerEvents()` returns `{ events: [], nextState: {} }`

2. **Returns empty result when no endsAt**
   - State with `workState: 'working'`, `endsAt: undefined`
   - Pass: Returns `{ events: [], nextState: {} }`

3. **Returns TimerTickEvent with correct timeLeft**
   - State with `endsAt: 1000`, call with `now: 400`
   - Pass: Returns event with `timeLeft: 600`, `overtime: false`

4. **Returns TimerTickEvent with overtime flag when past endsAt**
   - State with `endsAt: 1000`, call with `now: 1500`
   - Pass: Returns event with `timeLeft: -500`, `overtime: true`

5. **Returns warning event at threshold boundary (60s)**
   - State with `endsAt: 60000` (60s), `workState: 'working'`
   - Call with `now: 0` (60s remaining, exactly at WARNING_THRESHOLD_MS)
   - Pass: Returns WarningEvent because exactly at threshold

6. **Returns BreakApproaching sound for working state warning**
   - Same as above
   - Pass: Returns SoundEvent with `sound: 'break_approaching'`

7. **Returns BreakEndApproaching sound for break state warning**
   - State with `workState: 'break'`, at warning threshold
   - Pass: Returns SoundEvent with `sound: 'break_end_approaching'`

8. **Does NOT return warning when above threshold**
   - State with `endsAt: 120000` (120s), call with `now: 0`
   - Pass: No WarningEvent in result (only TimerTickEvent)

9. **Does NOT return duplicate warning within 30s window**
   - State with `lastWarningAt: 1000`, `endsAt: 60000`
   - Call with `now: 25000` (24s since last warning, within 30s dedup window)
   - Pass: No WarningEvent in result

10. **Returns warning after 30s dedup window expires**
    - State with `lastWarningAt: 0`, `endsAt: 60000`
    - Call with `now: 35000` (35s since last warning, outside 30s dedup window)
    - Pass: Returns WarningEvent

11. **Returns nextState.lastWarningAt when warning fires**
    - State triggering a warning at `now: 50000`
    - Pass: `result.nextState.lastWarningAt === 50000`

12. **Does NOT include nextState.lastWarningAt when no warning**
    - State not triggering a warning
    - Pass: `result.nextState.lastWarningAt === undefined`

**Coverage Requirements:**
- All branches in `computeTimerEvents` covered
- All WorkState values tested
- Boundary conditions for WARNING_THRESHOLD_MS tested

---

## Phase 4: Sound Player as Subscriber

### Objectives
- Decouple sound playback from event emission
- Sound becomes an "edge" that subscribes to events
- Enable testing that events are emitted without playing audio

### Scope
- Create SoundPlayer class that subscribes to EventBus
- Only plays sounds when it receives SoundEvents
- No changes to existing ISoundManager

### Dependencies
- Phase 2 (EventBus) - subscribes to events
- Existing `ISoundManager` from `sounds.ts`

### Task List

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 4.1 | Create `src/lib/sound-player.ts` | File exists with exports |
| 4.2 | Implement `SoundPlayer` class | Constructor takes EventBus and ISoundManager |
| 4.3 | Subscribe to EventBus in constructor | Filters for `type: 'sound'` events |
| 4.4 | Call `soundManager.playSound()` on SoundEvents | Passes the `sound` field from event |
| 4.5 | Implement `dispose()` method | Calls unsubscribe function, cleans up |
| 4.6 | Export factory function | `createSoundPlayer(eventBus, soundManager)` for convenience |

### Verification

**Test File:** `src/lib/__tests__/sound-player.test.ts`

**Test Scenarios:**

1. **SoundPlayer calls playSound on SoundEvent**
   - Create EventBus and mock ISoundManager
   - Create SoundPlayer
   - Emit SoundEvent with `sound: 'todo_complete'`
   - Pass: `soundManager.playSound` called with `'todo_complete'`

2. **SoundPlayer ignores non-sound events**
   - Create setup as above
   - Emit StateChangeEvent, WarningEvent, TimerTickEvent
   - Pass: `soundManager.playSound` never called

3. **SoundPlayer handles multiple SoundEvents**
   - Emit three different SoundEvents
   - Pass: `playSound` called three times with correct sounds

4. **SoundPlayer.dispose stops listening**
   - Create SoundPlayer and call `dispose()`
   - Emit SoundEvent
   - Pass: `playSound` not called after dispose

5. **createSoundPlayer factory returns working instance**
   - Use factory function
   - Emit SoundEvent
   - Pass: Sound plays correctly

**Coverage Requirements:**
- All SoundPlayer methods tested
- Mock ISoundManager to verify calls without audio

---

## Phase 5: Integration - Wire Everything Together

This phase is split into three sub-phases to reduce risk and provide clear checkpoints.

### Dependencies
- Phases 1-4 complete
- Existing test harness working

---

### Phase 5a: Clock Integration

**Objective:** Replace all `Date.now()` and timer calls with injected Clock.

**Gate:** App functions identically with `realClock`, tests can use `TestClock`.

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 5a.1 | Add `clock` to AppControllers interface | Optional `clock?: Clock` field |
| 5a.2 | Default to `realClock` in `main.tsx` | App works without explicit clock |
| 5a.3 | Audit App.tsx for `Date.now()` occurrences | Document actual line numbers (verify count before changing) |
| 5a.4 | Replace `Date.now()` with `clock.now()` | All occurrences use injected clock |
| 5a.5 | Replace `setInterval` with `clock.setInterval()` | Timer warning effect uses clock |
| 5a.6 | Update test harness to accept TestClock | Can pass TestClock in test mode |
| 5a.7 | Add `advance_time` command to test harness | Socket command advances TestClock |
| 5a.8 | Add `set_time` command to test harness | Socket command sets TestClock to absolute value |
| 5a.9 | Verify app works in dev mode | Manual smoke test: timers count down correctly |

**Checkpoint:** Run existing test suite. All tests pass. App works normally.

---

### Phase 5b: EventBus Wiring

**Objective:** Replace direct `playSound()` calls with EventBus emissions.

**Gate:** All sounds play via event subscription, no direct calls remain.

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 5b.1 | Add `eventBus` to AppControllers interface | Optional `eventBus?: EventBus` field |
| 5b.2 | Create `AppEventBus` in `main.tsx` | Passed to App and SoundPlayer |
| 5b.3 | Create SoundPlayer in `main.tsx` | Wired to EventBus and ISoundManager |
| 5b.4 | Extract warning logic to use `computeTimerEvents()` | Call pure function, apply `nextState` updates |
| 5b.5 | Emit timer events via EventBus | Warning/tick events go through bus |
| 5b.6 | Replace state-change `playSound()` with emissions | Planning→Working, Working→Break, etc. |
| 5b.7 | Replace task-completion `playSound()` with emission | TaskCompletedEvent triggers sound |
| 5b.8 | Remove direct `soundManager.playSound()` from App | Only SoundPlayer calls playSound |
| 5b.9 | Add `get_event_history` command to test harness | Returns EventBus history |
| 5b.10 | Add `clear_event_history` command to test harness | Clears history for test isolation |
| 5b.11 | Smoke test: verify audio plays in dev mode | State changes and warnings produce sound |

**Checkpoint:** Run existing test suite. All tests pass. Sounds play correctly.

---

### Phase 5c: Integration Tests

**Objective:** Add comprehensive E2E tests for time-based events.

**Gate:** Full test coverage for timer and event behavior.

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 5c.1 | Create `test/integration/timer-events.test.ts` | File exists with test structure |
| 5c.2 | Test: warning event fires at threshold | Uses TestClock + event history |
| 5c.3 | Test: correct sound for working warning | BreakApproaching sound emitted |
| 5c.4 | Test: correct sound for break warning | BreakEndApproaching sound emitted |
| 5c.5 | Test: warning deduplication works | No duplicate within 30s window |
| 5c.6 | Test: state change emits correct sound | session_start on planning→working |
| 5c.7 | Test: task completion emits sound | todo_complete on task done |
| 5c.8 | Verify `task-completion.test.ts` passes | No regressions |
| 5c.9 | Verify `tracker-mode.test.ts` passes | No regressions |
| 5c.10 | Run full test suite in CI | All tests green |

---

### Phase 5 Verification Summary

**Test Files:**
- `test/integration/timer-events.test.ts` (new)
- `test/integration/task-completion.test.ts` (existing - verify no regression)
- `test/integration/tracker-mode.test.ts` (existing - verify no regression)

**Test Scenarios:**

1. **Time advancement triggers warning event**
   - Load project, switch to working mode with 2-minute duration
   - Advance time by 1 minute 5 seconds (55s remaining)
   - Query event history
   - Pass: Contains WarningEvent with `state: 'working'`

2. **Time advancement triggers correct sound event**
   - Same setup as above
   - Pass: Contains SoundEvent with `sound: 'break_approaching'`

3. **Warning event does not duplicate within 30s window**
   - After triggering first warning, advance time by 10 more seconds
   - Query event history
   - Pass: Still only one WarningEvent (not two)

4. **State change emits correct sound event**
   - Load project, switch from planning to working
   - Query event history
   - Pass: Contains SoundEvent with `sound: 'session_start'`

5. **Task completion emits sound event**
   - Complete a task
   - Query event history
   - Pass: Contains SoundEvent with `sound: 'todo_complete'`

6. **Break state warning emits BreakEndApproaching**
   - Switch to break mode, advance to warning threshold
   - Query event history
   - Pass: Contains SoundEvent with `sound: 'break_end_approaching'`

7. **Existing task-completion tests pass unchanged**
   - Run full `task-completion.test.ts` suite
   - Pass: All tests pass

8. **Existing tracker-mode tests pass unchanged**
   - Run full `tracker-mode.test.ts` suite
   - Pass: All tests pass

**Coverage Requirements:**
- All timer-related events verified via E2E tests
- Zero regression in existing test suites
- Integration tests use TestClock and verify deterministic behavior

---

## Phase 6: Documentation and Cleanup

### Objectives
- Update all documentation to reflect new architecture
- Remove any dead code from refactoring
- Ensure future developers understand the patterns

### Scope
- Update TESTING.md
- Update inline documentation
- Clean up unused imports/code

### Dependencies
- Phase 5 complete and verified

### Task List

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 6.1 | Update TESTING.md with EventBus testing patterns | Documents how to use `advanceTime`, `setTime`, and `getEventHistory` |
| 6.2 | Add architecture diagram to README or docs | Visual showing Clock → TimerLogic → EventBus → Subscribers |
| 6.3 | Add JSDoc to all new public APIs | Clock, EventBus, TimerLogic all documented |
| 6.4 | Remove dead code | No unused imports or unreachable code |
| 6.5 | Verify lint passes | `bun run lint` succeeds |
| 6.6 | Verify type check passes | `bun run typecheck` succeeds |
| 6.7 | Add migration notes for in-flight changes | Document any breaking changes to AppControllers interface |

### Verification

**Manual Review Checklist:**

1. TESTING.md contains section on EventBus testing
2. New test helpers (`advanceTime`, `setTime`, `getEventHistory`) are documented
3. All new files have top-level doc comments explaining purpose
4. No `// TODO: remove` or similar comments left behind
5. Architecture diagram accurately reflects implementation
6. Migration notes exist if AppControllers interface changed

---

## Appendix A: File Structure After Refactoring

```
src/lib/
├── clock.ts           # NEW: Clock interface, realClock, TestClock
├── events.ts          # NEW: EventBus, AppEvent types
├── timer-logic.ts     # NEW: Pure timer computation functions
├── sound-player.ts    # NEW: EventBus subscriber for sounds
├── sounds.ts          # EXISTING: ISoundManager (unchanged)
├── project.ts         # EXISTING: ProjectManager (unchanged)
└── __tests__/
    ├── clock.test.ts        # NEW: Unit tests for Clock
    ├── events.test.ts       # NEW: Unit tests for EventBus
    ├── timer-logic.test.ts  # NEW: Unit tests for timer functions
    └── sound-player.test.ts # NEW: Unit tests for SoundPlayer

test/integration/
├── task-completion.test.ts  # EXISTING: Verify no regression
├── tracker-mode.test.ts     # EXISTING: Verify no regression
└── timer-events.test.ts     # NEW: E2E tests for time-based events
```

---

## Appendix B: Event Type Reference

All events include a `timestamp` field populated by the Clock at emission time.

```typescript
// Base event fields (all events extend this)
{ timestamp: number }

// Sound playback request
{ type: 'sound', timestamp: number, sound: SoundEventName, reason: string }

// Work state transition
{ type: 'state_change', timestamp: number, from: WorkState, to: WorkState }

// Timer approaching threshold
{ type: 'warning', timestamp: number, state: WorkState, timeLeft: number }

// Task marked complete
{ type: 'task_completed', timestamp: number, taskName: string }

// Timer tick (for UI updates)
{ type: 'timer_tick', timestamp: number, timeLeft: number, overtime: boolean }
```

---

## Appendix C: Test Harness Protocol Additions

**New Commands:**

```typescript
// Advance TestClock by specified milliseconds (relative)
{ type: "advance_time", ms: number }
// Response: { type: "ok", newTime: number }

// Set TestClock to absolute timestamp (useful for specific scenarios)
{ type: "set_time", timestamp: number }
// Response: { type: "ok", newTime: number }

// Get all events emitted since last clear
{ type: "get_event_history" }
// Response: { type: "event_history", events: AppEvent[] }

// Clear event history for test isolation
{ type: "clear_event_history" }
// Response: { type: "ok" }
```

**Usage Notes:**

- `advance_time` is preferred for most tests as it simulates natural time progression
- `set_time` is useful for edge cases requiring specific absolute timestamps
- `set_time` does NOT fire timers that would have fired between old and new time (use `advance_time` for that)

---

## Learnings Log

> Document discoveries, gotchas, and decisions made during implementation here.

| Date | Phase | Learning |
|------|-------|----------|
| 2025-12-19 | 1 | Use `globalThis` instead of `window` for timer APIs to support both browser and bun test environments |
| 2025-12-19 | 2 | Reentrant events complete delivery before returning - test expectations should use `some()` checks rather than strict order for subscriber-received events. History order reflects emission order. |
| 2025-12-19 | 3 | Added `computeStateChangeEvents()` and `computeTaskCompletedEvents()` helper functions to provide a complete API for all event-producing operations |
| 2025-12-19 | 4 | SoundPlayer is minimal by design - just subscribes and plays. Complex sound logic belongs in timer-logic where it can be unit tested |
