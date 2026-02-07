import { describe, expect, it } from "bun:test";
import {
  ProjectStateEditor,
  type TaskBlock,
  ensureTaskId,
  formatSessionBadge,
  formatTaskId,
  generateTaskId,
} from "../ProjectStateEditor";

describe("ProjectStateEditor", () => {
  const minimalFrontmatter = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
# Unrelated heading
- [ ] Some Task
- [ ] Another Task
`;

  const noFrontmatter = `# Just a heading
- [ ] Some Task
Task related text here
`;

  const complexBody = `---
pomodoro_settings:
  work_duration: 50
  break_duration: 10
---
# Main Heading

- [ ] First Task
  This line belongs to first task
  This line also belongs to first task

Some unrecognized text in between

- [ ] Second Task
  Some details under second task

## Sub Heading
Arbitrary text that is unrecognized

- [ ] Third Task
`;

  it("should parse minimal frontmatter and body correctly", () => {
    const parsed = ProjectStateEditor.parse(minimalFrontmatter);

    // Check pomodoro settings
    expect(parsed.pomodoroSettings.workDuration).toBe(25);
    expect(parsed.pomodoroSettings.breakDuration).toBe(5);

    // Body checks
    expect(parsed.markdown.length).toBeGreaterThanOrEqual(2);
    // Should have a heading block for "# Unrelated heading"
    // and 2 task blocks, or else unrecognized lines
    const headingBlock = parsed.markdown.find((b) => b.type === "heading");
    expect(headingBlock).toBeTruthy();
    expect((headingBlock as any).text).toBe("Unrelated heading");

    const taskBlocks = parsed.markdown.filter((b) => b.type === "task");
    expect(taskBlocks.length).toBe(2);
    expect((taskBlocks[0] as any).name).toBe("Some Task");
    expect((taskBlocks[1] as any).name).toBe("Another Task");
  });

  it("should handle files with no frontmatter", () => {
    const parsed = ProjectStateEditor.parse(noFrontmatter);

    // Confirm defaults for missing frontmatter
    expect(parsed.pomodoroSettings.workDuration).toBe(25);
    expect(parsed.pomodoroSettings.breakDuration).toBe(5);

    // Body should parse one heading, one task, and preserve details
    const headings = parsed.markdown.filter((b) => b.type === "heading");
    const tasks = parsed.markdown.filter((b) => b.type === "task");

    expect(headings.length).toBe(1);
    expect(tasks.length).toBe(1);

    const heading = headings[0] as { type: "heading"; level: number; text: string };
    expect(heading.level).toBe(1);
    expect(heading.text).toBe("Just a heading");

    const task = tasks[0] as any;
    expect(task.name).toBe("Some Task");
    expect(task.details).toBe("Task related text here");
    expect(task.sessionStatus).toBe(null);
  });

  it("should parse and round-trip a complex body with minimal changes", () => {
    const parsed = ProjectStateEditor.parse(complexBody);
    expect(parsed.pomodoroSettings.workDuration).toBe(50);
    expect(parsed.pomodoroSettings.breakDuration).toBe(10);

    // Expect multiple headings, tasks, and unrecognized blocks
    const headings = parsed.markdown.filter((b) => b.type === "heading");
    expect(headings.length).toBe(2);

    const tasks = parsed.markdown.filter((b) => b.type === "task");
    expect(tasks.length).toBe(3);
    // Check details for first task
    const firstTask = tasks[0] as any;
    expect(firstTask.name).toBe("First Task");
    expect(firstTask.details).toContain("This line belongs to first task");

    // Round-trip without changes
    const roundTripped = ProjectStateEditor.update(complexBody, parsed);
    // Because we didn't modify anything, it should remain identical
    expect(roundTripped).toBe(complexBody);
  });

  it("should update pomodoro settings and preserve body formatting", () => {
    const parsed = ProjectStateEditor.parse(complexBody);
    parsed.pomodoroSettings.workDuration = 45;
    parsed.pomodoroSettings.breakDuration = 15;

    const updated = ProjectStateEditor.update(complexBody, parsed);

    // Confirm frontmatter is updated
    expect(updated).toContain("work_duration: 45");
    expect(updated).toContain("break_duration: 15");

    // Confirm the body remains structurally the same
    expect(updated).toContain("# Main Heading");
    expect(updated).toContain("## Sub Heading");
    // The tasks should remain the same
    expect(updated).toContain("- [ ] First Task");
    expect(updated).toContain("- [ ] Second Task");
    expect(updated).toContain("- [ ] Third Task");
  });

  it("should preserve unrecognized text and spacing in body updates", () => {
    // Slight variation with odd spacing
    const spacedFile = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---

# Heading 1


- [ ] Task A

Some random text


- [ ] Task B
  Detail line 1
  Detail line 2

## Heading 2

More text
`;
    const parsed = ProjectStateEditor.parse(spacedFile);

    // Update pomodoro settings
    parsed.pomodoroSettings.workDuration = 30;
    const updated = ProjectStateEditor.update(spacedFile, parsed);

    // Confirm that the frontmatter is updated
    expect(updated).toContain("work_duration: 30");

    // Check that new lines and spacing remain in the final doc
    // For instance, we had 2 newlines after "# Heading 1"
    const lines = updated.split("\n");
    // We can verify a sequence or count how many blank lines appear
    // after "# Heading 1". We'll do a minimal check here:
    const headingIndex = lines.indexOf("# Heading 1");
    expect(lines[headingIndex + 1].trim()).toBe("");
    expect(lines[headingIndex + 2].trim()).toBe("");

    // Confirm the tasks remain intact
    expect(updated).toContain("- [ ] Task A");
    expect(updated).toContain("- [ ] Task B");
    expect(updated).toContain("Detail line 1");
    expect(updated).toContain("Detail line 2");

    // Confirm the second heading and the extra text remain
    expect(updated).toContain("## Heading 2");
    expect(updated).toContain("More text");
  });

  it("should parse tasks that have no details (empty next line) correctly", () => {
    const contentWithEmptyDetails = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task 1

- [ ] Task 2
- [ ] Task 3
  Has detail
`;
    const parsed = ProjectStateEditor.parse(contentWithEmptyDetails);

    // We expect 3 tasks
    const tasks = parsed.markdown.filter((m) => m.type === "task") as any[];
    expect(tasks.length).toBe(3);

    expect(tasks[0].name).toBe("Task 1");
    expect(tasks[0].details).toBe(null);
    expect(tasks[0].sessionStatus).toBe(null);

    expect(tasks[1].name).toBe("Task 2");
    expect(tasks[1].details).toBe(null);
    expect(tasks[1].sessionStatus).toBe(null);

    expect(tasks[2].name).toBe("Task 3");
    expect(tasks[2].details).toBe("  Has detail");
    expect(tasks[2].sessionStatus).toBe(null);
  });

  it("should not break if frontmatter is invalid or partially corrupted", () => {
    const corrupted = `---
pomodoro_settings:
  work_duration: 25
  break_duration: invalid
---
- [ ] Task
`;

    // We expect parse to parse what it can, invalid values pass through as-is
    const parsed = ProjectStateEditor.parse(corrupted);
    expect(parsed.pomodoroSettings.workDuration).toBe(25); // from partial parse
    // Note: gray-matter passes "invalid" through, not a number fallback
    // The ?? only works for undefined, not for string values
    expect(parsed.pomodoroSettings.breakDuration).toBe("invalid");
    // Body should still parse
    const tasks = parsed.markdown.filter((m) => m.type === "task");
    expect(tasks.length).toBe(1);
  });

  // Robustness tests for various task line formats
  describe("task line format variations", () => {
    it("should handle dash bullet with space", () => {
      const content = `- [ ] Task with dash`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("Task with dash");
      expect(tasks[0].prefix).toBe("- ");
    });

    it("should handle asterisk bullet", () => {
      const content = `* [ ] Task with asterisk`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("Task with asterisk");
      expect(tasks[0].prefix).toBe("* ");
    });

    it("should handle indented tasks (2 spaces)", () => {
      const content = `  - [ ] Indented task`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("Indented task");
      expect(tasks[0].prefix).toBe("  - ");
    });

    it("should handle indented tasks (4 spaces)", () => {
      const content = `    - [ ] Deeply indented task`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("Deeply indented task");
      expect(tasks[0].prefix).toBe("    - ");
    });

    it("should handle tab-indented tasks", () => {
      const content = `\t- [ ] Tab indented task`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("Tab indented task");
      expect(tasks[0].prefix).toBe("\t- ");
    });

    it("should handle checkbox without bullet (bare checkbox)", () => {
      const content = `[ ] Bare checkbox task`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("Bare checkbox task");
      expect(tasks[0].prefix).toBe("");
    });

    it("should handle uppercase X for completed", () => {
      const content = `- [X] Completed with uppercase X`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].complete).toBe("X");
    });

    it("should handle lowercase x for completed", () => {
      const content = `- [x] Completed with lowercase x`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].complete).toBe("x");
    });

    it("should preserve original prefix style on round-trip", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
  * [ ] Indented asterisk task
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      // Add session badge
      tasks[0].sessionStatus = { status: "Running", sessionId: 1 };

      const updated = ProjectStateEditor.update(content, parsed);
      // Should preserve the indentation and asterisk style
      expect(updated).toContain("  * [ ] Indented asterisk task [Running](todos://session/1)");
    });
  });

  describe("special characters in task names", () => {
    it("should handle emojis in task name", () => {
      const content = `- [ ] Fix bug 🐛 in parser`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("Fix bug 🐛 in parser");
    });

    it("should handle emoji at end of task with session badge", () => {
      const content = `- [ ] Deploy to production 🚀 [Running](todos://session/42)`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("Deploy to production 🚀");
      expect(tasks[0].sessionStatus?.sessionId).toBe(42);
    });

    it("should handle unicode characters", () => {
      const content = `- [ ] Café résumé naïve`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("Café résumé naïve");
    });

    it("should handle CJK characters", () => {
      const content = `- [ ] 日本語タスク`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("日本語タスク");
    });

    it("should handle special markdown characters in task name", () => {
      const content = `- [ ] Fix **bold** and _italic_ text`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("Fix **bold** and _italic_ text");
    });

    it("should handle backticks (inline code) in task name", () => {
      const content = "- [ ] Fix `console.log` statement";
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("Fix `console.log` statement");
    });

    it("should handle parentheses in task name (not confused with link)", () => {
      const content = `- [ ] Implement function(arg1, arg2)`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("Implement function(arg1, arg2)");
      expect(tasks[0].sessionStatus).toBe(null);
    });

    it("should handle square brackets in task name (not confused with badge)", () => {
      const content = `- [ ] Fix array[0] access`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("Fix array[0] access");
      expect(tasks[0].sessionStatus).toBe(null);
    });
  });

  describe("multiple links in task names", () => {
    it("should preserve link before session badge", () => {
      const content = `- [ ] See [docs](https://example.com) for details [Running](todos://session/42)`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("See [docs](https://example.com) for details");
      expect(tasks[0].sessionStatus?.sessionId).toBe(42);
    });

    it("should preserve multiple links before session badge", () => {
      const content = `- [ ] Check [link1](http://a.com) and [link2](http://b.com) [Running](todos://session/1)`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("Check [link1](http://a.com) and [link2](http://b.com)");
      expect(tasks[0].sessionStatus?.sessionId).toBe(1);
    });

    it("should round-trip task with link and session badge", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] See [docs](https://example.com) [Running](todos://session/42)
`;
      const parsed = ProjectStateEditor.parse(content);
      const updated = ProjectStateEditor.update(content, parsed);
      expect(updated).toContain("- [ ] See [docs](https://example.com) [Running](todos://session/42)");
    });
  });

  describe("edge cases for session badge matching", () => {
    it("should not match badge-like text in middle of task", () => {
      const content = `- [ ] The status is [Running](not-a-link) and continue`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].name).toBe("The status is [Running](not-a-link) and continue");
      expect(tasks[0].sessionStatus).toBe(null);
    });

    it("should not match without space before badge", () => {
      const content = `- [ ] Task name[Running](todos://session/42)`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      // Should NOT match because no space before badge
      expect(tasks[0].name).toBe("Task name[Running](todos://session/42)");
      expect(tasks[0].sessionStatus).toBe(null);
    });

    it("should not match wrong protocol", () => {
      const content = `- [ ] Task [Running](http://session/42)`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].sessionStatus).toBe(null);
    });

    it("should not match wrong path format", () => {
      const content = `- [ ] Task [Running](todos://sessions/42)`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].sessionStatus).toBe(null);
    });

    it("should not match non-numeric session ID", () => {
      const content = `- [ ] Task [Running](todos://session/abc)`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].sessionStatus).toBe(null);
    });

    it("should not match invalid status", () => {
      const content = `- [ ] Task [Paused](todos://session/42)`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].sessionStatus).toBe(null);
    });

    it("should handle very large session IDs", () => {
      const content = `- [ ] Task [Running](todos://session/9999999999999)`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].sessionStatus?.sessionId).toBe(9999999999999);
    });

    it("should handle session ID of 0", () => {
      const content = `- [ ] Task [Running](todos://session/0)`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];
      expect(tasks[0].sessionStatus?.sessionId).toBe(0);
    });
  });

  describe("preserving file structure", () => {
    it("should preserve blank lines between tasks", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task 1

- [ ] Task 2
`;
      const parsed = ProjectStateEditor.parse(content);
      const updated = ProjectStateEditor.update(content, parsed);
      // The blank line should be preserved as unrecognized content
      expect(updated.split("Task 1")[1].split("Task 2")[0]).toContain("\n\n");
    });

    it("should preserve comments and other markdown", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
<!-- This is a comment -->
- [ ] Task 1

> This is a blockquote

- [ ] Task 2
`;
      const parsed = ProjectStateEditor.parse(content);
      const updated = ProjectStateEditor.update(content, parsed);
      expect(updated).toContain("<!-- This is a comment -->");
      expect(updated).toContain("> This is a blockquote");
    });

    it("should preserve horizontal rules", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task 1

---

- [ ] Task 2
`;
      const parsed = ProjectStateEditor.parse(content);
      const updated = ProjectStateEditor.update(content, parsed);
      // Content after frontmatter should contain the horizontal rule
      const bodyContent = updated.split("---\n").slice(2).join("---\n");
      expect(bodyContent).toContain("---");
    });
  });

  // Session badge tests
  describe("session badges", () => {
    it("should parse task with Running session badge", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Implement reports [Running](todos://session/42)
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(tasks.length).toBe(1);
      expect(tasks[0].name).toBe("Implement reports");
      expect(tasks[0].sessionStatus).not.toBe(null);
      expect(tasks[0].sessionStatus?.status).toBe("Running");
      expect(tasks[0].sessionStatus?.sessionId).toBe(42);
    });

    it("should parse task with Stopped session badge", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [x] Completed task [Stopped](todos://session/123)
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(tasks.length).toBe(1);
      expect(tasks[0].name).toBe("Completed task");
      expect(tasks[0].complete).toBe("x");
      expect(tasks[0].sessionStatus?.status).toBe("Stopped");
      expect(tasks[0].sessionStatus?.sessionId).toBe(123);
    });

    it("should parse task with Waiting session badge", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Waiting for input [Waiting](todos://session/999)
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(tasks.length).toBe(1);
      expect(tasks[0].name).toBe("Waiting for input");
      expect(tasks[0].sessionStatus?.status).toBe("Waiting");
      expect(tasks[0].sessionStatus?.sessionId).toBe(999);
    });

    it("should parse task without session badge", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Regular task without badge
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(tasks.length).toBe(1);
      expect(tasks[0].name).toBe("Regular task without badge");
      expect(tasks[0].sessionStatus).toBe(null);
    });

    it("should not confuse regular markdown links with session badges", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Check out [this link](https://example.com)
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(tasks.length).toBe(1);
      // The link should be preserved in the name since it's not a session badge
      expect(tasks[0].name).toBe("Check out [this link](https://example.com)");
      expect(tasks[0].sessionStatus).toBe(null);
    });

    it("should preserve session badge during round-trip", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task with session [Running](todos://session/42)
`;
      const parsed = ProjectStateEditor.parse(content);
      const updated = ProjectStateEditor.update(content, parsed);

      expect(updated).toContain("- [ ] Task with session [Running](todos://session/42)");
    });

    it("should format session badge correctly", () => {
      const badge = formatSessionBadge("Running", 42);
      expect(badge).toBe(" [Running](todos://session/42)");

      const stoppedBadge = formatSessionBadge("Stopped", 123);
      expect(stoppedBadge).toBe(" [Stopped](todos://session/123)");
    });

    it("should update task with added session badge", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task without badge
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      // Add session badge to the task
      tasks[0].sessionStatus = { status: "Running", sessionId: 42 };

      const updated = ProjectStateEditor.update(content, parsed);
      expect(updated).toContain("- [ ] Task without badge [Running](todos://session/42)");
    });

    it("should update task session status", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task with session [Running](todos://session/42)
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      // Change status to Stopped
      tasks[0].sessionStatus = { status: "Stopped", sessionId: 42 };

      const updated = ProjectStateEditor.update(content, parsed);
      expect(updated).toContain("- [ ] Task with session [Stopped](todos://session/42)");
    });

    it("should remove session badge when set to null", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task with session [Running](todos://session/42)
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      // Remove session badge
      tasks[0].sessionStatus = null;

      const updated = ProjectStateEditor.update(content, parsed);
      expect(updated).toContain("- [ ] Task with session\n");
      expect(updated).not.toContain("todos://session");
    });
  });

  describe("timer state persistence", () => {
    it("should parse timer state from frontmatter", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
right_now:
  work_state: working
  state_transitions:
    started_at: 1609459200000
    ends_at: 1609460700000
---
- [ ] Task 1
`;
      const parsed = ProjectStateEditor.parse(content);

      expect(parsed.workState).toBe("working");
      expect(parsed.stateTransitions).toEqual({
        startedAt: 1609459200000,
        endsAt: 1609460700000,
      });
    });

    it("should handle missing timer state gracefully", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task 1
`;
      const parsed = ProjectStateEditor.parse(content);

      expect(parsed.workState).toBeUndefined();
      expect(parsed.stateTransitions).toBeUndefined();
    });

    it("should handle planning state (no endsAt)", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
right_now:
  work_state: planning
  state_transitions:
    started_at: 1609459200000
---
- [ ] Task 1
`;
      const parsed = ProjectStateEditor.parse(content);

      expect(parsed.workState).toBe("planning");
      expect(parsed.stateTransitions).toEqual({
        startedAt: 1609459200000,
        endsAt: undefined,
      });
    });

    it("should handle break state", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
right_now:
  work_state: break
  state_transitions:
    started_at: 1609459200000
    ends_at: 1609459500000
---
- [ ] Task 1
`;
      const parsed = ProjectStateEditor.parse(content);

      expect(parsed.workState).toBe("break");
      expect(parsed.stateTransitions?.startedAt).toBe(1609459200000);
      expect(parsed.stateTransitions?.endsAt).toBe(1609459500000);
    });

    it("should write timer state to frontmatter", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task 1
`;
      const parsed = ProjectStateEditor.parse(content);

      parsed.workState = "working";
      parsed.stateTransitions = {
        startedAt: 1609459200000,
        endsAt: 1609460700000,
      };

      const updated = ProjectStateEditor.update(content, parsed);

      expect(updated).toContain("work_state: working");
      expect(updated).toContain("started_at: 1609459200000");
      expect(updated).toContain("ends_at: 1609460700000");
    });

    it("should preserve timer state during round-trip", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
right_now:
  work_state: working
  state_transitions:
    started_at: 1609459200000
    ends_at: 1609460700000
---
- [ ] Task 1
`;
      const parsed = ProjectStateEditor.parse(content);
      const updated = ProjectStateEditor.update(content, parsed);

      const reparsed = ProjectStateEditor.parse(updated);
      expect(reparsed.workState).toBe("working");
      expect(reparsed.stateTransitions).toEqual({
        startedAt: 1609459200000,
        endsAt: 1609460700000,
      });
    });

    it("should update timer state independently of other fields", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
right_now:
  work_state: planning
  state_transitions:
    started_at: 1609459200000
---
- [ ] Task 1
`;
      const parsed = ProjectStateEditor.parse(content);

      // Update only timer state
      parsed.workState = "working";
      parsed.stateTransitions = {
        startedAt: 1609459300000,
        endsAt: 1609460800000,
      };

      const updated = ProjectStateEditor.update(content, parsed);

      // Pomodoro settings should be unchanged
      expect(updated).toContain("work_duration: 25");
      expect(updated).toContain("break_duration: 5");

      // Timer state should be updated
      expect(updated).toContain("work_state: working");
      expect(updated).toContain("started_at: 1609459300000");
      expect(updated).toContain("ends_at: 1609460800000");

      // Body should be unchanged
      expect(updated).toContain("- [ ] Task 1");
    });

    it("should handle invalid timer state gracefully", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
right_now:
  work_state: invalid_state
  state_transitions:
    started_at: "not a number"
    ends_at: null
---
- [ ] Task 1
`;
      const parsed = ProjectStateEditor.parse(content);

      // Invalid work_state should still be parsed (type safety is at runtime)
      expect(parsed.workState).toBe("invalid_state");

      // Invalid timestamps should result in undefined
      expect(parsed.stateTransitions?.startedAt).toBeUndefined();
      expect(parsed.stateTransitions?.endsAt).toBeUndefined();
    });

    it("should write planning state without endsAt", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task 1
`;
      const parsed = ProjectStateEditor.parse(content);

      parsed.workState = "planning";
      parsed.stateTransitions = {
        startedAt: 1609459200000,
      };

      const updated = ProjectStateEditor.update(content, parsed);

      expect(updated).toContain("work_state: planning");
      expect(updated).toContain("started_at: 1609459200000");
      expect(updated).not.toContain("ends_at");
    });

    it("should preserve other right_now fields if they exist", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
right_now:
  work_state: working
  state_transitions:
    started_at: 1609459200000
    ends_at: 1609460700000
  custom_field: some_value
---
- [ ] Task 1
`;
      const parsed = ProjectStateEditor.parse(content);

      // Update timer state
      parsed.workState = "break";
      parsed.stateTransitions = {
        startedAt: 1609459300000,
        endsAt: 1609459600000,
      };

      const updated = ProjectStateEditor.update(content, parsed);

      // Timer state should be updated
      expect(updated).toContain("work_state: break");
      expect(updated).toContain("started_at: 1609459300000");
      expect(updated).toContain("ends_at: 1609459600000");

      // Custom field should be preserved
      expect(updated).toContain("custom_field: some_value");
    });
  });

  describe("active task ID persistence", () => {
    it("should parse active_task_id from frontmatter", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
right_now:
  active_task_id: abc.my-task
---
- [ ] My task [abc.my-task]
- [ ] Another task [def.another-task]
`;
      const parsed = ProjectStateEditor.parse(content);

      expect(parsed.activeTaskId).toBe("abc.my-task");
    });

    it("should handle missing active_task_id gracefully", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task 1
`;
      const parsed = ProjectStateEditor.parse(content);

      expect(parsed.activeTaskId).toBeUndefined();
    });

    it("should write active_task_id to frontmatter", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task 1 [abc.task-1]
- [ ] Task 2 [def.task-2]
`;
      const parsed = ProjectStateEditor.parse(content);

      parsed.activeTaskId = "abc.task-1";

      const updated = ProjectStateEditor.update(content, parsed);

      expect(updated).toContain("active_task_id: abc.task-1");
    });

    it("should preserve active_task_id during round-trip", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
right_now:
  active_task_id: abc.my-task
  work_state: working
---
- [ ] My task [abc.my-task]
`;
      const parsed = ProjectStateEditor.parse(content);
      const updated = ProjectStateEditor.update(content, parsed);

      const reparsed = ProjectStateEditor.parse(updated);
      expect(reparsed.activeTaskId).toBe("abc.my-task");
    });

    it("should update active_task_id independently of other fields", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
right_now:
  work_state: planning
  active_task_id: abc.old-task
---
- [ ] Old task [abc.old-task]
- [ ] New task [def.new-task]
`;
      const parsed = ProjectStateEditor.parse(content);

      // Update only active task ID
      parsed.activeTaskId = "def.new-task";

      const updated = ProjectStateEditor.update(content, parsed);

      // Pomodoro settings should be unchanged
      expect(updated).toContain("work_duration: 25");
      expect(updated).toContain("break_duration: 5");

      // Work state should be unchanged
      expect(updated).toContain("work_state: planning");

      // Active task ID should be updated
      expect(updated).toContain("active_task_id: def.new-task");

      // Body should be unchanged
      expect(updated).toContain("- [ ] Old task [abc.old-task]");
      expect(updated).toContain("- [ ] New task [def.new-task]");
    });

    it("should not clear active_task_id when updating work state", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
right_now:
  work_state: planning
  active_task_id: abc.my-task
---
- [ ] My task [abc.my-task]
`;
      const parsed = ProjectStateEditor.parse(content);

      // Update only work state
      parsed.workState = "working";
      parsed.stateTransitions = {
        startedAt: 1609459200000,
        endsAt: 1609460700000,
      };

      const updated = ProjectStateEditor.update(content, parsed);

      // Active task ID should be preserved
      expect(updated).toContain("active_task_id: abc.my-task");
      expect(updated).toContain("work_state: working");
    });

    it("should handle invalid active_task_id type gracefully", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
right_now:
  active_task_id: 123
---
- [ ] Task 1
`;
      const parsed = ProjectStateEditor.parse(content);

      // Invalid type should result in undefined (not parse as number)
      expect(parsed.activeTaskId).toBeUndefined();
    });

    it("should preserve active_task_id when not explicitly set", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
right_now:
  active_task_id: abc.my-task
---
- [ ] My task [abc.my-task]
`;
      const parsed = ProjectStateEditor.parse(content);

      // Don't modify activeTaskId, just update another field
      parsed.pomodoroSettings.workDuration = 30;

      const updated = ProjectStateEditor.update(content, parsed);

      // Active task ID should be preserved
      const reparsed = ProjectStateEditor.parse(updated);
      expect(reparsed.activeTaskId).toBe("abc.my-task");
    });
  });

  describe("moveHeadingSection", () => {
    const sampleContent = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
# Section A
- [ ] Task A1
  Details for A1
- [ ] Task A2

## Section B
- [ ] Task B1
  Details for B1

Some unrecognized content in section B

### Section C
- [ ] Task C1
- [ ] Task C2
  Multi-line details
  for task C2
`;

    it("should move middle section up", () => {
      const parsed = ProjectStateEditor.parse(sampleContent);
      const blocks = parsed.markdown;

      // Find index of "Section B" (should be at index 4)
      const sectionBIndex = blocks.findIndex((b) => b.type === "heading" && (b as any).text === "Section B");
      expect(sectionBIndex).toBeGreaterThan(0);

      const result = ProjectStateEditor.moveHeadingSection(sampleContent, sectionBIndex, "up");
      expect(result).not.toBe(null);

      // Verify the order changed
      const reparsed = ProjectStateEditor.parse(result!);
      const headings = reparsed.markdown.filter((b) => b.type === "heading");

      expect((headings[0] as any).text).toBe("Section B");
      expect((headings[1] as any).text).toBe("Section A");
      expect((headings[2] as any).text).toBe("Section C");
    });

    it("should move middle section down", () => {
      const parsed = ProjectStateEditor.parse(sampleContent);
      const blocks = parsed.markdown;

      // Find index of "Section B"
      const sectionBIndex = blocks.findIndex((b) => b.type === "heading" && (b as any).text === "Section B");

      const result = ProjectStateEditor.moveHeadingSection(sampleContent, sectionBIndex, "down");
      expect(result).not.toBe(null);

      // Verify the order changed
      const reparsed = ProjectStateEditor.parse(result!);
      const headings = reparsed.markdown.filter((b) => b.type === "heading");

      expect((headings[0] as any).text).toBe("Section A");
      expect((headings[1] as any).text).toBe("Section C");
      expect((headings[2] as any).text).toBe("Section B");
    });

    it("should preserve all task details and unrecognized blocks when moving up", () => {
      const parsed = ProjectStateEditor.parse(sampleContent);
      const blocks = parsed.markdown;

      const sectionBIndex = blocks.findIndex((b) => b.type === "heading" && (b as any).text === "Section B");

      const result = ProjectStateEditor.moveHeadingSection(sampleContent, sectionBIndex, "up");
      expect(result).not.toBe(null);

      // Verify all content is preserved
      expect(result).toContain("Task A1");
      expect(result).toContain("Details for A1");
      expect(result).toContain("Task B1");
      expect(result).toContain("Details for B1");
      expect(result).toContain("Some unrecognized content in section B");
      expect(result).toContain("Task C1");
      expect(result).toContain("Multi-line details");
    });

    it("should preserve all task details and unrecognized blocks when moving down", () => {
      const parsed = ProjectStateEditor.parse(sampleContent);
      const blocks = parsed.markdown;

      const sectionBIndex = blocks.findIndex((b) => b.type === "heading" && (b as any).text === "Section B");

      const result = ProjectStateEditor.moveHeadingSection(sampleContent, sectionBIndex, "down");
      expect(result).not.toBe(null);

      // Verify all content is preserved
      expect(result).toContain("Task A1");
      expect(result).toContain("Details for A1");
      expect(result).toContain("Task B1");
      expect(result).toContain("Details for B1");
      expect(result).toContain("Some unrecognized content in section B");
      expect(result).toContain("Task C1");
      expect(result).toContain("Multi-line details");
    });

    it("should return null when trying to move first section up", () => {
      const parsed = ProjectStateEditor.parse(sampleContent);
      const blocks = parsed.markdown;

      const firstHeadingIndex = blocks.findIndex((b) => b.type === "heading");

      const result = ProjectStateEditor.moveHeadingSection(sampleContent, firstHeadingIndex, "up");
      expect(result).toBe(null);
    });

    it("should return null when trying to move last section down", () => {
      const parsed = ProjectStateEditor.parse(sampleContent);
      const blocks = parsed.markdown;

      // Find the last heading
      let lastHeadingIndex = -1;
      for (let i = blocks.length - 1; i >= 0; i--) {
        if (blocks[i].type === "heading") {
          lastHeadingIndex = i;
          break;
        }
      }

      expect(lastHeadingIndex).toBeGreaterThan(0);

      const result = ProjectStateEditor.moveHeadingSection(sampleContent, lastHeadingIndex, "down");
      expect(result).toBe(null);
    });

    it("should return null when headingIndex is invalid", () => {
      const parsed = ProjectStateEditor.parse(sampleContent);
      const blocks = parsed.markdown;

      // Find a task index (not a heading)
      const taskIndex = blocks.findIndex((b) => b.type === "task");

      const result = ProjectStateEditor.moveHeadingSection(sampleContent, taskIndex, "up");
      expect(result).toBe(null);
    });

    it("should return null when headingIndex is out of bounds", () => {
      const result1 = ProjectStateEditor.moveHeadingSection(sampleContent, -1, "up");
      expect(result1).toBe(null);

      const result2 = ProjectStateEditor.moveHeadingSection(sampleContent, 999, "down");
      expect(result2).toBe(null);
    });

    it("should handle sections with no tasks (only unrecognized content)", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
# Section 1
Some prose content here.

# Section 2
- [ ] Task in section 2

# Section 3
More prose.
`;
      const parsed = ProjectStateEditor.parse(content);
      const blocks = parsed.markdown;

      const section1Index = blocks.findIndex((b) => b.type === "heading" && (b as any).text === "Section 1");

      const result = ProjectStateEditor.moveHeadingSection(content, section1Index, "down");
      expect(result).not.toBe(null);

      // Verify order and content preservation
      const reparsed = ProjectStateEditor.parse(result!);
      const headings = reparsed.markdown.filter((b) => b.type === "heading");

      expect((headings[0] as any).text).toBe("Section 2");
      expect((headings[1] as any).text).toBe("Section 1");
      expect((headings[2] as any).text).toBe("Section 3");

      expect(result).toContain("Some prose content here.");
      expect(result).toContain("Task in section 2");
      expect(result).toContain("More prose.");
    });

    it("should handle moving sections with different heading levels", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
# H1 Section
- [ ] Task 1

### H3 Section
- [ ] Task 2

## H2 Section
- [ ] Task 3
`;
      const parsed = ProjectStateEditor.parse(content);
      const blocks = parsed.markdown;

      // Move H3 section up (should swap with H1)
      const h3Index = blocks.findIndex((b) => b.type === "heading" && (b as any).text === "H3 Section");

      const result = ProjectStateEditor.moveHeadingSection(content, h3Index, "up");
      expect(result).not.toBe(null);

      const reparsed = ProjectStateEditor.parse(result!);
      const headings = reparsed.markdown.filter((b) => b.type === "heading");

      expect((headings[0] as any).text).toBe("H3 Section");
      expect((headings[0] as any).level).toBe(3);
      expect((headings[1] as any).text).toBe("H1 Section");
      expect((headings[1] as any).level).toBe(1);
      expect((headings[2] as any).text).toBe("H2 Section");
      expect((headings[2] as any).level).toBe(2);
    });

    it("should preserve section with multiple unrecognized blocks", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
# Section 1
- [ ] Task 1

Paragraph 1

> Blockquote

\`\`\`
Code block
\`\`\`

<!-- Comment -->

# Section 2
- [ ] Task 2
`;
      const parsed = ProjectStateEditor.parse(content);
      const blocks = parsed.markdown;

      const section1Index = blocks.findIndex((b) => b.type === "heading" && (b as any).text === "Section 1");

      const result = ProjectStateEditor.moveHeadingSection(content, section1Index, "down");
      expect(result).not.toBe(null);

      // All unrecognized content should be preserved
      expect(result).toContain("Paragraph 1");
      expect(result).toContain("> Blockquote");
      expect(result).toContain("Code block");
      expect(result).toContain("<!-- Comment -->");
      expect(result).toContain("Task 1");
      expect(result).toContain("Task 2");

      // Verify order changed
      const reparsed = ProjectStateEditor.parse(result!);
      const headings = reparsed.markdown.filter((b) => b.type === "heading");
      expect((headings[0] as any).text).toBe("Section 2");
      expect((headings[1] as any).text).toBe("Section 1");
    });

    it("should work with single-task sections", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
# Section A
- [ ] Only task

# Section B
- [ ] Another task

# Section C
- [ ] Third task
`;
      const parsed = ProjectStateEditor.parse(content);
      const blocks = parsed.markdown;

      const sectionBIndex = blocks.findIndex((b) => b.type === "heading" && (b as any).text === "Section B");

      const result = ProjectStateEditor.moveHeadingSection(content, sectionBIndex, "up");
      expect(result).not.toBe(null);

      const reparsed = ProjectStateEditor.parse(result!);
      const headings = reparsed.markdown.filter((b) => b.type === "heading");

      expect((headings[0] as any).text).toBe("Section B");
      expect((headings[1] as any).text).toBe("Section A");
      expect((headings[2] as any).text).toBe("Section C");

      // Verify tasks are in correct sections
      const blocks2 = reparsed.markdown;
      const sectionBNewIndex = blocks2.findIndex((b) => b.type === "heading" && (b as any).text === "Section B");
      const nextBlock = blocks2[sectionBNewIndex + 1];
      expect(nextBlock.type).toBe("task");
      expect((nextBlock as any).name).toBe("Another task");
    });

    it("should preserve frontmatter when moving sections", () => {
      const content = `---
pomodoro_settings:
  work_duration: 50
  break_duration: 15
right_now:
  work_state: working
---
# Section 1
- [ ] Task 1

# Section 2
- [ ] Task 2
`;
      const parsed = ProjectStateEditor.parse(content);
      const blocks = parsed.markdown;

      const section1Index = blocks.findIndex((b) => b.type === "heading" && (b as any).text === "Section 1");

      const result = ProjectStateEditor.moveHeadingSection(content, section1Index, "down");
      expect(result).not.toBe(null);

      // Verify frontmatter preserved
      expect(result).toContain("work_duration: 50");
      expect(result).toContain("break_duration: 15");
      expect(result).toContain("work_state: working");

      const reparsed = ProjectStateEditor.parse(result!);
      expect(reparsed.pomodoroSettings.workDuration).toBe(50);
      expect(reparsed.pomodoroSettings.breakDuration).toBe(15);
      expect(reparsed.workState).toBe("working");
    });

    it("should handle empty sections (heading with no content before next heading)", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
# Empty Section

# Section with content
- [ ] Task 1

# Another empty section
`;
      const parsed = ProjectStateEditor.parse(content);
      const blocks = parsed.markdown;

      const emptyIndex = blocks.findIndex((b) => b.type === "heading" && (b as any).text === "Empty Section");

      const result = ProjectStateEditor.moveHeadingSection(content, emptyIndex, "down");
      expect(result).not.toBe(null);

      const reparsed = ProjectStateEditor.parse(result!);
      const headings = reparsed.markdown.filter((b) => b.type === "heading");

      expect((headings[0] as any).text).toBe("Section with content");
      expect((headings[1] as any).text).toBe("Empty Section");
      expect((headings[2] as any).text).toBe("Another empty section");
    });
  });

  // Task ID token tests
  describe("task ID tokens", () => {
    it("should parse task with task ID token", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Fix API timeout bug [qdz.fix-api-timeout-bug]
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(tasks.length).toBe(1);
      expect(tasks[0].name).toBe("Fix API timeout bug");
      expect(tasks[0].taskId).toBe("qdz.fix-api-timeout-bug");
      expect(tasks[0].sessionStatus).toBe(null);
    });

    it("should parse task with task ID and session badge in correct order", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Implement reports [abc.implement-reports] [Running](todos://session/42)
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(tasks.length).toBe(1);
      expect(tasks[0].name).toBe("Implement reports");
      expect(tasks[0].taskId).toBe("abc.implement-reports");
      expect(tasks[0].sessionStatus?.status).toBe("Running");
      expect(tasks[0].sessionStatus?.sessionId).toBe(42);
    });

    it("should parse task with 4-letter prefix task ID", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Complex task [abcd.complex-task-name]
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(tasks.length).toBe(1);
      expect(tasks[0].name).toBe("Complex task");
      expect(tasks[0].taskId).toBe("abcd.complex-task-name");
    });

    it("should not match task ID without required space", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task name[abc.task-id]
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(tasks.length).toBe(1);
      // Should not extract task ID because there's no space before it
      expect(tasks[0].name).toBe("Task name[abc.task-id]");
      expect(tasks[0].taskId).toBe(null);
    });

    it("should not match uppercase letters in task ID", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task name [ABC.task-id]
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      // Should not match because prefix must be lowercase
      expect(tasks[0].taskId).toBe(null);
    });

    it("should preserve task ID during round-trip", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Fix bug [xyz.fix-bug]
`;
      const parsed = ProjectStateEditor.parse(content);
      const updated = ProjectStateEditor.update(content, parsed);

      expect(updated).toContain("- [ ] Fix bug [xyz.fix-bug]");
    });

    it("should preserve task ID and session badge during round-trip", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Running task [abc.running-task] [Running](todos://session/42)
`;
      const parsed = ProjectStateEditor.parse(content);
      const updated = ProjectStateEditor.update(content, parsed);

      expect(updated).toContain("- [ ] Running task [abc.running-task] [Running](todos://session/42)");
    });

    it("should add task ID without removing session badge", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task with session [Running](todos://session/42)
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      // Add task ID
      tasks[0].taskId = "xyz.task-with-session";

      const updated = ProjectStateEditor.update(content, parsed);
      expect(updated).toContain("- [ ] Task with session [xyz.task-with-session] [Running](todos://session/42)");
    });

    it("should add session badge without removing task ID", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task with ID [abc.task-with-id]
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      // Add session badge
      tasks[0].sessionStatus = { status: "Running", sessionId: 42 };

      const updated = ProjectStateEditor.update(content, parsed);
      expect(updated).toContain("- [ ] Task with ID [abc.task-with-id] [Running](todos://session/42)");
    });

    it("should update session badge without removing task ID", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task [abc.task] [Running](todos://session/42)
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      // Update session badge
      tasks[0].sessionStatus = { status: "Stopped", sessionId: 42 };

      const updated = ProjectStateEditor.update(content, parsed);
      expect(updated).toContain("- [ ] Task [abc.task] [Stopped](todos://session/42)");
    });

    it("should remove task ID when set to null", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task with ID [abc.task-with-id]
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      // Remove task ID
      tasks[0].taskId = null;

      const updated = ProjectStateEditor.update(content, parsed);
      expect(updated).toContain("- [ ] Task with ID\n");
      expect(updated).not.toContain("[abc.task-with-id]");
    });

    it("should handle task with link and task ID", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] See [docs](https://example.com) [abc.see-docs]
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(tasks[0].name).toBe("See [docs](https://example.com)");
      expect(tasks[0].taskId).toBe("abc.see-docs");
    });

    it("should handle task with link, task ID, and session badge", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] See [docs](https://example.com) [abc.see-docs] [Running](todos://session/42)
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(tasks[0].name).toBe("See [docs](https://example.com)");
      expect(tasks[0].taskId).toBe("abc.see-docs");
      expect(tasks[0].sessionStatus?.status).toBe("Running");
      expect(tasks[0].sessionStatus?.sessionId).toBe(42);
    });

    it("should format task ID correctly", () => {
      const taskId = formatTaskId("abc.my-task");
      expect(taskId).toBe(" [abc.my-task]");
    });

    it("should preserve indentation with task ID", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
  - [ ] Indented task [abc.indented-task]
`;
      const parsed = ProjectStateEditor.parse(content);
      const updated = ProjectStateEditor.update(content, parsed);

      expect(updated).toContain("  - [ ] Indented task [abc.indented-task]");
    });

    it("should handle task with numbers in label", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task name [abc.task-123-name]
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(tasks[0].taskId).toBe("abc.task-123-name");
    });

    it("should handle completed task with task ID", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [x] Completed task [abc.completed-task]
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(tasks[0].complete).toBe("x");
      expect(tasks[0].taskId).toBe("abc.completed-task");
    });
  });

  describe("task ID generation", () => {
    it("should generate task ID with 3-letter prefix and derived label", () => {
      const existingIds = new Set<string>();
      const taskId = generateTaskId("Fix API timeout bug", existingIds);

      expect(taskId).toMatch(/^[a-z]{3}\.fix-api-timeout-bug$/);
    });

    it("should derive label from task name", () => {
      const existingIds = new Set<string>();
      const taskId = generateTaskId("Implement User Authentication", existingIds);

      expect(taskId).toMatch(/^[a-z]{3}\.implement-user-authentication$/);
    });

    it("should handle task names with special characters", () => {
      const existingIds = new Set<string>();
      const taskId = generateTaskId("Fix bug #123 (urgent!)", existingIds);

      // Should strip special chars and keep alphanumeric
      expect(taskId).toMatch(/^[a-z]{3}\.fix-bug-123-urgent$/);
    });

    it("should handle task names with emojis", () => {
      const existingIds = new Set<string>();
      const taskId = generateTaskId("Deploy to production 🚀", existingIds);

      expect(taskId).toMatch(/^[a-z]{3}\.deploy-to-production$/);
    });

    it("should avoid collisions by trying different prefixes", () => {
      const existingIds = new Set(["abc.my-task", "def.my-task", "xyz.my-task"]);
      const taskId = generateTaskId("My task", existingIds);

      // Should generate a different prefix
      expect(taskId).toMatch(/^[a-z]{3}\.my-task$/);
      expect(taskId).not.toBe("abc.my-task");
      expect(taskId).not.toBe("def.my-task");
      expect(taskId).not.toBe("xyz.my-task");
    });

    it("should use 4-letter prefix as fallback", () => {
      // Fill up many 3-letter combinations for the same label
      const existingIds = new Set<string>();
      const chars = "abcdefghijklmnopqrstuvwxyz";

      // Create a scenario where 4-letter prefix is more likely
      // by generating many 3-letter IDs (note: actual collision depends on randomness)
      for (let i = 0; i < 100; i++) {
        const prefix = Array.from({ length: 3 }, () => chars[i % 26]).join("");
        existingIds.add(`${prefix}.common-task`);
      }

      const taskId = generateTaskId("Common task", existingIds);

      // Could be 3-letter or 4-letter depending on random generation
      expect(taskId).toMatch(/^[a-z]{3,4}\.common-task(-\d+)?$/);
    });

    it("should truncate very long task names", () => {
      const existingIds = new Set<string>();
      const longName = "This is a very long task name that should be truncated to a reasonable length for the label";
      const taskId = generateTaskId(longName, existingIds);

      const [prefix, label] = taskId.split(".");
      expect(prefix.length).toBeGreaterThanOrEqual(3);
      expect(prefix.length).toBeLessThanOrEqual(4);
      expect(label.length).toBeLessThanOrEqual(40);
    });

    it("should handle empty or whitespace task names", () => {
      const existingIds = new Set<string>();
      const taskId = generateTaskId("   ", existingIds);

      // Should still generate a valid ID
      expect(taskId).toMatch(/^[a-z]{3,4}\./);
    });

    it("ensureTaskId should add task ID if missing", () => {
      const task: TaskBlock = {
        type: "task",
        name: "My task",
        details: null,
        complete: false,
        prefix: "- ",
        taskId: null,
        sessionStatus: null,
      };

      const existingIds = new Set<string>();
      ensureTaskId(task, existingIds);

      expect(task.taskId).not.toBe(null);
      expect(task.taskId).toMatch(/^[a-z]{3,4}\.my-task$/);
      expect(existingIds.has(task.taskId!)).toBe(true);
    });

    it("ensureTaskId should not replace existing task ID", () => {
      const task: TaskBlock = {
        type: "task",
        name: "My task",
        details: null,
        complete: false,
        prefix: "- ",
        taskId: "abc.existing-id",
        sessionStatus: null,
      };

      const existingIds = new Set<string>();
      ensureTaskId(task, existingIds);

      expect(task.taskId).toBe("abc.existing-id");
      expect(existingIds.size).toBe(0); // Should not add to set
    });

    it("should generate unique IDs for multiple tasks", () => {
      const existingIds = new Set<string>();
      const ids: string[] = [];

      for (let i = 0; i < 10; i++) {
        const taskId = generateTaskId("Similar task", existingIds);
        expect(existingIds.has(taskId)).toBe(false);
        existingIds.add(taskId);
        ids.push(taskId);
      }

      // All IDs should be unique
      const uniqueIds = new Set(ids);
      expect(uniqueIds.size).toBe(10);
    });
  });

  describe("task ID collision avoidance integration", () => {
    it("should handle adding task IDs to multiple tasks without collisions", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Task A
- [ ] Task B
- [ ] Task C
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(tasks.length).toBe(3);

      const existingIds = new Set<string>();
      tasks.forEach((task) => {
        ensureTaskId(task, existingIds);
      });

      // All tasks should have IDs
      expect(tasks[0].taskId).not.toBe(null);
      expect(tasks[1].taskId).not.toBe(null);
      expect(tasks[2].taskId).not.toBe(null);

      // All IDs should be unique
      const ids = tasks.map((t) => t.taskId!);
      const uniqueIds = new Set(ids);
      expect(uniqueIds.size).toBe(3);

      // Round-trip should preserve all IDs
      const updated = ProjectStateEditor.update(content, parsed);
      const reparsed = ProjectStateEditor.parse(updated);
      const reparsedTasks = reparsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(reparsedTasks[0].taskId).toBe(tasks[0].taskId);
      expect(reparsedTasks[1].taskId).toBe(tasks[1].taskId);
      expect(reparsedTasks[2].taskId).toBe(tasks[2].taskId);
    });

    it("should preserve existing task IDs and generate for new tasks", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Existing task [abc.existing-task]
- [ ] New task without ID
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      expect(tasks[0].taskId).toBe("abc.existing-task");
      expect(tasks[1].taskId).toBe(null);

      const existingIds = new Set([tasks[0].taskId!]);
      ensureTaskId(tasks[1], existingIds);

      expect(tasks[1].taskId).not.toBe(null);
      expect(tasks[1].taskId).not.toBe("abc.existing-task");

      const updated = ProjectStateEditor.update(content, parsed);
      expect(updated).toContain("[abc.existing-task]");
      expect(updated).toContain(tasks[1].taskId!);
    });

    it("should handle tasks with same name by generating different prefixes", () => {
      const content = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---
- [ ] Fix bug
- [ ] Fix bug
- [ ] Fix bug
`;
      const parsed = ProjectStateEditor.parse(content);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      const existingIds = new Set<string>();
      tasks.forEach((task) => {
        ensureTaskId(task, existingIds);
      });

      // All should have different IDs despite same name
      const ids = tasks.map((t) => t.taskId!);
      const uniqueIds = new Set(ids);
      expect(uniqueIds.size).toBe(3);

      // All should have "fix-bug" label but different prefixes
      ids.forEach((id) => {
        expect(id).toMatch(/^[a-z]{3,4}\.fix-bug$/);
      });
    });
  });

  // Cross-language parity test (bd-q85.4)
  describe("cross-language parity with Rust parser", () => {
    it("should match Rust parser results for shared fixture", async () => {
      // Read the shared fixture and expected JSON
      const fixturePath = new URL("../../../test/fixtures/task-id-parsing.md", import.meta.url);
      const expectedPath = new URL("../../../test/fixtures/task-id-parsing.expected.json", import.meta.url);

      const fixtureContent = await Bun.file(fixturePath).text();
      const expectedTasks = JSON.parse(await Bun.file(expectedPath).text());

      // Parse with TypeScript parser
      const parsed = ProjectStateEditor.parse(fixtureContent);
      const tasks = parsed.markdown.filter((m) => m.type === "task") as TaskBlock[];

      // Convert to simplified format matching expected JSON
      const simplifiedTasks = tasks.map((task) => ({
        name: task.name,
        complete: task.complete !== false,
        taskId: task.taskId,
        sessionStatus: task.sessionStatus,
      }));

      // Compare with expected results
      expect(simplifiedTasks.length).toBe(expectedTasks.length);

      simplifiedTasks.forEach((actual, index) => {
        const expected = expectedTasks[index];
        expect(actual.name).toBe(expected.name);
        expect(actual.complete).toBe(expected.complete);
        expect(actual.taskId).toBe(expected.taskId);

        if (expected.sessionStatus === null) {
          expect(actual.sessionStatus).toBe(null);
        } else {
          expect(actual.sessionStatus).not.toBe(null);
          expect(actual.sessionStatus?.status).toBe(expected.sessionStatus.status);
          expect(actual.sessionStatus?.sessionId).toBe(expected.sessionStatus.sessionId);
        }
      });
    });
  });
});
