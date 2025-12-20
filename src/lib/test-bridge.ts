// Test bridge API for E2E testing
// Exposes methods to control and query the app state from test runners

import { invoke } from "@tauri-apps/api/core";
import type { AppControllers } from "../main-test";
import type { ProjectMarkdown } from "./ProjectStateEditor";
import { TestClock } from "./clock";
import type { AppEvent } from "./events";
import type { LoadedProjectState, WorkState } from "./project";

export interface TestBridge {
  // State queries
  getState(): LoadedProjectState | undefined;
  getWorkState(): WorkState | undefined;
  getCurrentTask(): (ProjectMarkdown & { type: "task" }) | undefined;
  getTasks(): Array<ProjectMarkdown & { type: "task" }>;
  isCompact(): boolean;

  // State mutations
  openProject(path: string): Promise<void>;
  completeTask(taskName: string): Promise<void>;
  changeState(state: WorkState): Promise<void>;
  adjustTime(ms: number): Promise<void>;

  // Clock control (for TestClock)
  advanceClock(ms: number): void;
  setClockTime(timestamp: number): void;
  getClockTime(): number;

  // Event history (for testing event-driven behavior)
  getEventHistory(): AppEvent[];
  clearEventHistory(): void;

  // Fixture management (via Tauri commands)
  createTempDir(label?: string): Promise<string>;
  loadFixture(name: string, tempDir: string): Promise<string>;
  listFixtures(): Promise<string[]>;
  cleanupAll(): Promise<void>;
  getSocketPath(): Promise<string>;

  // Waiting utilities
  waitForState(predicate: (state: LoadedProjectState) => boolean, timeout?: number): Promise<void>;
  waitForWorkState(state: WorkState, timeout?: number): Promise<void>;
  waitForTaskCount(count: number, timeout?: number): Promise<void>;

  // Direct access to controllers (for advanced testing)
  controllers: AppControllers;
}

export function initializeTestBridge(controllers: AppControllers): TestBridge {
  const { projectManager, appWindows } = controllers;

  // Subscribe to state changes for waiting utilities
  let currentState: LoadedProjectState | undefined;
  const stateListeners: Array<(state: LoadedProjectState | undefined) => void> = [];

  projectManager.subscribe((state) => {
    currentState = state;
    stateListeners.forEach((listener) => listener(state));
  });

  const bridge: TestBridge = {
    // State queries
    getState() {
      return currentState;
    },

    getWorkState() {
      return currentState?.workState;
    },

    getCurrentTask() {
      return currentState?.projectFile.markdown.find(
        (m): m is ProjectMarkdown & { type: "task" } => m.type === "task" && !m.complete,
      );
    },

    getTasks() {
      return (
        currentState?.projectFile.markdown.filter((m): m is ProjectMarkdown & { type: "task" } => m.type === "task") ??
        []
      );
    },

    isCompact() {
      // Read from the atom directly
      return false; // TODO: need to read from jotai store
    },

    // State mutations
    async openProject(path: string) {
      // Directly load project instead of using openProject (which shows a dialog)
      await projectManager.loadProject(path, "absolute");
    },

    async completeTask(taskName: string) {
      await projectManager.updateProject((draft) => {
        const task = draft.markdown.find((m) => m.type === "task" && m.name === taskName);
        if (task && task.type === "task") {
          task.complete = "x";
        }
      });
    },

    async changeState(state: WorkState) {
      await projectManager.updateWorkState(state);
    },

    async adjustTime(ms: number) {
      if (!currentState) return;
      const { clock } = controllers;
      const currentEndsAt = currentState.stateTransitions.endsAt ?? clock.now();
      await projectManager.updateStateTransitions({
        startedAt: currentState.stateTransitions.startedAt,
        endsAt: currentEndsAt + ms,
      });
    },

    // Clock control (for TestClock)
    advanceClock(ms: number) {
      const { clock } = controllers;
      if (clock instanceof TestClock) {
        clock.advance(ms);
      } else {
        console.warn("[TestBridge] advanceClock called but clock is not a TestClock");
      }
    },

    setClockTime(timestamp: number) {
      const { clock } = controllers;
      if (clock instanceof TestClock) {
        clock.setTime(timestamp);
      } else {
        console.warn("[TestBridge] setClockTime called but clock is not a TestClock");
      }
    },

    getClockTime() {
      return controllers.clock.now();
    },

    // Event history (for testing event-driven behavior)
    getEventHistory() {
      return controllers.eventBus.getHistory();
    },

    clearEventHistory() {
      controllers.eventBus.clearHistory();
    },

    // Fixture management
    async createTempDir(label?: string) {
      return await invoke<string>("test_create_temp_dir", { label });
    },

    async loadFixture(name: string, tempDir: string) {
      return await invoke<string>("test_load_fixture", { name, tempDir });
    },

    async listFixtures() {
      return await invoke<string[]>("test_list_fixtures");
    },

    async cleanupAll() {
      await invoke("test_cleanup_all");
    },

    async getSocketPath() {
      return await invoke<string>("test_get_socket_path");
    },

    // Waiting utilities
    async waitForState(predicate, timeout = 5000) {
      return new Promise((resolve, reject) => {
        // Check immediately
        if (currentState && predicate(currentState)) {
          resolve();
          return;
        }

        const timeoutId = setTimeout(() => {
          const idx = stateListeners.indexOf(listener);
          if (idx !== -1) stateListeners.splice(idx, 1);
          reject(new Error(`Timeout waiting for state condition after ${timeout}ms`));
        }, timeout);

        const listener = (state: LoadedProjectState | undefined) => {
          if (state && predicate(state)) {
            clearTimeout(timeoutId);
            const idx = stateListeners.indexOf(listener);
            if (idx !== -1) stateListeners.splice(idx, 1);
            resolve();
          }
        };

        stateListeners.push(listener);
      });
    },

    async waitForWorkState(state, timeout = 5000) {
      return bridge.waitForState((s) => s.workState === state, timeout);
    },

    async waitForTaskCount(count, timeout = 5000) {
      return bridge.waitForState(
        (s) => s.projectFile.markdown.filter((m) => m.type === "task").length === count,
        timeout,
      );
    },

    // Direct access
    controllers,
  };

  return bridge;
}
