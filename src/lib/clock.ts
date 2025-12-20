/**
 * Clock abstraction for time control in tests.
 *
 * This module provides an injectable Clock interface that allows:
 * - Production code to use real time via `realClock`
 * - Tests to control time via `TestClock.advance()`
 *
 * @example
 * // Production usage
 * const clock = realClock;
 * const now = clock.now();
 *
 * @example
 * // Test usage
 * const clock = new TestClock();
 * clock.setTimeout(() => console.log("fired"), 1000);
 * clock.advance(1000); // logs "fired"
 */

/** Opaque type for timer IDs to prevent mixing with other numbers */
export type TimerId = number & { readonly __brand: "TimerId" };

/**
 * Interface for time and timer operations.
 * Allows dependency injection of time for testing.
 */
export interface Clock {
  /** Returns current time in milliseconds since epoch */
  now(): number;

  /**
   * Schedules a callback to run after `ms` milliseconds.
   * @returns A TimerId that can be passed to clearTimeout
   */
  setTimeout(callback: () => void, ms: number): TimerId;

  /**
   * Schedules a callback to run repeatedly every `ms` milliseconds.
   * @returns A TimerId that can be passed to clearInterval
   */
  setInterval(callback: () => void, ms: number): TimerId;

  /** Cancels a timeout scheduled with setTimeout */
  clearTimeout(id: TimerId): void;

  /** Cancels an interval scheduled with setInterval */
  clearInterval(id: TimerId): void;
}

/**
 * Real clock implementation that delegates to browser/Node APIs.
 * Use this in production code.
 */
export const realClock: Clock = {
  now: () => Date.now(),

  setTimeout: (callback: () => void, ms: number): TimerId => {
    return globalThis.setTimeout(callback, ms) as unknown as TimerId;
  },

  setInterval: (callback: () => void, ms: number): TimerId => {
    return globalThis.setInterval(callback, ms) as unknown as TimerId;
  },

  clearTimeout: (id: TimerId): void => {
    globalThis.clearTimeout(id as unknown as number);
  },

  clearInterval: (id: TimerId): void => {
    globalThis.clearInterval(id as unknown as number);
  },
};

interface ScheduledTimer {
  id: TimerId;
  callback: () => void;
  fireAt: number;
  interval?: number; // If set, this is a repeating interval
}

/**
 * Test clock implementation with controllable time.
 *
 * Time starts at 0 and only advances when `advance()` is called.
 * Timers fire in correct chronological order during `advance()`.
 *
 * Timer Cascade Behavior: Timers scheduled during handler execution
 * that fall within the advancement window will fire within the same
 * `advance()` call.
 *
 * @example
 * const clock = new TestClock();
 * clock.setTimeout(() => {
 *   clock.setTimeout(() => console.log("nested"), 50);
 * }, 100);
 * clock.advance(200); // Both timers fire: first at 100ms, nested at 150ms
 */
export class TestClock implements Clock {
  private currentTime = 0;
  private nextId = 1;
  private timers: Map<TimerId, ScheduledTimer> = new Map();

  /** Returns the current simulated time in milliseconds */
  now(): number {
    return this.currentTime;
  }

  setTimeout(callback: () => void, ms: number): TimerId {
    const id = this.nextId++ as TimerId;
    this.timers.set(id, {
      id,
      callback,
      fireAt: this.currentTime + ms,
    });
    return id;
  }

  setInterval(callback: () => void, ms: number): TimerId {
    const id = this.nextId++ as TimerId;
    this.timers.set(id, {
      id,
      callback,
      fireAt: this.currentTime + ms,
      interval: ms,
    });
    return id;
  }

  clearTimeout(id: TimerId): void {
    this.timers.delete(id);
  }

  clearInterval(id: TimerId): void {
    this.timers.delete(id);
  }

  /**
   * Advances time by the specified number of milliseconds.
   * Fires all timers that would have triggered during this time period.
   *
   * Timers are fired in chronological order. Timers scheduled during
   * handler execution that fall within the advancement window will
   * fire within the same call (cascade behavior).
   *
   * @param ms - Number of milliseconds to advance (must be >= 0)
   * @throws Error if ms is negative
   */
  advance(ms: number): void {
    if (ms < 0) {
      throw new Error("Cannot advance time by negative amount");
    }

    const targetTime = this.currentTime + ms;

    // Process timers until we've passed the target time
    while (true) {
      // Find the next timer that should fire
      let nextTimer: ScheduledTimer | undefined;
      for (const timer of this.timers.values()) {
        if (timer.fireAt <= targetTime) {
          if (!nextTimer || timer.fireAt < nextTimer.fireAt) {
            nextTimer = timer;
          }
        }
      }

      // No more timers to fire
      if (!nextTimer) {
        break;
      }

      // Advance time to this timer's fire time
      this.currentTime = nextTimer.fireAt;

      // If it's an interval, reschedule it before calling callback
      // (so callback can clear it if needed)
      if (nextTimer.interval !== undefined) {
        nextTimer.fireAt = this.currentTime + nextTimer.interval;
      } else {
        // One-shot timer, remove it
        this.timers.delete(nextTimer.id);
      }

      // Fire the callback
      nextTimer.callback();
    }

    // Advance to final target time
    this.currentTime = targetTime;
  }

  /**
   * Sets the clock to an absolute time.
   * Does NOT fire timers that would have fired between old and new time.
   * Use `advance()` if you need timers to fire.
   *
   * @param timestamp - Absolute time in milliseconds
   */
  setTime(timestamp: number): void {
    this.currentTime = timestamp;
  }

  /** Clears all scheduled timers */
  clearAllTimers(): void {
    this.timers.clear();
  }

  /** Returns the number of pending timers */
  getPendingTimerCount(): number {
    return this.timers.size;
  }
}
