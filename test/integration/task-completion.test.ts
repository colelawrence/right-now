// Integration test: Task completion flow
// This is the proof-of-concept E2E test that exercises the full stack:
// Test Runner → Unix Socket → Rust → Tauri Events → Frontend → Test Bridge → File I/O

import { afterAll, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import {
  cleanupTestTempDir,
  createTestTempDir,
  getRunner,
  loadTestFixture,
  setupTestHarness,
  teardownTestHarness,
} from "../harness/setup";

// Type for the app state we get back
interface ProjectState {
  fullPath: string;
  workState: "planning" | "working" | "break";
  projectFile: {
    pomodoroSettings: {
      workDuration: number;
      breakDuration: number;
    };
    markdown: Array<{
      type: "task" | "heading" | "unrecognized";
      name?: string;
      complete?: string | false;
      text?: string;
      level?: number;
    }>;
  };
  stateTransitions: {
    startedAt: number;
    endsAt?: number;
  };
}

describe("Task Completion Flow", () => {
  // Increase timeout for slow harness startup (WebView init takes time)
  beforeAll(async () => {
    await setupTestHarness();
  }, 120000); // 2 minute timeout for harness startup

  afterAll(async () => {
    await teardownTestHarness();
  }, 30000);

  beforeEach(async () => {
    await cleanupTestTempDir();
  }, 30000);

  it("completing a task persists to markdown file", async () => {
    const runner = getRunner();

    // 1. Setup: Create temp dir and load fixture
    const tempDir = await createTestTempDir();
    const projectPath = await loadTestFixture("minimal");
    console.log("[Test] Loaded fixture to:", projectPath);

    // 2. Open project in the app
    const openResponse = await runner.invoke({ type: "open_project", path: projectPath });
    console.log("[Test] Open project response:", JSON.stringify(openResponse));

    // Debug: Log the state immediately after opening
    const debugState = await runner.getState();
    console.log("[Test] State after openProject:", JSON.stringify(debugState, null, 2)?.slice(0, 500));

    // 3. Wait for project to load
    await runner.waitForProject(10000);
    console.log("[Test] Project loaded");

    // 4. Verify initial state - task is incomplete
    const before = (await runner.getState()) as ProjectState;
    const task = before.projectFile.markdown.find((b) => b.type === "task" && b.name === "First task");
    console.log("[Test] Initial task state:", task);

    expect(task).toBeDefined();
    expect(task?.complete).toBe(false);

    // 5. Complete the task via test bridge
    await runner.completeTask("First task");
    console.log("[Test] Completed task");

    // 6. Wait a moment for file write to complete
    await new Promise((resolve) => setTimeout(resolve, 500));

    // 7. Verify in-memory state updated
    const after = (await runner.getState()) as ProjectState;
    const updatedTask = after.projectFile.markdown.find((b) => b.type === "task" && b.name === "First task");
    console.log("[Test] Updated task state:", updatedTask);

    expect(updatedTask).toBeDefined();
    expect(updatedTask?.complete).toBe("x");

    // 8. THE KEY ASSERTION: Verify file on disk changed
    const fileContent = await Bun.file(projectPath).text();
    console.log("[Test] File content:\n", fileContent);

    expect(fileContent).toContain("- [x] First task");
    expect(fileContent).not.toContain("- [ ] First task");

    // 9. Cleanup
    await runner.cleanupTempDir(tempDir);
    console.log("[Test] Cleaned up");
  }, 60000);

  it("completing multiple tasks preserves file structure", async () => {
    const runner = getRunner();

    // Setup
    const tempDir = await createTestTempDir();
    const projectPath = await loadTestFixture("minimal");
    await runner.openProject(projectPath);
    await runner.waitForProject(10000);

    // Complete both tasks
    await runner.completeTask("First task");
    await new Promise((resolve) => setTimeout(resolve, 200));
    await runner.completeTask("Second task");
    await new Promise((resolve) => setTimeout(resolve, 500));

    // Verify file has both tasks completed
    const fileContent = await Bun.file(projectPath).text();

    expect(fileContent).toContain("- [x] First task");
    expect(fileContent).toContain("- [x] Second task");

    // Verify frontmatter is preserved
    expect(fileContent).toContain("pomodoro_settings:");
    expect(fileContent).toContain("work_duration:");

    // Cleanup
    await runner.cleanupTempDir(tempDir);
  }, 60000);

  it("state transitions work correctly", async () => {
    const runner = getRunner();

    // Setup
    const tempDir = await createTestTempDir();
    const projectPath = await loadTestFixture("minimal");
    await runner.openProject(projectPath);
    await runner.waitForProject(10000);

    // Initial state should be planning
    const initial = (await runner.getState()) as ProjectState;
    expect(initial.workState).toBe("planning");

    // Start working
    await runner.changeState("working");
    await new Promise((resolve) => setTimeout(resolve, 200));

    const working = (await runner.getState()) as ProjectState;
    expect(working.workState).toBe("working");
    expect(working.stateTransitions.endsAt).toBeDefined();

    // Take a break
    await runner.changeState("break");
    await new Promise((resolve) => setTimeout(resolve, 200));

    const onBreak = (await runner.getState()) as ProjectState;
    expect(onBreak.workState).toBe("break");

    // Back to planning
    await runner.changeState("planning");
    await new Promise((resolve) => setTimeout(resolve, 200));

    const planning = (await runner.getState()) as ProjectState;
    expect(planning.workState).toBe("planning");
    expect(planning.stateTransitions.endsAt).toBeUndefined();

    // Cleanup
    await runner.cleanupTempDir(tempDir);
  }, 60000);
});
