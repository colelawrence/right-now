import { describe, expect, it } from "bun:test";
import { ProjectStateEditor } from "../ProjectStateEditor";

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
    expect(parsed.markdown).toMatchInlineSnapshot(`
      [
        {
          "level": 1,
          "text": "Just a heading",
          "type": "heading",
        },
        {
          "complete": false,
          "details": "Task related text here",
          "name": "Some Task",
          "prefix": "- ",
          "type": "task",
        },
      ]
    `);
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
    const tasks = parsed.markdown.filter((m) => m.type === "task");
    expect(tasks).toMatchInlineSnapshot(`
      [
        {
          "complete": false,
          "details": "",
          "name": "Task 1",
          "prefix": "- ",
          "type": "task",
        },
        {
          "complete": false,
          "details": null,
          "name": "Task 2",
          "prefix": "- ",
          "type": "task",
        },
        {
          "complete": false,
          "details": "  Has detail",
          "name": "Task 3",
          "prefix": "- ",
          "type": "task",
        },
      ]
    `);
  });

  it("should not break if frontmatter is invalid or partially corrupted", () => {
    const corrupted = `---
pomodoro_settings:
  work_duration: 25
  break_duration: invalid
---
- [ ] Task
`;

    // We expect parse to fallback to defaults gracefully
    const parsed = ProjectStateEditor.parse(corrupted);
    expect(parsed.pomodoroSettings.workDuration).toBe(25); // from partial parse
    expect(parsed.pomodoroSettings.breakDuration).toBe(5); // fallback
    // Body should still parse
    expect(parsed.markdown[0].type).toBe("task");
  });
});
