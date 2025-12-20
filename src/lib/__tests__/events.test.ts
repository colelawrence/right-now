import { beforeEach, describe, expect, it, spyOn } from "bun:test";
import {
  type AppEvent,
  AppEventBus,
  type SoundEvent,
  type StateChangeEvent,
  type TaskCompletedEvent,
  type TimerTickEvent,
  type WarningEvent,
  createEventBus,
} from "../events";
import { SoundEventName } from "../sounds";

describe("AppEventBus", () => {
  let eventBus: AppEventBus;

  // Test event factories
  const createSoundEvent = (overrides: Partial<SoundEvent> = {}): SoundEvent => ({
    type: "sound",
    timestamp: 1000,
    sound: SoundEventName.TodoComplete,
    reason: "test",
    ...overrides,
  });

  const createStateChangeEvent = (overrides: Partial<StateChangeEvent> = {}): StateChangeEvent => ({
    type: "state_change",
    timestamp: 1000,
    from: "planning",
    to: "working",
    ...overrides,
  });

  const createWarningEvent = (overrides: Partial<WarningEvent> = {}): WarningEvent => ({
    type: "warning",
    timestamp: 1000,
    state: "working",
    timeLeft: 30000,
    ...overrides,
  });

  const createTaskCompletedEvent = (overrides: Partial<TaskCompletedEvent> = {}): TaskCompletedEvent => ({
    type: "task_completed",
    timestamp: 1000,
    taskName: "Test task",
    ...overrides,
  });

  const createTimerTickEvent = (overrides: Partial<TimerTickEvent> = {}): TimerTickEvent => ({
    type: "timer_tick",
    timestamp: 1000,
    timeLeft: 60000,
    overtime: false,
    ...overrides,
  });

  beforeEach(() => {
    eventBus = new AppEventBus();
  });

  describe("emit()", () => {
    it("notifies subscribers", () => {
      const events: AppEvent[] = [];
      eventBus.subscribe((event) => events.push(event));

      const soundEvent = createSoundEvent();
      eventBus.emit(soundEvent);

      expect(events.length).toBe(1);
      expect(events[0]).toBe(soundEvent);
    });

    it("notifies multiple subscribers", () => {
      const events1: AppEvent[] = [];
      const events2: AppEvent[] = [];

      eventBus.subscribe((event) => events1.push(event));
      eventBus.subscribe((event) => events2.push(event));

      const event = createSoundEvent();
      eventBus.emit(event);

      expect(events1.length).toBe(1);
      expect(events2.length).toBe(1);
      expect(events1[0]).toBe(event);
      expect(events2[0]).toBe(event);
    });

    it("handles no subscribers gracefully", () => {
      // Should not throw
      expect(() => eventBus.emit(createSoundEvent())).not.toThrow();
    });
  });

  describe("subscribe()", () => {
    it("returns working unsubscribe function", () => {
      const events: AppEvent[] = [];
      const unsubscribe = eventBus.subscribe((event) => events.push(event));

      eventBus.emit(createSoundEvent());
      expect(events.length).toBe(1);

      unsubscribe();
      eventBus.emit(createSoundEvent());
      expect(events.length).toBe(1); // No new events after unsubscribe
    });

    it("can unsubscribe multiple times safely", () => {
      const unsubscribe = eventBus.subscribe(() => {});
      unsubscribe();
      unsubscribe(); // Should not throw
      expect(true).toBe(true);
    });
  });

  describe("subscribeByType()", () => {
    it("only receives matching events", () => {
      const soundEvents: SoundEvent[] = [];
      eventBus.subscribeByType("sound", (event) => soundEvents.push(event));

      eventBus.emit(createSoundEvent());
      eventBus.emit(createStateChangeEvent());
      eventBus.emit(createWarningEvent());

      expect(soundEvents.length).toBe(1);
      expect(soundEvents[0].type).toBe("sound");
    });

    it("returns working unsubscribe function", () => {
      const events: SoundEvent[] = [];
      const unsubscribe = eventBus.subscribeByType("sound", (event) => events.push(event));

      eventBus.emit(createSoundEvent());
      expect(events.length).toBe(1);

      unsubscribe();
      eventBus.emit(createSoundEvent());
      expect(events.length).toBe(1);
    });

    it("works with all event types", () => {
      const stateEvents: StateChangeEvent[] = [];
      const warningEvents: WarningEvent[] = [];
      const taskEvents: TaskCompletedEvent[] = [];
      const timerEvents: TimerTickEvent[] = [];

      eventBus.subscribeByType("state_change", (e) => stateEvents.push(e));
      eventBus.subscribeByType("warning", (e) => warningEvents.push(e));
      eventBus.subscribeByType("task_completed", (e) => taskEvents.push(e));
      eventBus.subscribeByType("timer_tick", (e) => timerEvents.push(e));

      eventBus.emit(createStateChangeEvent());
      eventBus.emit(createWarningEvent());
      eventBus.emit(createTaskCompletedEvent());
      eventBus.emit(createTimerTickEvent());

      expect(stateEvents.length).toBe(1);
      expect(warningEvents.length).toBe(1);
      expect(taskEvents.length).toBe(1);
      expect(timerEvents.length).toBe(1);
    });
  });

  describe("getHistory()", () => {
    it("returns all events in order", () => {
      const event1 = createSoundEvent({ timestamp: 1000 });
      const event2 = createStateChangeEvent({ timestamp: 2000 });
      const event3 = createWarningEvent({ timestamp: 3000 });

      eventBus.emit(event1);
      eventBus.emit(event2);
      eventBus.emit(event3);

      const history = eventBus.getHistory();
      expect(history.length).toBe(3);
      expect(history[0]).toBe(event1);
      expect(history[1]).toBe(event2);
      expect(history[2]).toBe(event3);
    });

    it("returns a copy (immutable)", () => {
      eventBus.emit(createSoundEvent());

      const history1 = eventBus.getHistory();
      history1.push(createSoundEvent()); // Mutate the returned array

      const history2 = eventBus.getHistory();
      expect(history2.length).toBe(1); // Original history unchanged
    });

    it("returns empty array when no events", () => {
      expect(eventBus.getHistory()).toEqual([]);
    });
  });

  describe("clearHistory()", () => {
    it("resets history", () => {
      eventBus.emit(createSoundEvent());
      eventBus.emit(createSoundEvent());
      expect(eventBus.getHistory().length).toBe(2);

      eventBus.clearHistory();
      expect(eventBus.getHistory().length).toBe(0);
    });

    it("allows new events after clearing", () => {
      eventBus.emit(createSoundEvent());
      eventBus.clearHistory();

      const newEvent = createStateChangeEvent();
      eventBus.emit(newEvent);

      const history = eventBus.getHistory();
      expect(history.length).toBe(1);
      expect(history[0]).toBe(newEvent);
    });
  });

  describe("event type discrimination", () => {
    it("can identify each event type via type field", () => {
      eventBus.emit(createSoundEvent());
      eventBus.emit(createStateChangeEvent());
      eventBus.emit(createWarningEvent());
      eventBus.emit(createTaskCompletedEvent());
      eventBus.emit(createTimerTickEvent());

      const history = eventBus.getHistory();
      expect(history[0].type).toBe("sound");
      expect(history[1].type).toBe("state_change");
      expect(history[2].type).toBe("warning");
      expect(history[3].type).toBe("task_completed");
      expect(history[4].type).toBe("timer_tick");
    });
  });

  describe("timestamp handling", () => {
    it("all events have timestamps", () => {
      const events = [
        createSoundEvent({ timestamp: 100 }),
        createStateChangeEvent({ timestamp: 200 }),
        createWarningEvent({ timestamp: 300 }),
        createTaskCompletedEvent({ timestamp: 400 }),
        createTimerTickEvent({ timestamp: 500 }),
      ];

      events.forEach((e) => eventBus.emit(e));

      const history = eventBus.getHistory();
      expect(history[0].timestamp).toBe(100);
      expect(history[1].timestamp).toBe(200);
      expect(history[2].timestamp).toBe(300);
      expect(history[3].timestamp).toBe(400);
      expect(history[4].timestamp).toBe(500);
    });
  });

  describe("reentrant emission", () => {
    it("delivers reentrant events to all subscribers", () => {
      const allEvents: AppEvent[] = [];

      // Subscriber A emits a new event when receiving 'sound'
      eventBus.subscribe((event) => {
        if (event.type === "sound") {
          eventBus.emit(createStateChangeEvent({ timestamp: 2000 }));
        }
      });

      // Subscriber B captures all events
      eventBus.subscribe((event) => allEvents.push(event));

      // Emit initial sound event
      eventBus.emit(createSoundEvent({ timestamp: 1000 }));

      // Both original SoundEvent and reentrant StateChangeEvent should be received
      // Note: Due to synchronous delivery, reentrant event completes first,
      // so B receives StateChangeEvent before SoundEvent
      expect(allEvents.length).toBe(2);
      expect(allEvents.some((e) => e.type === "sound")).toBe(true);
      expect(allEvents.some((e) => e.type === "state_change")).toBe(true);

      // History reflects emission order
      const history = eventBus.getHistory();
      expect(history[0].type).toBe("sound");
      expect(history[1].type).toBe("state_change");
    });
  });

  describe("error isolation", () => {
    it("subscriber error does not prevent other subscribers", () => {
      const events: AppEvent[] = [];

      // Handler A throws
      eventBus.subscribe(() => {
        throw new Error("test error");
      });

      // Handler B captures events
      eventBus.subscribe((event) => events.push(event));

      // Should not throw and Handler B should still receive
      eventBus.emit(createSoundEvent());
      expect(events.length).toBe(1);
    });

    it("subscriber error is logged to console", () => {
      const consoleSpy = spyOn(console, "error").mockImplementation(() => {});

      eventBus.subscribe(() => {
        throw new Error("test error message");
      });

      eventBus.emit(createSoundEvent());

      expect(consoleSpy).toHaveBeenCalled();
      const [message, error] = consoleSpy.mock.calls[0] as [string, Error];
      expect(message).toBe("EventBus subscriber error:");
      expect(error.message).toBe("test error message");

      consoleSpy.mockRestore();
    });

    it("continues delivery after multiple errors", () => {
      const events: AppEvent[] = [];

      eventBus.subscribe(() => {
        throw new Error("error 1");
      });
      eventBus.subscribe(() => {
        throw new Error("error 2");
      });
      eventBus.subscribe((event) => events.push(event));
      eventBus.subscribe(() => {
        throw new Error("error 3");
      });

      // Mock console.error to suppress test output
      const consoleSpy = spyOn(console, "error").mockImplementation(() => {});

      eventBus.emit(createSoundEvent());
      expect(events.length).toBe(1);

      consoleSpy.mockRestore();
    });
  });

  describe("utility methods", () => {
    it("getSubscriberCount returns correct count", () => {
      expect(eventBus.getSubscriberCount()).toBe(0);

      const unsub1 = eventBus.subscribe(() => {});
      expect(eventBus.getSubscriberCount()).toBe(1);

      const unsub2 = eventBus.subscribe(() => {});
      expect(eventBus.getSubscriberCount()).toBe(2);

      unsub1();
      expect(eventBus.getSubscriberCount()).toBe(1);

      unsub2();
      expect(eventBus.getSubscriberCount()).toBe(0);
    });
  });
});

describe("createEventBus()", () => {
  it("returns working EventBus instance", () => {
    const eventBus = createEventBus();
    const events: AppEvent[] = [];

    eventBus.subscribe((event) => events.push(event));
    eventBus.emit({
      type: "sound",
      timestamp: 1000,
      sound: SoundEventName.SessionStart,
      reason: "test",
    });

    expect(events.length).toBe(1);
    expect(eventBus.getHistory().length).toBe(1);
  });
});
