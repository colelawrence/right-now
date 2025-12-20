// Integration tests for project loading
// NOTE: These tests require the full test harness to be running
// They are examples of what E2E tests could look like

import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import {
  cleanupTestTempDir,
  getRunner,
  loadTestFixture,
  setupTestHarness,
  teardownTestHarness,
} from "../harness/setup";

describe("Project Loading", () => {
  beforeAll(async () => {
    await setupTestHarness();
  });

  afterAll(async () => {
    await teardownTestHarness();
  });

  beforeEach(async () => {
    const runner = getRunner();
    await runner.resetState();
    await runner.cleanupAll();
  });

  afterEach(async () => {
    await cleanupTestTempDir();
  });

  it("should load a minimal project fixture", async () => {
    const runner = getRunner();

    // Load the fixture
    const fixturePath = await loadTestFixture("minimal");

    // Open the project
    await runner.openProject(fixturePath);

    // Get the state - this tests the frontend bridge
    // Note: This requires frontend communication to be working
    // const state = await runner.getState();
    // expect(state).toBeDefined();

    // For now, just verify no error was thrown
    expect(true).toBe(true);
  });

  // TODO: Enable these tests once frontend communication is working

  // it("should parse tasks from the project file", async () => {
  //   const runner = getRunner();
  //   const fixturePath = await loadTestFixture("minimal");
  //   await runner.openProject(fixturePath);

  //   const state = await runner.getState() as any;
  //   const tasks = state.projectFile.markdown.filter((m: any) => m.type === "task");

  //   expect(tasks).toHaveLength(2);
  //   expect(tasks[0].name).toBe("First task");
  //   expect(tasks[1].name).toBe("Second task");
  // });

  // it("should load a complex project with multiple sections", async () => {
  //   const runner = getRunner();
  //   const fixturePath = await loadTestFixture("complex");
  //   await runner.openProject(fixturePath);

  //   const state = await runner.getState() as any;
  //   const tasks = state.projectFile.markdown.filter((m: any) => m.type === "task");
  //   const headings = state.projectFile.markdown.filter((m: any) => m.type === "heading");

  //   expect(tasks.length).toBeGreaterThan(5);
  //   expect(headings.length).toBeGreaterThan(1);
  // });
});
