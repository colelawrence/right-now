import { describe, expect, it } from "bun:test";
import { CrClient, type CrTransport } from "../context-resurrection/client";
import type { ContextSnapshotV1, CrDaemonRequest, CrDaemonResponse } from "../context-resurrection/types";

const sampleSnapshot: ContextSnapshotV1 = {
  id: "2026-02-06T13:12:33Z_abc.test-task",
  version: 1,
  project_path: "/tmp/TODO.md",
  task_id: "abc.test-task",
  task_title_at_capture: "Test task",
  captured_at: "2026-02-06T13:12:33Z",
  capture_reason: "manual",
};

function mockTransport(resolver: (req: CrDaemonRequest) => CrDaemonResponse): {
  transport: CrTransport;
  calls: CrDaemonRequest[];
} {
  const calls: CrDaemonRequest[] = [];
  const transport: CrTransport = async (req) => {
    calls.push(req);
    return resolver(req);
  };
  return { transport, calls };
}

describe("CrClient", () => {
  it("latest(): returns Ok(snapshot) on cr_snapshot", async () => {
    const { transport, calls } = mockTransport(() => ({ type: "cr_snapshot", snapshot: sampleSnapshot }));
    const client = new CrClient(transport);

    const result = await client.latest("/tmp/TODO.md", "abc.test-task");

    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value?.task_id).toBe("abc.test-task");
    }

    expect(calls).toEqual([
      {
        type: "cr_latest",
        project_path: "/tmp/TODO.md",
        task_id: "abc.test-task",
      },
    ]);
  });

  it("latest(): maps not_found error code to Ok(null)", async () => {
    const { transport } = mockTransport(() => ({ type: "error", code: "not_found", message: "No snapshots found" }));
    const client = new CrClient(transport);

    const result = await client.latest("/tmp/TODO.md", "abc.test-task");

    expect(result).toEqual({ ok: true, value: null });
  });

  it("list(): returns Ok(snapshots) on cr_snapshots", async () => {
    const { transport, calls } = mockTransport(() => ({ type: "cr_snapshots", snapshots: [sampleSnapshot] }));
    const client = new CrClient(transport);

    const result = await client.list("/tmp/TODO.md", "abc.test-task", 10);

    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.value.length).toBe(1);
      expect(result.value[0]?.id).toBe(sampleSnapshot.id);
    }

    expect(calls[0]).toEqual({
      type: "cr_list",
      project_path: "/tmp/TODO.md",
      task_id: "abc.test-task",
      limit: 10,
    });
  });

  it("get(): maps not_found error code to Ok(null)", async () => {
    const { transport } = mockTransport(() => ({
      type: "error",
      code: "not_found",
      message: "Failed to read snapshot: not found",
    }));
    const client = new CrClient(transport);

    const result = await client.get("/tmp/TODO.md", "abc.test-task", "snap-1");
    expect(result).toEqual({ ok: true, value: null });
  });

  it("captureNow(): maps skipped error code to Ok(null)", async () => {
    const { transport } = mockTransport(() => ({
      type: "error",
      code: "skipped",
      message: "Capture was skipped (dedup/rate-limit)",
    }));
    const client = new CrClient(transport);

    const result = await client.captureNow("/tmp/TODO.md", "abc.test-task", "hello");
    expect(result).toEqual({ ok: true, value: null });
  });

  it("deleteTask(): returns deleted_count", async () => {
    const { transport } = mockTransport(() => ({ type: "cr_deleted", deleted_count: 3 }));
    const client = new CrClient(transport);

    const result = await client.deleteTask("/tmp/TODO.md", "abc.test-task");

    expect(result).toEqual({ ok: true, value: 3 });
  });

  it("handles daemon unavailable (transport throws)", async () => {
    const transport: CrTransport = async (_req) => {
      throw new Error("connect ECONNREFUSED");
    };

    const client = new CrClient(transport);

    const result = await client.latest("/tmp/TODO.md", "abc.test-task");

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.type).toBe("daemon_unavailable");
    }
  });
});
