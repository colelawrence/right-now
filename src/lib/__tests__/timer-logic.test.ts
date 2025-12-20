import { describe, expect, it } from "bun:test";
import type { SoundEvent, TimerTickEvent, WarningEvent } from "../events";
import { SoundEventName, WARNING_THRESHOLD_MS } from "../sounds";
import {
  type TimerState,
  WARNING_DEDUP_WINDOW_MS,
  computeStateChangeEvents,
  computeTaskCompletedEvents,
  computeTimerEvents,
} from "../timer-logic";

describe("computeTimerEvents", () => {
  const createTimerState = (overrides: Partial<TimerState> = {}): TimerState => ({
    workState: "working",
    startedAt: 0,
    endsAt: 120000, // 2 minutes
    lastWarningAt: undefined,
    ...overrides,
  });

  describe("planning state", () => {
    it("returns empty result in planning state", () => {
      const state = createTimerState({ workState: "planning" });
      const result = computeTimerEvents(state, 60000);

      expect(result.events).toEqual([]);
      expect(result.nextState).toEqual({});
    });
  });

  describe("no endsAt", () => {
    it("returns empty result when no endsAt", () => {
      const state = createTimerState({ endsAt: undefined });
      const result = computeTimerEvents(state, 60000);

      expect(result.events).toEqual([]);
      expect(result.nextState).toEqual({});
    });
  });

  describe("timer tick events", () => {
    it("returns TimerTickEvent with correct timeLeft", () => {
      const state = createTimerState({ endsAt: 1000 });
      const result = computeTimerEvents(state, 400);

      const tickEvent = result.events.find((e) => e.type === "timer_tick") as TimerTickEvent;
      expect(tickEvent).toBeDefined();
      expect(tickEvent.timeLeft).toBe(600);
      expect(tickEvent.overtime).toBe(false);
    });

    it("returns TimerTickEvent with overtime flag when past endsAt", () => {
      const state = createTimerState({ endsAt: 1000 });
      const result = computeTimerEvents(state, 1500);

      const tickEvent = result.events.find((e) => e.type === "timer_tick") as TimerTickEvent;
      expect(tickEvent).toBeDefined();
      expect(tickEvent.timeLeft).toBe(-500);
      expect(tickEvent.overtime).toBe(true);
    });
  });

  describe("warning events", () => {
    it("returns warning event at threshold boundary", () => {
      // 60s remaining (exactly at WARNING_THRESHOLD_MS)
      const state = createTimerState({ endsAt: WARNING_THRESHOLD_MS });
      const result = computeTimerEvents(state, 0);

      const warningEvent = result.events.find((e) => e.type === "warning") as WarningEvent;
      expect(warningEvent).toBeDefined();
      expect(warningEvent.state).toBe("working");
      expect(warningEvent.timeLeft).toBe(WARNING_THRESHOLD_MS);
    });

    it("returns BreakApproaching sound for working state warning", () => {
      const state = createTimerState({ workState: "working", endsAt: WARNING_THRESHOLD_MS });
      const result = computeTimerEvents(state, 0);

      const soundEvent = result.events.find((e) => e.type === "sound") as SoundEvent;
      expect(soundEvent).toBeDefined();
      expect(soundEvent.sound).toBe(SoundEventName.BreakApproaching);
    });

    it("returns BreakEndApproaching sound for break state warning", () => {
      const state = createTimerState({ workState: "break", endsAt: WARNING_THRESHOLD_MS });
      const result = computeTimerEvents(state, 0);

      const soundEvent = result.events.find((e) => e.type === "sound") as SoundEvent;
      expect(soundEvent).toBeDefined();
      expect(soundEvent.sound).toBe(SoundEventName.BreakEndApproaching);
    });

    it("does NOT return warning when above threshold", () => {
      // 120s remaining (above WARNING_THRESHOLD_MS)
      const state = createTimerState({ endsAt: 120000 });
      const result = computeTimerEvents(state, 0);

      const warningEvent = result.events.find((e) => e.type === "warning");
      expect(warningEvent).toBeUndefined();
    });

    it("does NOT return duplicate warning within 30s window", () => {
      const state = createTimerState({
        endsAt: WARNING_THRESHOLD_MS,
        lastWarningAt: 1000,
      });
      // 24s since last warning (within 30s dedup window)
      const result = computeTimerEvents(state, 25000);

      const warningEvent = result.events.find((e) => e.type === "warning");
      expect(warningEvent).toBeUndefined();
    });

    it("returns warning after 30s dedup window expires", () => {
      const state = createTimerState({
        endsAt: WARNING_THRESHOLD_MS + 35000, // 95s end time
        lastWarningAt: 0,
      });
      // 35s since last warning (outside 30s dedup window), now at 60s remaining
      const result = computeTimerEvents(state, 35000);

      const warningEvent = result.events.find((e) => e.type === "warning");
      expect(warningEvent).toBeDefined();
    });
  });

  describe("nextState updates", () => {
    it("returns nextState.lastWarningAt when warning fires", () => {
      const state = createTimerState({ endsAt: WARNING_THRESHOLD_MS });
      const now = 50000;
      const result = computeTimerEvents(state, now);

      expect(result.nextState.lastWarningAt).toBe(now);
    });

    it("does NOT include nextState.lastWarningAt when no warning", () => {
      const state = createTimerState({ endsAt: 200000 }); // 140s remaining
      const result = computeTimerEvents(state, 60000);

      expect(result.nextState.lastWarningAt).toBeUndefined();
    });
  });

  describe("overtime warnings", () => {
    it("returns warning when in overtime", () => {
      const state = createTimerState({ endsAt: 1000 });
      // 500ms overtime
      const result = computeTimerEvents(state, 1500);

      const warningEvent = result.events.find((e) => e.type === "warning") as WarningEvent;
      expect(warningEvent).toBeDefined();
      expect(warningEvent.timeLeft).toBe(-500);
    });

    it("deduplicates overtime warnings", () => {
      const state = createTimerState({
        endsAt: 1000,
        lastWarningAt: 1100, // First overtime warning at 100ms overtime
      });
      // 10s since last warning, still overtime
      const result = computeTimerEvents(state, 11100);

      const warningEvent = result.events.find((e) => e.type === "warning");
      expect(warningEvent).toBeUndefined();
    });
  });
});

