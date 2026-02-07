/**
 * Unit tests for CrController: cancellation semantics + key flows.
 */

import { beforeEach, describe, expect, mock, test } from "bun:test";
import type { SessionClient } from "../SessionClient";
import type { CrClient } from "../context-resurrection/client";
import { CrController, type CrControllerDeps, type CrControllerState } from "../context-resurrection/controller";
import type { ContextSnapshotV1, CrResult } from "../context-resurrection/types";

// Mock clients
function createMockCrClient(): CrClient {
  return {
    latest: mock(async () => ({ ok: true, value: null }) as CrResult<ContextSnapshotV1 | null>),
    list: mock(async () => ({ ok: true, value: [] }) as CrResult<ContextSnapshotV1[]>),
    get: mock(async () => ({ ok: true, value: null }) as CrResult<ContextSnapshotV1 | null>),
    captureNow: mock(async () => ({ ok: true, value: null }) as CrResult<ContextSnapshotV1 | null>),
    deleteTask: mock(async () => ({ ok: true, value: 0 }) as CrResult<number>),
    deleteProject: mock(async () => ({ ok: true, value: 0 }) as CrResult<number>),
  } as unknown as CrClient;
}

function createMockSessionClient(): SessionClient {
  return {
    startSession: mock(async () => ({
      id: 1,
      task_key: "test",
      project_path: "/test",
      status: "Running" as const,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    })),
    continueSession: mock(async () => ({
      session: {
        id: 1,
        task_key: "test",
        project_path: "/test",
        status: "Running" as const,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      },
      tail: [],
    })),
    stopSession: mock(async () => ({
      id: 1,
      task_key: "test",
      project_path: "/test",
      status: "Stopped" as const,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    })),
    listSessions: mock(async () => []),
  } as unknown as SessionClient;
}

function createMockSnapshot(overrides?: Partial<ContextSnapshotV1>): ContextSnapshotV1 {
  // Create a snapshot that's old enough to pass the eligibility check (> 60 minutes ago)
  const twoHoursAgo = new Date(Date.now() - 2 * 60 * 60 * 1000);
  return {
    id: "snapshot-1",
    version: 1,
    project_path: "/test/project",
    task_id: "task-1",
    task_title_at_capture: "Test Task",
    captured_at: twoHoursAgo.toISOString(),
    capture_reason: "manual",
    ...overrides,
  };
}

