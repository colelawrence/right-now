// Event-driven integration tests
// Demonstrates deterministic testing with TestClock and EventBus
//
// These tests showcase the power of the test harness:
// - No waiting for real time to pass
// - Assertions on events (side effects) not just state
// - Full control over time for edge cases

import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import type { AppEvent, SoundEvent, StateChangeEvent, TaskCompletedEvent } from "../../src/lib/events";
import {
  cleanupTestTempDir,
  getRunner,
  loadTestFixture,
  setupTestHarness,
  teardownTestHarness,
} from "../harness/setup";

// Type for the state we get back from the harness
interface ProjectState {
  workState: "planning" | "working" | "break";
  stateTransitions: {
    startedAt: number;
    endsAt?: number;
  };
  projectFile: {
    pomodoroSettings: {
      workDuration: number;
      breakDuration: number;
    };
    markdown: Array<{
      type: string;
      name?: string;
      complete?: string | false;
    }>;
  };
}

// Type guard helpers
function isSoundEvent(e: AppEvent): e is SoundEvent {
  return e.type === "sound";
}

function isStateChangeEvent(e: AppEvent): e is StateChangeEvent {
  return e.type === "state_change";
}

function isTaskCompletedEvent(e: AppEvent): e is TaskCompletedEvent {
  return e.type === "task_completed";
}

describe("Event-Driven Testing", () => {
  beforeAll(async () => {
    await setupTestHarness();
  });

  afterAll(async () => {
    await teardownTestHarness();
  });

  beforeEach(async () => {
    const runner = getRunner();
    await runner.resetState();
    await runner.cleanupAll();
    // Clear event history for test isolation
    await runner.clearEventHistory();
  });

  afterEach(async () => {
    await cleanupTestTempDir();
  });

  describe("Clock Control", () => {
    it("should report clock time", async () => {
      const runner = getRunner();

      const time1 = await runner.getClockTime();
      expect(typeof time1).toBe("number");
      expect(time1).toBeGreaterThan(0);
    });

    it("should advance clock by specified milliseconds", async () => {
      const runner = getRunner();

      const initialTime = await runner.getClockTime();
      await runner.advanceClock(5000); // 5 seconds
      const newTime = await runner.getClockTime();

      expect(newTime).toBe(initialTime + 5000);
    });

    it("should set clock to specific timestamp", async () => {
      const runner = getRunner();

      const targetTime = Date.now() + 60 * 60 * 1000; // 1 hour from now
      await runner.setClockTime(targetTime);
      const clockTime = await runner.getClockTime();

      expect(clockTime).toBe(targetTime);
    });
  });

  describe("Event History", () => {
    it("should start with empty event history after clear", async () => {
      const runner = getRunner();
      await runner.clearEventHistory();

      const events = await runner.getEventHistory();
      expect(events).toEqual([]);
    });

    it("should record state change events", async () => {
      const runner = getRunner();
      const fixturePath = await loadTestFixture("minimal");
      await runner.openProject(fixturePath);
      await runner.clearEventHistory();

      // Transition to working
      await runner.changeState("working");

      const events = await runner.getEventHistory();

      // Should have state change events
      const stateChangeEvents = events.filter(isStateChangeEvent);
      expect(stateChangeEvents.length).toBeGreaterThan(0);
    });

    it("should record task completion events", async () => {
      const runner = getRunner();
      const fixturePath = await loadTestFixture("minimal");
      await runner.openProject(fixturePath);
      await runner.clearEventHistory();

      // Complete a task
      await runner.completeTask("Task 1");

      const events = await runner.getEventHistory();

      // Should have task completed event
      const taskEvents = events.filter(isTaskCompletedEvent);
      expect(taskEvents.length).toBe(1);

      // Should also have sound event
      const soundEvents = events.filter(isSoundEvent);
      expect(soundEvents.length).toBeGreaterThan(0);
    });
  });

  describe("State Change Events", () => {
    it("should emit sound event when starting work", async () => {
      const runner = getRunner();
      const fixturePath = await loadTestFixture("minimal");
      await runner.openProject(fixturePath);
      await runner.clearEventHistory();

      await runner.changeState("working");

      const events = await runner.getEventHistory();
      const soundEvents = events.filter(isSoundEvent).filter((e) => e.sound === "session_start");

      expect(soundEvents.length).toBe(1);
    });

    it("should emit sound event when starting break", async () => {
      const runner = getRunner();
      const fixturePath = await loadTestFixture("minimal");
      await runner.openProject(fixturePath);

      // First go to working
      await runner.changeState("working");
      await runner.clearEventHistory();

      // Then go to break
      await runner.changeState("break");

      const events = await runner.getEventHistory();
      const soundEvents = events.filter(isSoundEvent).filter((e) => e.sound === "break_start");

      expect(soundEvents.length).toBe(1);
    });

    it("should include from/to states in state change event", async () => {
      const runner = getRunner();
      const fixturePath = await loadTestFixture("minimal");
      await runner.openProject(fixturePath);
      await runner.clearEventHistory();

      await runner.changeState("working");

      const events = await runner.getEventHistory();
      const stateChange = events.find(isStateChangeEvent);

      expect(stateChange).toBeDefined();
      expect(stateChange?.from).toBe("planning");
      expect(stateChange?.to).toBe("working");
    });
  });

  describe("Full Pomodoro Cycle", () => {
    it("should complete full cycle with correct events", async () => {
      const runner = getRunner();
      const fixturePath = await loadTestFixture("minimal");
      await runner.openProject(fixturePath);
      await runner.clearEventHistory();

      // Planning -> Working
      await runner.changeState("working");

      // Working -> Break
      await runner.changeState("break");

      // Break -> Planning
      await runner.changeState("planning");

      const events = await runner.getEventHistory();
      const stateChanges = events.filter(isStateChangeEvent);

      // Should have 3 state changes
      expect(stateChanges.length).toBe(3);

      // Verify the progression
      expect(stateChanges[0]?.from).toBe("planning");
      expect(stateChanges[0]?.to).toBe("working");

      expect(stateChanges[1]?.from).toBe("working");
      expect(stateChanges[1]?.to).toBe("break");

      expect(stateChanges[2]?.from).toBe("break");
      expect(stateChanges[2]?.to).toBe("planning");
    });
  });

  describe("Task Completion", () => {
    it("should emit todo_complete sound on task completion", async () => {
      const runner = getRunner();
      const fixturePath = await loadTestFixture("minimal");
      await runner.openProject(fixturePath);
      await runner.clearEventHistory();

      await runner.completeTask("Task 1");

      const events = await runner.getEventHistory();
      const soundEvents = events.filter(isSoundEvent).filter((e) => e.sound === "todo_complete");

      expect(soundEvents.length).toBe(1);
    });

    it("should include task name in completion event", async () => {
      const runner = getRunner();
      const fixturePath = await loadTestFixture("minimal");
      await runner.openProject(fixturePath);
      await runner.clearEventHistory();

      await runner.completeTask("Task 1");

      const events = await runner.getEventHistory();
      const taskEvent = events.find(isTaskCompletedEvent);

      expect(taskEvent).toBeDefined();
      expect(taskEvent?.taskName).toBe("Task 1");
    });
  });
});
