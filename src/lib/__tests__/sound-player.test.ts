import { beforeEach, describe, expect, it } from "bun:test";
import { AppEventBus, type SoundEvent, type StateChangeEvent, type TimerTickEvent, type WarningEvent } from "../events";
import { SoundPlayer, createSoundPlayer } from "../sound-player";
import { type ISoundManager, SoundEventName } from "../sounds";

describe("SoundPlayer", () => {
  let eventBus: AppEventBus;
  let mockSoundManager: ISoundManager;
  let playedSounds: SoundEventName[];

  // Helper to create events
  const createSoundEvent = (sound: SoundEventName): SoundEvent => ({
    type: "sound",
    timestamp: 1000,
    sound,
    reason: "test",
  });

  const createStateChangeEvent = (): StateChangeEvent => ({
    type: "state_change",
    timestamp: 1000,
    from: "planning",
    to: "working",
  });

  const createWarningEvent = (): WarningEvent => ({
    type: "warning",
    timestamp: 1000,
    state: "working",
    timeLeft: 30000,
  });

  const createTimerTickEvent = (): TimerTickEvent => ({
    type: "timer_tick",
    timestamp: 1000,
    timeLeft: 60000,
    overtime: false,
  });

  beforeEach(() => {
    eventBus = new AppEventBus();
    playedSounds = [];
    mockSoundManager = {
      playSound: (event: SoundEventName) => {
        playedSounds.push(event);
      },
      listSoundPacks: async () => [],
      listSoundVariations: async () => [],
    } as ISoundManager;
  });

  describe("playSound on SoundEvent", () => {
    it("calls playSound on SoundEvent", () => {
      new SoundPlayer(eventBus, mockSoundManager);
      eventBus.emit(createSoundEvent(SoundEventName.TodoComplete));

      expect(playedSounds).toEqual([SoundEventName.TodoComplete]);
    });

    it("plays correct sound from event", () => {
      new SoundPlayer(eventBus, mockSoundManager);
      eventBus.emit(createSoundEvent(SoundEventName.SessionStart));

      expect(playedSounds).toEqual([SoundEventName.SessionStart]);
    });
  });

  describe("ignores non-sound events", () => {
    it("ignores StateChangeEvent", () => {
      new SoundPlayer(eventBus, mockSoundManager);
      eventBus.emit(createStateChangeEvent());

      expect(playedSounds).toEqual([]);
    });

    it("ignores WarningEvent", () => {
      new SoundPlayer(eventBus, mockSoundManager);
      eventBus.emit(createWarningEvent());

      expect(playedSounds).toEqual([]);
    });

    it("ignores TimerTickEvent", () => {
      new SoundPlayer(eventBus, mockSoundManager);
      eventBus.emit(createTimerTickEvent());

      expect(playedSounds).toEqual([]);
    });
  });

  describe("handles multiple SoundEvents", () => {
    it("plays multiple different sounds", () => {
      new SoundPlayer(eventBus, mockSoundManager);

      eventBus.emit(createSoundEvent(SoundEventName.SessionStart));
      eventBus.emit(createSoundEvent(SoundEventName.TodoComplete));
      eventBus.emit(createSoundEvent(SoundEventName.BreakStart));

      expect(playedSounds).toEqual([
        SoundEventName.SessionStart,
        SoundEventName.TodoComplete,
        SoundEventName.BreakStart,
      ]);
    });

    it("plays same sound multiple times", () => {
      new SoundPlayer(eventBus, mockSoundManager);

      eventBus.emit(createSoundEvent(SoundEventName.TodoComplete));
      eventBus.emit(createSoundEvent(SoundEventName.TodoComplete));
      eventBus.emit(createSoundEvent(SoundEventName.TodoComplete));

      expect(playedSounds).toEqual([
        SoundEventName.TodoComplete,
        SoundEventName.TodoComplete,
        SoundEventName.TodoComplete,
      ]);
    });
  });

  describe("dispose()", () => {
    it("stops listening after dispose", () => {
      const soundPlayer = new SoundPlayer(eventBus, mockSoundManager);

      // Should work before dispose
      eventBus.emit(createSoundEvent(SoundEventName.SessionStart));
      expect(playedSounds).toEqual([SoundEventName.SessionStart]);

      // Dispose
      soundPlayer.dispose();

      // Should not receive after dispose
      eventBus.emit(createSoundEvent(SoundEventName.TodoComplete));
      expect(playedSounds).toEqual([SoundEventName.SessionStart]); // Still just the first one
    });

    it("can dispose multiple times safely", () => {
      const soundPlayer = new SoundPlayer(eventBus, mockSoundManager);
      soundPlayer.dispose();
      soundPlayer.dispose(); // Should not throw
      expect(true).toBe(true);
    });
  });

  describe("mixed events", () => {
    it("only responds to SoundEvents among mixed events", () => {
      new SoundPlayer(eventBus, mockSoundManager);

      eventBus.emit(createStateChangeEvent());
      eventBus.emit(createSoundEvent(SoundEventName.SessionStart));
      eventBus.emit(createWarningEvent());
      eventBus.emit(createSoundEvent(SoundEventName.BreakApproaching));
      eventBus.emit(createTimerTickEvent());
      eventBus.emit(createSoundEvent(SoundEventName.TodoComplete));

      expect(playedSounds).toEqual([
        SoundEventName.SessionStart,
        SoundEventName.BreakApproaching,
        SoundEventName.TodoComplete,
      ]);
    });
  });
});

describe("createSoundPlayer()", () => {
  it("returns working SoundPlayer instance", () => {
    const eventBus = new AppEventBus();
    const playedSounds: SoundEventName[] = [];
    const mockSoundManager = {
      playSound: (event: SoundEventName) => {
        playedSounds.push(event);
      },
      listSoundPacks: async () => [],
      listSoundVariations: async () => [],
    } as ISoundManager;

    const soundPlayer = createSoundPlayer(eventBus, mockSoundManager);

    eventBus.emit({
      type: "sound",
      timestamp: 1000,
      sound: SoundEventName.SessionStart,
      reason: "test",
    });

    expect(playedSounds).toEqual([SoundEventName.SessionStart]);

    // Cleanup
    soundPlayer.dispose();
  });
});