describe("CrController", () => {
  let controller: CrController;
  let crClient: CrClient;
  let sessionClient: SessionClient;
  let stateChanges: CrControllerState[];
  let onActiveTaskChange: ReturnType<typeof mock>;

  beforeEach(() => {
    crClient = createMockCrClient();
    sessionClient = createMockSessionClient();
    stateChanges = [];
    onActiveTaskChange = mock(async () => {});

    const deps: CrControllerDeps = {
      crClient,
      sessionClient,
      onStateChange: (state) => stateChanges.push(state),
      onActiveTaskChange,
    };

    controller = new CrController(deps);
  });

  describe("resetForProject", () => {
    test("resets all state to initial values", () => {
      controller.resetForProject();

      const state = controller.getState();
      expect(state.daemonUnavailable).toBe(false);
      expect(state.taskHasContext).toEqual({});
      expect(state.cardSnapshot).toBe(null);
      expect(state.cardPinned).toBe(false);
      expect(state.dismissedSnapshotId).toBe(null);
      expect(stateChanges.length).toBe(1);
    });

    test("cancels in-flight load requests", async () => {
      // Start a load that will take some time
      let resolveLoad: (value: unknown) => void;
      const loadPromise = new Promise((resolve) => {
        resolveLoad = resolve;
      });

      crClient.latest = mock(async () => {
        await loadPromise;
        return { ok: true, value: createMockSnapshot() };
      }) as typeof crClient.latest;

      // Start load (don't await)
      const loadCall = controller.load({
        projectPath: "/test",
        activeTaskId: "task-1",
        tasks: [{ taskId: "task-1" }],
      });

      // Reset before load completes
      controller.resetForProject();

      // Complete the load
      resolveLoad!(undefined);
      await loadCall;

      // State should not include the load result (request was cancelled)
      const state = controller.getState();
      expect(state.cardSnapshot).toBe(null);
    });
  });

  describe("load", () => {
    test("loads resurrection state successfully", async () => {
      const snapshot = createMockSnapshot();
      crClient.latest = mock(async () => ({
        ok: true,
        value: snapshot,
      })) as typeof crClient.latest;

      await controller.load({
        projectPath: "/test",
        activeTaskId: "task-1",
        tasks: [{ taskId: "task-1" }],
      });

      const state = controller.getState();
      expect(state.daemonUnavailable).toBe(false);
      expect(state.taskHasContext).toEqual({ "task-1": true });
      expect(state.cardSnapshot).toEqual(snapshot);
    });

    test("handles daemon unavailable", async () => {
      crClient.latest = mock(async () => ({
        ok: false,
        error: { type: "daemon_unavailable", message: "Daemon not running" },
      })) as typeof crClient.latest;

      await controller.load({
        projectPath: "/test",
        activeTaskId: "task-1",
        tasks: [{ taskId: "task-1" }],
      });

      const state = controller.getState();
      expect(state.daemonUnavailable).toBe(true);
    });

    test("does not update snapshot when card is pinned", async () => {
      const initialSnapshot = createMockSnapshot({ id: "initial" });
      const newSnapshot = createMockSnapshot({ id: "new" });

      // Set initial state with pinned card
      crClient.latest = mock(async () => ({
        ok: true,
        value: initialSnapshot,
      })) as typeof crClient.latest;

      await controller.load({
        projectPath: "/test",
        activeTaskId: "task-1",
        tasks: [{ taskId: "task-1" }],
      });

      // Pin the card
      controller.dismissCard();
      await controller.openForTask("/test", "task-1");

      // Load with new snapshot
      crClient.latest = mock(async () => ({
        ok: true,
        value: newSnapshot,
      })) as typeof crClient.latest;

      await controller.load({
        projectPath: "/test",
        activeTaskId: "task-1",
        tasks: [{ taskId: "task-1" }],
      });

      const state = controller.getState();
      // Should still show initial snapshot (card is pinned)
      expect(state.cardSnapshot?.id).toBe("initial");
    });

    test("respects dismissed snapshot ID", async () => {
      const dismissedSnapshot = createMockSnapshot({ id: "dismissed" });

      crClient.latest = mock(async () => ({
        ok: true,
        value: dismissedSnapshot,
      })) as typeof crClient.latest;

      // Load once
      await controller.load({
        projectPath: "/test",
        activeTaskId: "task-1",
        tasks: [{ taskId: "task-1" }],
      });

      // Dismiss the card
      controller.dismissCard();

      // Load again (same snapshot)
      await controller.load({
        projectPath: "/test",
        activeTaskId: "task-1",
        tasks: [{ taskId: "task-1" }],
      });

      const state = controller.getState();
      // Should not show dismissed snapshot
      expect(state.cardSnapshot).toBe(null);
      expect(state.dismissedSnapshotId).toBe("dismissed");
    });

    test("cancels previous load for same task (last-call-wins)", async () => {
      let resolveFirst: (value: unknown) => void;
      let resolveSecond: (value: unknown) => void;

      const firstPromise = new Promise((resolve) => {
        resolveFirst = resolve;
      });
      const secondPromise = new Promise((resolve) => {
        resolveSecond = resolve;
      });

      const firstSnapshot = createMockSnapshot({ id: "first" });
      const secondSnapshot = createMockSnapshot({ id: "second" });

      let callCount = 0;
      crClient.latest = mock(async () => {
        callCount++;
        if (callCount === 1) {
          await firstPromise;
          return { ok: true, value: firstSnapshot };
        }
        await secondPromise;
        return { ok: true, value: secondSnapshot };
      }) as typeof crClient.latest;

      // Start first load (don't await)
      const firstLoad = controller.load({
        projectPath: "/test",
        activeTaskId: "task-1",
        tasks: [{ taskId: "task-1" }],
      });

      // Start second load (same task)
      const secondLoad = controller.load({
        projectPath: "/test",
        activeTaskId: "task-1",
        tasks: [{ taskId: "task-1" }],
      });

      // Complete second first
      resolveSecond!(undefined);
      await secondLoad;

      // Now complete first
      resolveFirst!(undefined);
      await firstLoad;

      const state = controller.getState();
      // Should only have the second result
      expect(state.cardSnapshot?.id).toBe("second");
    });
  });

  describe("openForTask", () => {
    test("pins card and loads snapshot", async () => {
      const snapshot = createMockSnapshot();
      crClient.latest = mock(async () => ({
        ok: true,
        value: snapshot,
      })) as typeof crClient.latest;

      await controller.openForTask("/test", "task-1");

      const state = controller.getState();
      expect(state.cardPinned).toBe(true);
      expect(state.cardSnapshot).toEqual(snapshot);
      expect(state.dismissedSnapshotId).toBe(null);
      expect(onActiveTaskChange).toHaveBeenCalledWith("task-1");
    });

    test("throws when daemon unavailable", async () => {
      crClient.latest = mock(async () => ({
        ok: false,
        error: { type: "daemon_unavailable", message: "Daemon not running" },
      })) as typeof crClient.latest;

      await expect(controller.openForTask("/test", "task-1")).rejects.toThrow("Context Resurrection is unavailable");

      const state = controller.getState();
      expect(state.daemonUnavailable).toBe(true);
    });

    test("throws when no snapshots found", async () => {
      crClient.latest = mock(async () => ({
        ok: true,
        value: null,
      })) as typeof crClient.latest;

      await expect(controller.openForTask("/test", "task-1")).rejects.toThrow("No snapshots found");

      const state = controller.getState();
      expect(state.cardSnapshot).toBe(null);
    });
  });

  describe("resume", () => {
    test("continues existing running session", async () => {
      const snapshot = createMockSnapshot({
        terminal: {
          session_id: 123,
          status: "Running",
        },
      });

      await controller.resume("/test", snapshot);

      expect(sessionClient.continueSession).toHaveBeenCalledWith(123, 512);
      expect(onActiveTaskChange).toHaveBeenCalledWith("task-1");

      const state = controller.getState();
      expect(state.cardSnapshot).toBe(null);
      expect(state.cardPinned).toBe(false);
    });

    test("starts new session when terminal is stopped", async () => {
      const snapshot = createMockSnapshot({
        terminal: {
          session_id: 123,
          status: "Stopped",
        },
      });

      await controller.resume("/test", snapshot);

      expect(sessionClient.startSession).toHaveBeenCalledWith("task-1", "/test", "task-1");
      expect(onActiveTaskChange).toHaveBeenCalledWith("task-1");

      const state = controller.getState();
      expect(state.cardSnapshot).toBe(null);
      expect(state.cardPinned).toBe(false);
    });

    test("starts new session when no terminal context", async () => {
      const snapshot = createMockSnapshot();

      await controller.resume("/test", snapshot);

      expect(sessionClient.startSession).toHaveBeenCalledWith("task-1", "/test", "task-1");
    });
  });

  describe("saveNote", () => {
    test("saves note and updates state", async () => {
      const initialSnapshot = createMockSnapshot();
      const savedSnapshot = createMockSnapshot({ id: "saved", user_note: "Test note" });

      // Set up initial state
      crClient.latest = mock(async () => ({
        ok: true,
        value: initialSnapshot,
      })) as typeof crClient.latest;

      await controller.load({
        projectPath: "/test",
        activeTaskId: "task-1",
        tasks: [{ taskId: "task-1" }],
      });

      // Mock captureNow
      crClient.captureNow = mock(async () => ({
        ok: true,
        value: savedSnapshot,
      })) as typeof crClient.captureNow;

      await controller.saveNote("/test", "Test note");

      const state = controller.getState();
      expect(state.cardSnapshot).toEqual(savedSnapshot);
      expect(state.cardPinned).toBe(true);
      expect(state.dismissedSnapshotId).toBe(null);
      expect(state.taskHasContext["task-1"]).toBe(true);
    });

    test("throws when no current snapshot", async () => {
      await expect(controller.saveNote("/test", "Test note")).rejects.toThrow("No snapshot to save note for");
    });

    test("handles daemon unavailable", async () => {
      const snapshot = createMockSnapshot();
      crClient.latest = mock(async () => ({
        ok: true,
        value: snapshot,
      })) as typeof crClient.latest;

      await controller.load({
        projectPath: "/test",
        activeTaskId: "task-1",
        tasks: [{ taskId: "task-1" }],
      });

      crClient.captureNow = mock(async () => ({
        ok: false,
        error: { type: "daemon_unavailable", message: "Daemon not running" },
      })) as typeof crClient.captureNow;

      await expect(controller.saveNote("/test", "Test note")).rejects.toThrow();

      const state = controller.getState();
      expect(state.daemonUnavailable).toBe(true);
    });
  });

  describe("dismissCard", () => {
    test("clears snapshot and remembers dismissed ID", async () => {
      const snapshot = createMockSnapshot({ id: "test-id" });

      crClient.latest = mock(async () => ({
        ok: true,
        value: snapshot,
      })) as typeof crClient.latest;

      await controller.load({
        projectPath: "/test",
        activeTaskId: "task-1",
        tasks: [{ taskId: "task-1" }],
      });

      controller.dismissCard();

      const state = controller.getState();
      expect(state.cardSnapshot).toBe(null);
      expect(state.cardPinned).toBe(false);
      expect(state.dismissedSnapshotId).toBe("test-id");
    });
  });

  describe("forgetTask", () => {
    test("deletes task context after confirmation", async () => {
      const snapshot = createMockSnapshot();

      crClient.latest = mock(async () => ({
        ok: true,
        value: snapshot,
      })) as typeof crClient.latest;

      await controller.load({
        projectPath: "/test",
        activeTaskId: "task-1",
        tasks: [{ taskId: "task-1" }],
      });

      crClient.deleteTask = mock(async () => ({
        ok: true,
        value: 5,
      })) as typeof crClient.deleteTask;

      // Mock confirm
      global.confirm = mock(() => true);

      const deletedCount = await controller.forgetTask("/test");

      expect(deletedCount).toBe(5);
      expect(crClient.deleteTask).toHaveBeenCalledWith("/test", "task-1");

      const state = controller.getState();
      expect(state.cardSnapshot).toBe(null);
      expect(state.taskHasContext["task-1"]).toBe(false);
    });

    test("returns 0 when user cancels", async () => {
      const snapshot = createMockSnapshot();

      crClient.latest = mock(async () => ({
        ok: true,
        value: snapshot,
      })) as typeof crClient.latest;

      await controller.load({
        projectPath: "/test",
        activeTaskId: "task-1",
        tasks: [{ taskId: "task-1" }],
      });

      global.confirm = mock(() => false);

      const deletedCount = await controller.forgetTask("/test");

      expect(deletedCount).toBe(0);
      expect(crClient.deleteTask).not.toHaveBeenCalled();
    });

    test("throws when no current snapshot", async () => {
      global.confirm = mock(() => true);
      await expect(controller.forgetTask("/test")).rejects.toThrow("No snapshot to forget");
    });
  });

  describe("forgetProject", () => {
    test("deletes project context after confirmation", async () => {
      crClient.deleteProject = mock(async () => ({
        ok: true,
        value: 10,
      })) as typeof crClient.deleteProject;

      global.confirm = mock(() => true);

      const deletedCount = await controller.forgetProject("/test");

      expect(deletedCount).toBe(10);
      expect(crClient.deleteProject).toHaveBeenCalledWith("/test");

      const state = controller.getState();
      expect(state.taskHasContext).toEqual({});
      expect(state.cardSnapshot).toBe(null);
    });

    test("returns 0 when user cancels", async () => {
      global.confirm = mock(() => false);

      const deletedCount = await controller.forgetProject("/test");

      expect(deletedCount).toBe(0);
      expect(crClient.deleteProject).not.toHaveBeenCalled();
    });
  });
});
