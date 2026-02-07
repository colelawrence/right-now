import { describe, expect, it } from "bun:test";
import { saveNoteSnapshot } from "../context-resurrection/note";
import type { ContextSnapshotV1, CrResult } from "../context-resurrection/types";

function ok<T>(value: T): CrResult<T> {
  return { ok: true, value };
}

function err<T>(error: any): CrResult<T> {
  return { ok: false, error };
}

describe("saveNoteSnapshot", () => {
  const snapshot: ContextSnapshotV1 = {
    id: "snap-1",
    version: 1,
    project_path: "/tmp/TODO.md",
    task_id: "abc.task",
    task_title_at_capture: "Task",
    captured_at: "2026-02-06T13:12:33Z",
    capture_reason: "manual",
    user_note: "hello",
  };

  it("returns error when note is empty", async () => {
    const client = {
      captureNow: async () => ok(snapshot),
    };

    const res = await saveNoteSnapshot(client, "/tmp/TODO.md", "abc.task", "   ");
    expect(res.ok).toBe(false);
    if (!res.ok) {
      expect(res.error.type).toBe("daemon_error");
    }
  });

  it("captures note and returns new snapshot", async () => {
    let called: any[] | null = null;
    const client = {
      captureNow: async (...args: any[]) => {
        called = args;
        return ok(snapshot);
      },
    };

    const res = await saveNoteSnapshot(client, "/tmp/TODO.md", "abc.task", "  hello  ");
    expect(res).toEqual({ ok: true, value: snapshot });
    expect(called).toEqual(["/tmp/TODO.md", "abc.task", "hello"]);
  });

  it("maps skipped capture to an error", async () => {
    const client = {
      captureNow: async () => ok(null),
    };

    const res = await saveNoteSnapshot(client, "/tmp/TODO.md", "abc.task", "hello");
    expect(res.ok).toBe(false);
    if (!res.ok) {
      expect(res.error.type).toBe("skipped");
    }
  });

  it("propagates daemon_unavailable", async () => {
    const client = {
      captureNow: async () => err({ type: "daemon_unavailable" }),
    };

    const res = await saveNoteSnapshot(client, "/tmp/TODO.md", "abc.task", "hello");
    expect(res.ok).toBe(false);
    if (!res.ok) {
      expect(res.error.type).toBe("daemon_unavailable");
    }
  });
});
