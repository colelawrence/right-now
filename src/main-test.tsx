// Test harness entry point
// This is a separate entry point for running E2E tests inside Tauri

import { Buffer } from "buffer";
import { Provider as JotaiProvider } from "jotai/react";
import React from "react";
import ReactDOM from "react-dom/client";

import "./styles.css";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getDefaultStore } from "jotai";
import { TestHarness } from "./components/TestHarness";
import { type Clock, TestClock } from "./lib/clock";
import { AppEventBus, type EventBus } from "./lib/events";
import { ProjectManager } from "./lib/project";
import { ISoundManager } from "./lib/sounds";
import { ProjectStore } from "./lib/store";
import { type TestBridge, initializeTestBridge } from "./lib/test-bridge";
import { AppWindows } from "./lib/windows";

// Test command types from Rust
interface TestCommand {
  command:
    | "get_state"
    | "reset_state"
    | "open_project"
    | "complete_task"
    | "change_state"
    | "get_event_history"
    | "clear_event_history"
    | "advance_clock"
    | "set_clock_time"
    | "get_clock_time";
  request_id: string;
  path?: string;
  task_name?: string;
  state?: string;
  ms?: number;
  timestamp?: number;
}

if (typeof globalThis.Buffer === "undefined") {
  globalThis.Buffer = Buffer;
}

// Declare the global test bridge
declare global {
  interface Window {
    __TEST_BRIDGE__: TestBridge;
  }
}

export interface AppControllers {
  projectManager: ProjectManager;
  appWindows: AppWindows;
  projectStore: ProjectStore;
  soundManager: ISoundManager;
  eventBus: EventBus;
  clock: Clock;
}

// Set up listener for test commands from Rust
async function setupTestCommandListener(testBridge: TestBridge) {
  await listen<TestCommand>("test:command", async (event) => {
    const cmd = event.payload;
    console.info("[Test Harness] Received command:", cmd);

    try {
      let responseData: unknown = null;

      switch (cmd.command) {
        case "get_state": {
          const state = testBridge.getState();
          responseData = state ?? null;
          break;
        }
        case "reset_state": {
          // Reset state by clearing the current project
          // The test should load a new fixture after reset
          responseData = null;
          break;
        }
        case "open_project": {
          if (cmd.path) {
            console.info("[Test Harness] Opening project:", cmd.path);
            try {
              await testBridge.openProject(cmd.path);
              // Return the state after opening so we can debug
              const stateAfter = testBridge.getState();
              console.info("[Test Harness] State after openProject:", stateAfter ? "loaded" : "null");
              // Return error info if state didn't load
              if (!stateAfter) {
                responseData = { debug: "state_is_null_after_open" };
              }
            } catch (e) {
              console.error("[Test Harness] Error opening project:", e);
              responseData = { error: String(e) };
            }
          }
          break;
        }
        case "complete_task": {
          if (cmd.task_name) {
            await testBridge.completeTask(cmd.task_name);
          }
          responseData = null;
          break;
        }
        case "change_state": {
          if (cmd.state) {
            await testBridge.changeState(cmd.state as "planning" | "working" | "break");
          }
          responseData = null;
          break;
        }
        case "get_event_history": {
          responseData = testBridge.getEventHistory();
          break;
        }
        case "clear_event_history": {
          testBridge.clearEventHistory();
          responseData = null;
          break;
        }
        case "advance_clock": {
          if (cmd.ms !== undefined) {
            testBridge.advanceClock(cmd.ms);
          }
          responseData = null;
          break;
        }
        case "set_clock_time": {
          if (cmd.timestamp !== undefined) {
            testBridge.setClockTime(cmd.timestamp);
          }
          responseData = null;
          break;
        }
        case "get_clock_time": {
          responseData = testBridge.getClockTime();
          break;
        }
      }

      // Send response back to Rust
      await invoke("test_respond", {
        requestId: cmd.request_id,
        data: responseData,
      });
      console.info("[Test Harness] Sent response for:", cmd.request_id);
    } catch (error) {
      console.error("[Test Harness] Error handling command:", error);
      // Still respond to avoid timeout, but with error indicator
      await invoke("test_respond", {
        requestId: cmd.request_id,
        data: { error: String(error) },
      });
    }
  });
}

// Initialize test harness
async function initializeTestHarness() {
  const jotaiStore = getDefaultStore();
  let controllers: AppControllers | undefined;
  let error: Error | undefined;

  try {
    console.info("[Test Harness] Initializing...");

    // Initialize core services
    const projectStore = await ProjectStore.initialize();
    console.info("[Test Harness] Store initialized");

    // Create TestClock for deterministic time control
    const clock = new TestClock();
    console.info("[Test Harness] TestClock initialized");

    const projectManager = new ProjectManager(projectStore, clock);
    console.info("[Test Harness] Project manager initialized");

    const appWindows = new AppWindows();
    await appWindows.initialize();
    console.info("[Test Harness] App windows initialized");

    // Initialize sound manager (will be a no-op in test mode)
    const soundManager = await ISoundManager.initialize(jotaiStore);
    console.info("[Test Harness] Sound manager initialized");

    // Create EventBus for event-driven testing
    const eventBus = new AppEventBus();
    console.info("[Test Harness] EventBus initialized");

    controllers = { projectManager, appWindows, projectStore, soundManager, eventBus, clock };

    // Initialize the test bridge and expose it globally
    const testBridge = initializeTestBridge(controllers);
    window.__TEST_BRIDGE__ = testBridge;
    console.info("[Test Harness] Test bridge exposed at window.__TEST_BRIDGE__");

    // Listen for test commands from the Unix socket server via Rust
    await setupTestCommandListener(testBridge);
    console.info("[Test Harness] Test command listener set up");

    // Don't auto-load a project in test mode - tests will load fixtures
  } catch (e) {
    console.error("[Test Harness] Failed to initialize:", e);
    error = e instanceof Error ? e : new Error(String(e));
  }

  console.info("[Test Harness] Rendering...", { controllers, error });

  // Render the test harness
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <JotaiProvider store={getDefaultStore()}>
        <TestHarness controllers={controllers} startupError={error} />
      </JotaiProvider>
    </React.StrictMode>,
  );
}

// Start the test harness
initializeTestHarness();