describe("computeStateChangeEvents", () => {
  it("emits state_change event", () => {
    const events = computeStateChangeEvents("planning", "working", 1000);

    const stateChange = events.find((e) => e.type === "state_change");
    expect(stateChange).toBeDefined();
    expect(stateChange!.type).toBe("state_change");
    if (stateChange!.type === "state_change") {
      expect(stateChange.from).toBe("planning");
      expect(stateChange.to).toBe("working");
    }
  });

  it("emits SessionStart sound for planning -> working", () => {
    const events = computeStateChangeEvents("planning", "working", 1000);

    const soundEvent = events.find((e) => e.type === "sound") as SoundEvent;
    expect(soundEvent).toBeDefined();
    expect(soundEvent.sound).toBe(SoundEventName.SessionStart);
  });

  it("emits BreakStart sound for working -> break", () => {
    const events = computeStateChangeEvents("working", "break", 1000);

    const soundEvent = events.find((e) => e.type === "sound") as SoundEvent;
    expect(soundEvent).toBeDefined();
    expect(soundEvent.sound).toBe(SoundEventName.BreakStart);
  });

  it("emits WorkResumed sound for break -> working", () => {
    const events = computeStateChangeEvents("break", "working", 1000);

    const soundEvent = events.find((e) => e.type === "sound") as SoundEvent;
    expect(soundEvent).toBeDefined();
    expect(soundEvent.sound).toBe(SoundEventName.WorkResumed);
  });

  it("emits SessionEnd sound for -> planning", () => {
    const events = computeStateChangeEvents("working", "planning", 1000);

    const soundEvent = events.find((e) => e.type === "sound") as SoundEvent;
    expect(soundEvent).toBeDefined();
    expect(soundEvent.sound).toBe(SoundEventName.SessionEnd);
  });

  it("includes timestamp on all events", () => {
    const events = computeStateChangeEvents("planning", "working", 12345);

    events.forEach((event) => {
      expect(event.timestamp).toBe(12345);
    });
  });
});

describe("computeTaskCompletedEvents", () => {
  it("emits task_completed event", () => {
    const events = computeTaskCompletedEvents("Fix the bug", 1000);

    const taskEvent = events.find((e) => e.type === "task_completed");
    expect(taskEvent).toBeDefined();
    if (taskEvent?.type === "task_completed") {
      expect(taskEvent.taskName).toBe("Fix the bug");
    }
  });

  it("emits TodoComplete sound", () => {
    const events = computeTaskCompletedEvents("Fix the bug", 1000);

    const soundEvent = events.find((e) => e.type === "sound") as SoundEvent;
    expect(soundEvent).toBeDefined();
    expect(soundEvent.sound).toBe(SoundEventName.TodoComplete);
  });

  it("includes timestamp on all events", () => {
    const events = computeTaskCompletedEvents("Test task", 54321);

    events.forEach((event) => {
      expect(event.timestamp).toBe(54321);
    });
  });

  it("includes task name in sound reason", () => {
    const events = computeTaskCompletedEvents("Important task", 1000);

    const soundEvent = events.find((e) => e.type === "sound") as SoundEvent;
    expect(soundEvent.reason).toContain("Important task");
  });
});

describe("constants", () => {
  it("WARNING_DEDUP_WINDOW_MS is 30 seconds", () => {
    expect(WARNING_DEDUP_WINDOW_MS).toBe(30000);
  });

  it("WARNING_THRESHOLD_MS is 60 seconds", () => {
    expect(WARNING_THRESHOLD_MS).toBe(60000);
  });
});
