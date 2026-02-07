import { describe, expect, it } from "bun:test";
import { DEFAULT_RESURRECTION_THRESHOLD_MS, selectCardData, shouldShowCard } from "../context-resurrection/selectors";
import type { ContextSnapshotV1, CrResult } from "../context-resurrection/types";

function ok<T>(value: T): CrResult<T> {
  return { ok: true, value };
}

describe("context-resurrection selectors", () => {
  it("shouldShowCard(): false when daemon unavailable", () => {
    const latest: CrResult<ContextSnapshotV1 | null> = {
      ok: false,
      error: { type: "daemon_unavailable" },
    };

    expect(shouldShowCard(latest)).toBe(false);
  });

  it("shouldShowCard(): false when no snapshot", () => {
    expect(shouldShowCard(ok(null))).toBe(false);
  });

  it("shouldShowCard(): false when snapshot is too recent", () => {
    const now = Date.now();
    const capturedAt = new Date(now - DEFAULT_RESURRECTION_THRESHOLD_MS + 1_000).toISOString();

    const snapshot: ContextSnapshotV1 = {
      id: "snap-1",
      version: 1,
      project_path: "/tmp/TODO.md",
      task_id: "abc.test-task",
      task_title_at_capture: "Test task",
      captured_at: capturedAt,
      capture_reason: "manual",
    };

    expect(shouldShowCard(ok(snapshot))).toBe(false);
  });

  it("shouldShowCard(): true when snapshot is older than threshold", () => {
    const now = Date.now();
    const capturedAt = new Date(now - DEFAULT_RESURRECTION_THRESHOLD_MS - 1_000).toISOString();

    const snapshot: ContextSnapshotV1 = {
      id: "snap-1",
      version: 1,
      project_path: "/tmp/TODO.md",
      task_id: "abc.test-task",
      task_title_at_capture: "Test task",
      captured_at: capturedAt,
      capture_reason: "manual",
    };

    expect(shouldShowCard(ok(snapshot))).toBe(true);
  });

  it("shouldShowCard(): lastActivityMs suppresses card if recent", () => {
    const now = Date.now();
    const capturedAt = new Date(now - DEFAULT_RESURRECTION_THRESHOLD_MS - 10_000).toISOString();

    const snapshot: ContextSnapshotV1 = {
      id: "snap-1",
      version: 1,
      project_path: "/tmp/TODO.md",
      task_id: "abc.test-task",
      task_title_at_capture: "Test task",
      captured_at: capturedAt,
      capture_reason: "manual",
    };

    const lastActivityMs = now - 5_000; // very recent
    expect(shouldShowCard(ok(snapshot), lastActivityMs)).toBe(false);
  });

  it("selectCardData(): shapes terminal tail + attention", () => {
    const tail = Array.from({ length: 25 }, (_, i) => `line-${i + 1}`).join("\n");

    const snapshot: ContextSnapshotV1 = {
      id: "snap-1",
      version: 1,
      project_path: "/tmp/TODO.md",
      task_id: "abc.test-task",
      task_title_at_capture: "Test task",
      captured_at: "2026-02-06T13:12:33Z",
      capture_reason: "session_stopped",
      user_note: "Remember the thing",
      terminal: {
        session_id: 123,
        status: "Stopped",
        exit_code: 0,
        last_attention: {
          attention_type: "error",
          preview: "error: build failed",
          triggered_at: "2026-02-06T13:12:00Z",
        },
        tail_inline: tail,
      },
    };

    const data = selectCardData(snapshot);

    expect(data.snapshotId).toBe("snap-1");
    expect(data.taskId).toBe("abc.test-task");
    expect(data.userNote).toBe("Remember the thing");

    expect(data.terminal?.sessionId).toBe(123);
    expect(data.terminal?.status).toBe("Stopped");
    expect(data.terminal?.tailExcerpt?.startsWith("line-6")).toBe(true); // last 20 lines of 25
    expect(data.terminal?.tailExcerpt?.includes("line-25")).toBe(true);
    expect(data.terminal?.lastAttention?.attention_type).toBe("error");
  });
});
