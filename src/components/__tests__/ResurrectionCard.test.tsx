import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import type { ContextSnapshotV1 } from "../../lib/context-resurrection/types";
import { ResurrectionCard } from "../ResurrectionCard";

describe("ResurrectionCard", () => {
  it("renders snapshot content (task title, tail excerpt, note)", () => {
    const tail = Array.from({ length: 25 }, (_, i) => `line-${String(i + 1).padStart(2, "0")}`).join("\n");

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

    const html = renderToStaticMarkup(<ResurrectionCard snapshot={snapshot} onDismiss={() => {}} />);

    expect(html).toContain("Test task");
    expect(html).toContain("abc.test-task");
    expect(html).toContain("Remember the thing");

    // Tail excerpt should include last 20 lines only
    expect(html).toContain("line-25");
    expect(html).toContain("line-06");
    expect(html).not.toContain("line-05");

    // Attention preview should render
    expect(html).toContain("error: build failed");
  });

  it("does not crash when optional fields are missing", () => {
    const snapshot: ContextSnapshotV1 = {
      id: "snap-2",
      version: 1,
      project_path: "/tmp/TODO.md",
      task_id: "abc.missing-fields",
      task_title_at_capture: "No terminal",
      captured_at: "2026-02-06T13:12:33Z",
      capture_reason: "manual",
    };

    const html = renderToStaticMarkup(<ResurrectionCard snapshot={snapshot} onDismiss={() => {}} />);

    expect(html).toContain("No terminal");
    expect(html).toContain("No additional context captured");
  });
});
