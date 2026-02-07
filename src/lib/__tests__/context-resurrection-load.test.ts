import { describe, expect, it } from "bun:test";
import { type CrLatestClient, loadResurrectionState } from "../context-resurrection/load";
import { DEFAULT_RESURRECTION_THRESHOLD_MS } from "../context-resurrection/selectors";
import type { ContextSnapshotV1, CrResult } from "../context-resurrection/types";

function ok<T>(value: T): CrResult<T> {
  return { ok: true, value };
}

function err<T>(error: any): CrResult<T> {
  return { ok: false, error };
}

function makeSnapshot(taskId: string, capturedAtIso: string): ContextSnapshotV1 {
  return {
    id: `${capturedAtIso}_${taskId}`,
    version: 1,
    project_path: "/tmp/TODO.md",
    task_id: taskId,
    task_title_at_capture: "Task",
    captured_at: capturedAtIso,
    capture_reason: "manual",
  };
}

describe("loadResurrectionState", () => {
  it("selects activeTaskId snapshot when eligible", async () => {
    const now = Date.now();
    const capturedAt = new Date(now - DEFAULT_RESURRECTION_THRESHOLD_MS - 10_000).toISOString();

    const client: CrLatestClient = {
      latest: async (_projectPath, taskId) => {
        if (taskId === "abc.active") return ok(makeSnapshot("abc.active", capturedAt));
        return ok(null);
      },
    };

    const result = await loadResurrectionState({
      client,
      projectPath: "/tmp/TODO.md",
      activeTaskId: "abc.active",
      tasks: [{ taskId: "abc.active" }, { taskId: "def.other" }],
    });

    expect(result.daemonUnavailable).toBe(false);
    expect(result.taskHasContext["abc.active"]).toBe(true);
    expect(result.selected?.taskId).toBe("abc.active");
    expect(result.selected?.snapshot.task_id).toBe("abc.active");
  });

  it("does not select snapshot when too recent", async () => {
    const now = Date.now();
    const capturedAt = new Date(now - DEFAULT_RESURRECTION_THRESHOLD_MS + 10_000).toISOString();

    const client: CrLatestClient = {
      latest: async (_projectPath, taskId) => {
        if (taskId === "abc.active") return ok(makeSnapshot("abc.active", capturedAt));
        return ok(null);
      },
    };

    const result = await loadResurrectionState({
      client,
      projectPath: "/tmp/TODO.md",
      activeTaskId: "abc.active",
      tasks: [{ taskId: "abc.active" }],
    });

    expect(result.selected).toBe(null);
  });

  it("falls back to most recent snapshot across tasks when active task has none", async () => {
    const now = Date.now();
    const older = new Date(now - DEFAULT_RESURRECTION_THRESHOLD_MS - 50_000).toISOString();
    const newer = new Date(now - DEFAULT_RESURRECTION_THRESHOLD_MS - 10_000).toISOString();

    const client: CrLatestClient = {
      latest: async (_projectPath, taskId) => {
        if (taskId === "a.task") return ok(makeSnapshot("a.task", older));
        if (taskId === "b.task") return ok(makeSnapshot("b.task", newer));
        return ok(null);
      },
    };

    const result = await loadResurrectionState({
      client,
      projectPath: "/tmp/TODO.md",
      activeTaskId: "missing.active",
      tasks: [{ taskId: "a.task" }, { taskId: "b.task" }],
    });

    expect(result.selected?.taskId).toBe("b.task");
    expect(result.taskHasContext["a.task"]).toBe(true);
    expect(result.taskHasContext["b.task"]).toBe(true);
  });

  it("treats daemon_unavailable as a hard disabled state", async () => {
    const client: CrLatestClient = {
      latest: async () => err({ type: "daemon_unavailable" }),
    };

    const result = await loadResurrectionState({
      client,
      projectPath: "/tmp/TODO.md",
      activeTaskId: "abc.active",
      tasks: [{ taskId: "abc.active" }],
    });

    expect(result.daemonUnavailable).toBe(true);
    expect(result.selected).toBe(null);
    expect(Object.keys(result.taskHasContext).length).toBe(0);
  });
});
