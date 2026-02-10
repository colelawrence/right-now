import { describe, expect, it } from "bun:test";
import { formatAgentBrief } from "../context-resurrection/agent-brief";
import type { ContextSnapshotV1 } from "../context-resurrection/types";

function makeSnapshot(partial: Partial<ContextSnapshotV1> = {}): ContextSnapshotV1 {
  return {
    id: "snap-1",
    version: 1,
    project_path: "/Users/test/project/TODO.md",
    task_id: "abc.test-task",
    task_title_at_capture: "Test task",
    captured_at: "2026-02-07T00:00:00.000Z",
    capture_reason: "manual",
    ...partial,
  };
}

describe("formatAgentBrief", () => {
  it("includes core metadata + note + attention + tail excerpt", () => {
    const tail = ["line-1", "line-2", "line-3", "line-4", "line-5", "line-6"].join("\n");

    const snapshot = makeSnapshot({
      user_note: "Remember to check the failing test.",
      terminal: {
        session_id: 123,
        status: "Stopped",
        exit_code: 1,
        last_attention: {
          attention_type: "error",
          preview: "error: build failed",
          triggered_at: "2026-02-07T00:00:10.000Z",
        },
        tail_inline: tail,
      },
    });

    const brief = formatAgentBrief(snapshot, { maxTailLines: 3 });

    expect(brief).toContain("You are an AI coding assistant");
    expect(brief).toContain("Project (TODO.md): /Users/test/project/TODO.md");
    expect(brief).toContain("Task: abc.test-task");
    expect(brief).toContain("Title at capture: Test task");
    expect(brief).toContain("captured_at: 2026-02-07T00:00:00.000Z");
    expect(brief).toContain("reason: manual");

    expect(brief).toContain("User note:");
    expect(brief).toContain("Remember to check the failing test.");

    expect(brief).toContain("Terminal:");
    expect(brief).toContain("session_id: 123");
    expect(brief).toContain("status: Stopped");
    expect(brief).toContain("exit_code: 1");

    expect(brief).toContain("attention_type: error");
    expect(brief).toContain("error: build failed");

    // Tail excerpt should include only the last 3 lines.
    expect(brief).toContain("line-4");
    expect(brief).toContain("line-6");
    expect(brief).not.toContain("line-1");

    // Should end with newline (clipboard-friendly).
    expect(brief.endsWith("\n")).toBe(true);
  });

  it("handles missing optional fields", () => {
    const brief = formatAgentBrief(makeSnapshot({ terminal: undefined, user_note: undefined }), { maxTailLines: 10 });

    expect(brief).toContain("Terminal: (none captured)");
    expect(brief).toContain("Please respond with:");
  });
});
