import { describe, expect, it } from "bun:test";
import { forgetProjectContext, forgetTaskContext } from "../context-resurrection/forget";
import type { CrResult } from "../context-resurrection/types";

function ok<T>(value: T): CrResult<T> {
  return { ok: true, value };
}

function err<T>(error: any): CrResult<T> {
  return { ok: false, error };
}

describe("context-resurrection forget helpers", () => {
  it("forgetTaskContext() calls deleteTask and clears indicator for that task", async () => {
    const calls: any[] = [];
    const client = {
      deleteTask: async (projectPath: string, taskId: string) => {
        calls.push([projectPath, taskId]);
        return ok(3);
      },
      deleteProject: async () => ok(0),
    };

    const res = await forgetTaskContext(client, "/tmp/TODO.md", "abc.task", { "abc.task": true, "def.other": true });

    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.value.deletedCount).toBe(3);
      expect(res.value.next).toEqual({ "abc.task": false, "def.other": true });
    }

    expect(calls).toEqual([["/tmp/TODO.md", "abc.task"]]);
  });

  it("forgetProjectContext() clears all indicators", async () => {
    const client = {
      deleteTask: async () => ok(0),
      deleteProject: async () => ok(10),
    };

    const res = await forgetProjectContext(client, "/tmp/TODO.md");
    expect(res).toEqual({ ok: true, value: { deletedCount: 10, next: {} } });
  });

  it("propagates client errors", async () => {
    const client = {
      deleteTask: async () => err({ type: "daemon_unavailable" }),
      deleteProject: async () => err({ type: "daemon_unavailable" }),
    };

    const res = await forgetTaskContext(client, "/tmp/TODO.md", "abc.task", {});
    expect(res.ok).toBe(false);
    if (!res.ok) {
      expect(res.error.type).toBe("daemon_unavailable");
    }
  });
});
