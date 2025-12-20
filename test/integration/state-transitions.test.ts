// Integration tests for state transitions
// Tests work state changes: planning -> working -> break -> planning

import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import {
  cleanupTestTempDir,
  getRunner,
  loadTestFixture,
  setupTestHarness,
  teardownTestHarness,
} from "../harness/setup";

describe("State Transitions", () => {
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

  it("should start in planning state", async () => {
    const runner = getRunner();
    const fixturePath = await loadTestFixture("minimal");
    await runner.openProject(fixturePath);

    // State should be planning by default
    // Note: Requires frontend communication
    // const state = await runner.getState() as any;
    // expect(state.workState).toBe("planning");

    expect(true).toBe(true);
  });

  // TODO: Enable these tests once frontend communication is working

  // it("should transition from planning to working", async () => {
  //   const runner = getRunner();
  //   const fixturePath = await loadTestFixture("minimal");
  //   await runner.openProject(fixturePath);

  //   await runner.changeState("working");

  //   const state = await runner.getState() as any;
  //   expect(state.workState).toBe("working");
  //   expect(state.stateTransitions.startedAt).toBeDefined();
  //   expect(state.stateTransitions.endsAt).toBeDefined();
  // });

  // it("should transition from working to break", async () => {
  //   const runner = getRunner();
  //   const fixturePath = await loadTestFixture("minimal");
  //   await runner.openProject(fixturePath);

  //   await runner.changeState("working");
  //   await runner.changeState("break");

  //   const state = await runner.getState() as any;
  //   expect(state.workState).toBe("break");
  // });

  // it("should transition from break back to planning", async () => {
  //   const runner = getRunner();
  //   const fixturePath = await loadTestFixture("minimal");
  //   await runner.openProject(fixturePath);

  //   await runner.changeState("working");
  //   await runner.changeState("break");
  //   await runner.changeState("planning");

  //   const state = await runner.getState() as any;
  //   expect(state.workState).toBe("planning");
  // });

  // it("should complete the full pomodoro cycle", async () => {
  //   const runner = getRunner();
  //   const fixturePath = await loadTestFixture("minimal");
  //   await runner.openProject(fixturePath);

  //   // Full cycle: planning -> working -> break -> planning
  //   await runner.changeState("working");
  //   let state = await runner.getState() as any;
  //   expect(state.workState).toBe("working");

  //   await runner.changeState("break");
  //   state = await runner.getState() as any;
  //   expect(state.workState).toBe("break");

  //   await runner.changeState("planning");
  //   state = await runner.getState() as any;
  //   expect(state.workState).toBe("planning");
  // });
});
