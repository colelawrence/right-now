/**
 * Pure timer logic functions for computing timer events.
 *
 * This module contains stateless functions that compute what events should
 * be emitted based on timer state and current time. The functions are pure -
 * they don't have side effects and return both events to emit AND state
 * updates to apply.
 *
 * @example
 * const result = computeTimerEvents(timerState, clock.now());
 * result.events.forEach(e => eventBus.emit(e));
 * Object.assign(timerState, result.nextState);
 */

import type { AppEvent, SoundEvent, TimerTickEvent, WarningEvent } from "./events";
import type { WorkState } from "./project";
import { SoundEventName, WARNING_THRESHOLD_MS } from "./sounds";

/** Deduplication window for warnings - don't repeat within this time */
export const WARNING_DEDUP_WINDOW_MS = 30 * 1000; // 30 seconds

/**
 * Timer state required for computing timer events.
 */
export interface TimerState {
  /** Current work state */
  workState: WorkState;
  /** When the current state started */
  startedAt: number;
  /** When the current state should end (undefined for planning) */
  endsAt?: number;
  /** When the last warning was fired (for deduplication) */
  lastWarningAt?: number;
}

/**
 * Result of computing timer events.
 * Contains events to emit and state updates to apply.
 */
export interface TimerResult {
  /** Events to emit via EventBus */
  events: AppEvent[];
  /** State updates to apply (partial - only changed fields) */
  nextState: Partial<TimerState>;
}

/**
 * Computes timer events based on current state and time.
 *
 * This is a pure function - it doesn't modify state or have side effects.
 * The caller is responsible for:
 * 1. Emitting the returned events via EventBus
 * 2. Applying the returned state updates
 *
 * @param state Current timer state
 * @param now Current timestamp (from Clock.now())
 * @returns Events to emit and state updates to apply
 *
 * @example
 * const result = computeTimerEvents(state, clock.now());
 * result.events.forEach(e => eventBus.emit(e));
 * Object.assign(timerState, result.nextState);
 */
export function computeTimerEvents(state: TimerState, now: number): TimerResult {
  const events: AppEvent[] = [];
  const nextState: Partial<TimerState> = {};

  // No timer events in planning state
  if (state.workState === "planning") {
    return { events, nextState };
  }

  // No timer events if no end time set
  if (state.endsAt === undefined) {
    return { events, nextState };
  }

  const timeLeft = state.endsAt - now;
  const overtime = timeLeft < 0;

  // Always emit a timer tick event
  const tickEvent: TimerTickEvent = {
    type: "timer_tick",
    timestamp: now,
    timeLeft,
    overtime,
  };
  events.push(tickEvent);

  // Check if we should emit a warning
  if (shouldEmitWarning(state, now, timeLeft)) {
    // Create warning event
    const warningEvent: WarningEvent = {
      type: "warning",
      timestamp: now,
      state: state.workState,
      timeLeft,
    };
    events.push(warningEvent);

    // Create sound event with appropriate sound for state
    const soundEvent: SoundEvent = {
      type: "sound",
      timestamp: now,
      sound: getWarningSoundForState(state.workState),
      reason: `Timer warning: ${Math.abs(timeLeft / 1000).toFixed(0)}s ${overtime ? "overtime" : "remaining"}`,
    };
    events.push(soundEvent);

    // Update lastWarningAt for deduplication
    nextState.lastWarningAt = now;
  }

  return { events, nextState };
}

/**
 * Determines if a warning should be emitted based on state and time.
 */
function shouldEmitWarning(state: TimerState, now: number, timeLeft: number): boolean {
  // Only warn when at or below threshold
  if (timeLeft > WARNING_THRESHOLD_MS) {
    return false;
  }

  // Check deduplication window
  if (state.lastWarningAt !== undefined) {
    const timeSinceLastWarning = now - state.lastWarningAt;
    if (timeSinceLastWarning < WARNING_DEDUP_WINDOW_MS) {
      return false;
    }
  }

  return true;
}

/**
 * Gets the appropriate warning sound for the current work state.
 */
function getWarningSoundForState(workState: WorkState): SoundEventName {
  switch (workState) {
    case "working":
      return SoundEventName.BreakApproaching;
    case "break":
      return SoundEventName.BreakEndApproaching;
    default:
      // This shouldn't happen since we filter planning state
      return SoundEventName.BreakApproaching;
  }
}

/**
 * Computes events for a state change.
 *
 * @param from Previous work state
 * @param to New work state
 * @param now Current timestamp
 * @returns Events to emit for this state change
 */
export function computeStateChangeEvents(from: WorkState, to: WorkState, now: number): AppEvent[] {
  const events: AppEvent[] = [];

  // State change event
  events.push({
    type: "state_change",
    timestamp: now,
    from,
    to,
  });

  // Sound event based on transition
  const sound = getStateChangeSoundFor(from, to);
  if (sound !== undefined) {
    events.push({
      type: "sound",
      timestamp: now,
      sound,
      reason: `State change: ${from} -> ${to}`,
    });
  }

  return events;
}

/**
 * Gets the appropriate sound for a state transition.
 */
function getStateChangeSoundFor(from: WorkState, to: WorkState): SoundEventName | undefined {
  if (from === "planning" && to === "working") {
    return SoundEventName.SessionStart;
  }
  if (from === "working" && to === "break") {
    return SoundEventName.BreakStart;
  }
  if (from === "break" && to === "working") {
    return SoundEventName.WorkResumed;
  }
  if (to === "planning") {
    return SoundEventName.SessionEnd;
  }
  // Working to working (shouldn't happen) or other transitions
  return SoundEventName.WorkResumed;
}

/**
 * Computes events for task completion.
 *
 * @param taskName Name of the completed task
 * @param now Current timestamp
 * @returns Events to emit for this task completion
 */
export function computeTaskCompletedEvents(taskName: string, now: number): AppEvent[] {
  return [
    {
      type: "task_completed",
      timestamp: now,
      taskName,
    },
    {
      type: "sound",
      timestamp: now,
      sound: SoundEventName.TodoComplete,
      reason: `Task completed: ${taskName}`,
    },
  ];
}
