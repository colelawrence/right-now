import { describe, expect, it } from "bun:test";
import { TestClock, realClock } from "../clock";

describe("TestClock", () => {
  describe("now()", () => {
    it("returns 0 initially", () => {
      const clock = new TestClock();
      expect(clock.now()).toBe(0);
    });

    it("returns controlled time after advance", () => {
      const clock = new TestClock();
      clock.advance(1000);
      expect(clock.now()).toBe(1000);
    });

    it("accumulates multiple advances", () => {
      const clock = new TestClock();
      clock.advance(500);
      clock.advance(300);
      clock.advance(200);
      expect(clock.now()).toBe(1000);
    });
  });

  describe("setTimeout()", () => {
    it("fires at correct time", () => {
      const clock = new TestClock();
      let fired = false;
      clock.setTimeout(() => {
        fired = true;
      }, 500);

      clock.advance(400);
      expect(fired).toBe(false);

      clock.advance(100);
      expect(fired).toBe(true);
    });

    it("does not fire before scheduled time", () => {
      const clock = new TestClock();
      let fired = false;
      clock.setTimeout(() => {
        fired = true;
      }, 100);

      clock.advance(99);
      expect(fired).toBe(false);
    });

    it("fires exactly at scheduled time", () => {
      const clock = new TestClock();
      let firedAt: number | null = null;
      clock.setTimeout(() => {
        firedAt = clock.now();
      }, 500);

      clock.advance(500);
      expect(firedAt).toBe(500);
    });

    it("only fires once", () => {
      const clock = new TestClock();
      let count = 0;
      clock.setTimeout(() => {
        count++;
      }, 100);

      clock.advance(500);
      expect(count).toBe(1);
    });
  });

  describe("setInterval()", () => {
    it("fires repeatedly", () => {
      const clock = new TestClock();
      let count = 0;
      clock.setInterval(() => {
        count++;
      }, 100);

      clock.advance(350);
      expect(count).toBe(3); // Fired at 100, 200, 300
    });

    it("fires at exact intervals", () => {
      const clock = new TestClock();
      const fireTimes: number[] = [];
      clock.setInterval(() => {
        fireTimes.push(clock.now());
      }, 100);

      clock.advance(350);
      expect(fireTimes).toEqual([100, 200, 300]);
    });

    it("continues firing across multiple advances", () => {
      const clock = new TestClock();
      let count = 0;
      clock.setInterval(() => {
        count++;
      }, 100);

      clock.advance(150); // fires at 100
      expect(count).toBe(1);

      clock.advance(200); // fires at 200, 300
      expect(count).toBe(3);
    });
  });

  describe("clearTimeout()", () => {
    it("prevents timer from firing", () => {
      const clock = new TestClock();
      let fired = false;
      const id = clock.setTimeout(() => {
        fired = true;
      }, 500);

      clock.clearTimeout(id);
      clock.advance(1000);
      expect(fired).toBe(false);
    });

    it("can be called multiple times safely", () => {
      const clock = new TestClock();
      const id = clock.setTimeout(() => {}, 500);
      clock.clearTimeout(id);
      clock.clearTimeout(id); // Should not throw
      expect(true).toBe(true);
    });
  });

  describe("clearInterval()", () => {
    it("stops interval from firing", () => {
      const clock = new TestClock();
      let count = 0;
      const id = clock.setInterval(() => {
        count++;
      }, 100);

      clock.advance(250); // fires at 100, 200
      expect(count).toBe(2);

      clock.clearInterval(id);
      clock.advance(200); // would have fired at 300, 400
      expect(count).toBe(2);
    });

    it("can be cleared from within callback", () => {
      const clock = new TestClock();
      let count = 0;
      const id = clock.setInterval(() => {
        count++;
        if (count === 2) {
          clock.clearInterval(id);
        }
      }, 100);

      clock.advance(500);
      expect(count).toBe(2);
    });
  });

  describe("multiple timers", () => {
    it("fires in correct chronological order", () => {
      const clock = new TestClock();
      const log: string[] = [];

      clock.setTimeout(() => log.push("A"), 300);
      clock.setTimeout(() => log.push("B"), 100);
      clock.setTimeout(() => log.push("C"), 200);

      clock.advance(400);
      expect(log).toEqual(["B", "C", "A"]);
    });

    it("handles timers scheduled at same time", () => {
      const clock = new TestClock();
      const log: string[] = [];

      clock.setTimeout(() => log.push("A"), 100);
      clock.setTimeout(() => log.push("B"), 100);

      clock.advance(100);
      // Both should fire, order depends on map iteration (but both fire)
      expect(log.length).toBe(2);
      expect(log).toContain("A");
      expect(log).toContain("B");
    });

    it("handles mix of setTimeout and setInterval", () => {
      const clock = new TestClock();
      const log: string[] = [];

      clock.setInterval(() => log.push("interval"), 100);
      clock.setTimeout(() => log.push("timeout"), 150);

      clock.advance(250);
      // interval at 100, timeout at 150, interval at 200
      expect(log).toEqual(["interval", "timeout", "interval"]);
    });
  });

  describe("nested timer scheduling (cascade)", () => {
    it("fires nested timers within same advance window", () => {
      const clock = new TestClock();
      const log: string[] = [];

      clock.setTimeout(() => {
        log.push("outer");
        clock.setTimeout(() => {
          log.push("nested");
        }, 50);
      }, 100);

      clock.advance(200);
      expect(log).toEqual(["outer", "nested"]);
    });

    it("does not fire nested timers outside advance window", () => {
      const clock = new TestClock();
      const log: string[] = [];

      clock.setTimeout(() => {
        log.push("outer");
        clock.setTimeout(() => {
          log.push("nested");
        }, 150); // Would fire at 250, outside 200 window
      }, 100);

      clock.advance(200);
      expect(log).toEqual(["outer"]);

      clock.advance(100); // Now reach 300
      expect(log).toEqual(["outer", "nested"]);
    });

    it("handles deeply nested timers", () => {
      const clock = new TestClock();
      const log: string[] = [];

      clock.setTimeout(() => {
        log.push("level1");
        clock.setTimeout(() => {
          log.push("level2");
          clock.setTimeout(() => {
            log.push("level3");
          }, 10);
        }, 10);
      }, 10);

      clock.advance(100);
      expect(log).toEqual(["level1", "level2", "level3"]);
    });
  });

  describe("advance()", () => {
    it("advance(0) fires timers scheduled for now", () => {
      const clock = new TestClock();
      let fired = false;
      clock.setTimeout(() => {
        fired = true;
      }, 0);

      clock.advance(0);
      expect(fired).toBe(true);
    });

    it("throws on negative advance", () => {
      const clock = new TestClock();
      expect(() => clock.advance(-100)).toThrow("Cannot advance time by negative amount");
    });

    it("can advance by very large amounts", () => {
      const clock = new TestClock();
      let count = 0;
      clock.setInterval(() => {
        count++;
      }, 1000);

      clock.advance(10000);
      expect(count).toBe(10);
    });
  });

  describe("setTime()", () => {
    it("sets absolute time", () => {
      const clock = new TestClock();
      clock.setTime(5000);
      expect(clock.now()).toBe(5000);
    });

    it("does NOT fire timers when jumping forward", () => {
      const clock = new TestClock();
      let fired = false;
      clock.setTimeout(() => {
        fired = true;
      }, 100);

      clock.setTime(500);
      expect(fired).toBe(false);
    });

    it("can be used to go backward in time", () => {
      const clock = new TestClock();
      clock.advance(1000);
      clock.setTime(500);
      expect(clock.now()).toBe(500);
    });
  });

  describe("utility methods", () => {
    it("clearAllTimers removes all pending timers", () => {
      const clock = new TestClock();
      let count = 0;

      clock.setTimeout(() => count++, 100);
      clock.setInterval(() => count++, 50);
      clock.setTimeout(() => count++, 200);

      clock.clearAllTimers();
      clock.advance(500);
      expect(count).toBe(0);
    });

    it("getPendingTimerCount returns correct count", () => {
      const clock = new TestClock();
      expect(clock.getPendingTimerCount()).toBe(0);

      clock.setTimeout(() => {}, 100);
      expect(clock.getPendingTimerCount()).toBe(1);

      clock.setInterval(() => {}, 50);
      expect(clock.getPendingTimerCount()).toBe(2);

      const id = clock.setTimeout(() => {}, 200);
      expect(clock.getPendingTimerCount()).toBe(3);

      clock.clearTimeout(id);
      expect(clock.getPendingTimerCount()).toBe(2);
    });
  });
});

describe("realClock", () => {
  it("now() returns actual time", () => {
    const before = Date.now();
    const clockTime = realClock.now();
    const after = Date.now();

    expect(clockTime).toBeGreaterThanOrEqual(before);
    expect(clockTime).toBeLessThanOrEqual(after);
  });

  it("now() returns increasing values", async () => {
    const first = realClock.now();
    await new Promise((resolve) => setTimeout(resolve, 5));
    const second = realClock.now();

    expect(second).toBeGreaterThanOrEqual(first);
  });

  it("setTimeout returns a valid timer ID", () => {
    const id = realClock.setTimeout(() => {}, 10000);
    expect(id).toBeDefined();
    realClock.clearTimeout(id);
  });

  it("setInterval returns a valid timer ID", () => {
    const id = realClock.setInterval(() => {}, 10000);
    expect(id).toBeDefined();
    realClock.clearInterval(id);
  });
});
