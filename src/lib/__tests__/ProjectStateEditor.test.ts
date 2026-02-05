import { describe, expect, it } from "bun:test";
import { ProjectStateEditor, type TaskBlock, formatSessionBadge } from "../ProjectStateEditor";

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
});
