// Integration test: Tracker mode task progression
// Tests that when in working (tracker) mode, completing a task shows the next task

import { afterAll, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import {
  cleanupTestTempDir,
  createTestTempDir,
  getRunner,
  loadTestFixture,
  setupTestHarness,
  teardownTestHarness,
} from "../harness/setup";

// Type for the app state
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

// Helper to get the first incomplete task
function getCurrentTask(state: ProjectState) {
  return state.projectFile.markdown.find((m) => m.type === "task" && !m.complete);
}

// Helper to get all tasks
function getTasks(state: ProjectState) {
  return state.projectFile.markdown.filter((m) => m.type === "task");
}

describe("Tracker Mode Task Progression", () => {
  beforeAll(async () => {
    await setupTestHarness();
  }, 120000);

  afterAll(async () => {
    await teardownTestHarness();
  }, 30000);

  beforeEach(async () => {
    await cleanupTestTempDir();
  }, 30000);

  it("entering working mode shows the first incomplete task as current", async () => {
    const runner = getRunner();

    // Setup
    const tempDir = await createTestTempDir();
    const projectPath = await loadTestFixture("minimal");
    await runner.openProject(projectPath);
    await runner.waitForProject(10000);

    // Initial state should be planning
    const before = (await runner.getState()) as ProjectState;
    expect(before.workState).toBe("planning");

    // Get initial current task
    const initialTask = getCurrentTask(before);
    expect(initialTask?.name).toBe("First task");

    // Switch to working (tracker) mode
    await runner.changeState("working");
    await new Promise((resolve) => setTimeout(resolve, 200));

    // Verify we're in working mode
    const working = (await runner.getState()) as ProjectState;
    expect(working.workState).toBe("working");

    // Current task should still be the first incomplete task
    const currentInWorking = getCurrentTask(working);
    expect(currentInWorking?.name).toBe("First task");

    // Cleanup
    await runner.cleanupTempDir(tempDir);
  }, 60000);

  it("completing a task in working mode advances to the next task", async () => {
    const runner = getRunner();

    // Setup
    const tempDir = await createTestTempDir();
    const projectPath = await loadTestFixture("minimal");
    await runner.openProject(projectPath);
    await runner.waitForProject(10000);

    // Switch to working mode
    await runner.changeState("working");
    await new Promise((resolve) => setTimeout(resolve, 200));

    // Verify initial current task
    const before = (await runner.getState()) as ProjectState;
    expect(before.workState).toBe("working");
    const firstTask = getCurrentTask(before);
    expect(firstTask?.name).toBe("First task");
    console.log("[Test] Current task before completion:", firstTask?.name);

    // Complete the first task
    await runner.completeTask("First task");
    await new Promise((resolve) => setTimeout(resolve, 300));

    // Verify the next task is now current
    const after = (await runner.getState()) as ProjectState;
    const nextTask = getCurrentTask(after);
    console.log("[Test] Current task after completion:", nextTask?.name);

    expect(nextTask).toBeDefined();
    expect(nextTask?.name).toBe("Second task");

    // Verify the first task is now complete
    const tasks = getTasks(after);
    const completedFirst = tasks.find((t) => t.name === "First task");
    expect(completedFirst?.complete).toBe("x");

    // Cleanup
    await runner.cleanupTempDir(tempDir);
  }, 60000);

  it("completing all tasks in working mode leaves no current task", async () => {
    const runner = getRunner();

    // Setup
    const tempDir = await createTestTempDir();
    const projectPath = await loadTestFixture("minimal");
    await runner.openProject(projectPath);
    await runner.waitForProject(10000);

    // Switch to working mode
    await runner.changeState("working");
    await new Promise((resolve) => setTimeout(resolve, 200));

    // Complete all tasks
    await runner.completeTask("First task");
    await new Promise((resolve) => setTimeout(resolve, 200));
    await runner.completeTask("Second task");
    await new Promise((resolve) => setTimeout(resolve, 300));

    // Verify no incomplete tasks remain
    const after = (await runner.getState()) as ProjectState;
    const currentTask = getCurrentTask(after);
    console.log("[Test] Current task after completing all:", currentTask);

    expect(currentTask).toBeUndefined();

    // Verify all tasks are complete
    const tasks = getTasks(after);
    expect(tasks.every((t) => t.complete === "x")).toBe(true);
    console.log("[Test] All tasks completed");

    // Cleanup
    await runner.cleanupTempDir(tempDir);
  }, 60000);

  it("task completion persists to file while in working mode", async () => {
    const runner = getRunner();

    // Setup
    const tempDir = await createTestTempDir();
    const projectPath = await loadTestFixture("minimal");
    await runner.openProject(projectPath);
    await runner.waitForProject(10000);

    // Switch to working mode
    await runner.changeState("working");
    await new Promise((resolve) => setTimeout(resolve, 200));

    // Complete first task
    await runner.completeTask("First task");
    await new Promise((resolve) => setTimeout(resolve, 500));

    // THE KEY ASSERTION: File on disk should reflect the change
    const fileContent = await Bun.file(projectPath).text();
    console.log("[Test] File content after completion in working mode:\n", fileContent);

    expect(fileContent).toContain("- [x] First task");
    expect(fileContent).toContain("- [ ] Second task");

    // Cleanup
    await runner.cleanupTempDir(tempDir);
  }, 60000);
});
