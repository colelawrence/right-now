// Test runner utility for E2E testing
// Spawns the test harness binary and communicates via Unix socket

import { type ChildProcess, spawn } from "child_process";
import { existsSync } from "fs";
import { type Socket, createConnection } from "net";
import { tmpdir } from "os";
import { join } from "path";

// Test request types (matches Rust TestRequest enum)
export type TestRequest =
  | { type: "ping" }
  | { type: "create_temp_dir"; label?: string }
  | { type: "load_fixture"; name: string; temp_dir: string }
  | { type: "list_fixtures" }
  | { type: "get_state" }
  | { type: "reset_state" }
  | { type: "open_project"; path: string }
  | { type: "complete_task"; task_name: string }
  | { type: "change_state"; state: string }
  | { type: "cleanup_temp_dir"; path: string }
  | { type: "cleanup_all" }
  | { type: "shutdown" }
  // Clock control (deterministic time)
  | { type: "advance_clock"; ms: number }
  | { type: "set_clock_time"; timestamp: number }
  | { type: "get_clock_time" }
  // Event history (event-driven testing)
  | { type: "get_event_history" }
  | { type: "clear_event_history" };

// Test response types (matches Rust TestResponse enum)
export type TestResponse =
  | { type: "pong" }
  | { type: "ok"; data?: unknown }
  | { type: "temp_dir_created"; path: string }
  | { type: "fixture_loaded"; path: string }
  | { type: "fixture_list"; fixtures: string[] }
  | { type: "state"; state: unknown }
  | { type: "error"; message: string }
  | { type: "shutting_down" };

const DEFAULT_SOCKET_PATH = join(tmpdir(), "rightnow-test-harness.sock");
const DEFAULT_BINARY_PATH = "./target/debug/rn-test-harness";

export interface TauriTestRunnerOptions {
  binaryPath?: string;
  socketPath?: string;
  buildFirst?: boolean;
  connectTimeout?: number;
  startupTimeout?: number;
  /** Start the vite dev server before the harness (required for dev mode) */
  startDevServer?: boolean;
  devServerPort?: number;
  /** Use `tauri dev` command instead of running binary directly (handles dev server automatically) */
  useTauriDev?: boolean;
}

export class TauriTestRunner {
  private process: ChildProcess | null = null;
  private devServerProcess: ChildProcess | null = null;
  private socket: Socket | null = null;
  private socketPath: string;
  private binaryPath: string;
  private connectTimeout: number;
  private startupTimeout: number;
  private startDevServer: boolean;
  private devServerPort: number;
  private useTauriDev: boolean;

  constructor(options: TauriTestRunnerOptions = {}) {
    this.socketPath = options.socketPath ?? DEFAULT_SOCKET_PATH;
    this.binaryPath = options.binaryPath ?? DEFAULT_BINARY_PATH;
    this.connectTimeout = options.connectTimeout ?? 1000;
    this.startupTimeout = options.startupTimeout ?? 30000;
    this.startDevServer = options.startDevServer ?? true;
    this.devServerPort = options.devServerPort ?? 1421;
    this.useTauriDev = options.useTauriDev ?? false;
  }

  async start(): Promise<void> {
    // Start the vite dev server first (the harness binary expects it at devUrl)
    if (this.startDevServer && !this.useTauriDev) {
      await this.startViteDevServer();
    }

    if (this.useTauriDev) {
      console.log(`[TestRunner] Starting test harness via 'bun run tauri:test'`);

      // Use tauri dev which handles the dev server and binary together
      this.process = spawn("bun", ["run", "tauri:test"], {
        stdio: ["pipe", "pipe", "pipe"],
        env: { ...process.env },
        cwd: process.cwd(),
      });
    } else {
      console.log(`[TestRunner] Starting test harness: ${this.binaryPath}`);

      // Spawn the test harness binary
      this.process = spawn(this.binaryPath, [], {
        stdio: ["pipe", "pipe", "pipe"],
        env: { ...process.env },
      });
    }

    // Log stdout/stderr
    this.process.stdout?.on("data", (data) => {
      console.log(`[TestHarness] ${data.toString().trim()}`);
    });

    this.process.stderr?.on("data", (data) => {
      console.error(`[TestHarness] ${data.toString().trim()}`);
    });

    this.process.on("error", (error) => {
      console.error(`[TestRunner] Process error: ${error.message}`);
    });

    this.process.on("exit", (code) => {
      console.log(`[TestRunner] Process exited with code: ${code}`);
    });

    // Wait for the socket to become available
    await this.waitForSocket();

    // Connect to the socket
    await this.connect();

    // Verify connection with ping
    const response = await this.invoke({ type: "ping" });
    if (response.type !== "pong") {
      throw new Error(`Unexpected ping response: ${JSON.stringify(response)}`);
    }

    console.log("[TestRunner] Socket connection verified, waiting for frontend...");

    // Wait for frontend to be ready by polling getState
    // The frontend needs time to initialize after the Rust side is ready
    await this.waitForFrontendReady();

    console.log("[TestRunner] Test harness ready");
  }

  /**
   * Wait for frontend to be ready by polling getState
   * The socket/Rust side comes up before the frontend finishes initializing
   */
  private async waitForFrontendReady(): Promise<void> {
    const startTime = Date.now();
    const timeout = this.startupTimeout;
    const pollInterval = 500;
    let lastError: Error | null = null;

    while (Date.now() - startTime < timeout) {
      try {
        // getState requires frontend to be ready and responding
        const response = await this.invoke({ type: "get_state" });
        if (response.type === "state") {
          console.log("[TestRunner] Frontend ready");
          return;
        }
        if (response.type === "error") {
          lastError = new Error(response.message);
          console.log(`[TestRunner] Frontend not ready yet: ${response.message}`);
        }
      } catch (error) {
        lastError = error instanceof Error ? error : new Error(String(error));
        console.log(`[TestRunner] Frontend not ready yet: ${lastError.message}`);
      }
      await new Promise((resolve) => setTimeout(resolve, pollInterval));
    }

    throw new Error(`Timeout waiting for frontend to be ready after ${timeout}ms. Last error: ${lastError?.message}`);
  }

  /**
   * Start the vite dev server for the test harness frontend
   */
  private async startViteDevServer(): Promise<void> {
    console.log("[TestRunner] Starting vite dev server...");

    this.devServerProcess = spawn("bun", ["run", "dev:test"], {
      stdio: ["pipe", "pipe", "pipe"],
      env: { ...process.env },
      cwd: process.cwd(),
    });

    // Log output
    this.devServerProcess.stdout?.on("data", (data) => {
      const msg = data.toString().trim();
      if (msg) console.log(`[Vite] ${msg}`);
    });

    this.devServerProcess.stderr?.on("data", (data) => {
      const msg = data.toString().trim();
      if (msg) console.error(`[Vite] ${msg}`);
    });

    this.devServerProcess.on("error", (error) => {
      console.error(`[TestRunner] Vite process error: ${error.message}`);
    });

    // Wait for the server to be ready by checking if the port is listening
    await this.waitForDevServer();
    console.log("[TestRunner] Vite dev server ready");
  }

  /**
   * Wait for the vite dev server to be ready
   */
  private async waitForDevServer(): Promise<void> {
    const startTime = Date.now();
    const timeout = 30000; // 30s for vite to start
    const checkInterval = 200;

    while (Date.now() - startTime < timeout) {
      try {
        // Try to fetch from the dev server
        const response = await fetch(`http://localhost:${this.devServerPort}/`, {
          method: "HEAD",
        });
        if (response.ok) {
          return;
        }
      } catch {
        // Server not ready yet
      }
      await new Promise((resolve) => setTimeout(resolve, checkInterval));
    }

    throw new Error(`Timeout waiting for vite dev server after ${timeout}ms`);
  }

  private async waitForSocket(): Promise<void> {
    const startTime = Date.now();
    const checkInterval = 500; // Slower polling to reduce race conditions

    console.log(`[TestRunner] Waiting for socket at: ${this.socketPath}`);

    while (Date.now() - startTime < this.startupTimeout) {
      // First check if socket file exists to avoid connection errors
      if (!existsSync(this.socketPath)) {
        await new Promise((resolve) => setTimeout(resolve, checkInterval));
        continue;
      }

      // Small delay after file exists to let it fully initialize
      await new Promise((resolve) => setTimeout(resolve, 100));

      try {
        // Socket file exists, try to connect
        await this.tryConnect();
        console.log("[TestRunner] Socket connection successful");
        return;
      } catch (error: unknown) {
        const err = error as { code?: string; message?: string };
        // Socket exists but connection failed, wait and retry
        // ENOENT or ECONNREFUSED are expected during startup
        if (err.code === "ENOENT" || err.code === "ECONNREFUSED") {
          console.log(`[TestRunner] Socket not ready (${err.code}), retrying...`);
        } else {
          console.log(`[TestRunner] Socket connection failed: ${err.message}, retrying...`);
        }
        await new Promise((resolve) => setTimeout(resolve, checkInterval));
      }
    }

    throw new Error(`Timeout waiting for socket after ${this.startupTimeout}ms`);
  }

  private async tryConnect(): Promise<void> {
    return new Promise((resolve, reject) => {
      const socket = createConnection(this.socketPath);

      // Must attach error handler BEFORE connection callback
      // to prevent unhandled error event
      socket.once("error", (error) => {
        socket.destroy();
        reject(error);
      });

      socket.once("connect", () => {
        socket.destroy();
        resolve();
      });
    });
  }

  private async connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error(`Connection timeout after ${this.connectTimeout}ms`));
      }, this.connectTimeout);

      this.socket = createConnection(this.socketPath, () => {
        clearTimeout(timeout);
        resolve();
      });

      this.socket.on("error", (error) => {
        clearTimeout(timeout);
        reject(error);
      });
    });
  }

  async invoke(request: TestRequest): Promise<TestResponse> {
    if (!this.socket) {
      throw new Error("Not connected to test harness");
    }

    return new Promise((resolve, reject) => {
      // Send the request
      const message = JSON.stringify(request) + "\n";
      this.socket!.write(message);

      // Read the response
      let buffer = "";

      const onData = (data: Buffer) => {
        buffer += data.toString();

        const newlineIndex = buffer.indexOf("\n");
        if (newlineIndex !== -1) {
          const line = buffer.substring(0, newlineIndex);
          buffer = buffer.substring(newlineIndex + 1);

          this.socket!.off("data", onData);
          this.socket!.off("error", onError);

          try {
            const response = JSON.parse(line) as TestResponse;
            resolve(response);
          } catch (error) {
            reject(new Error(`Failed to parse response: ${line}`));
          }
        }
      };

      const onError = (error: Error) => {
        this.socket!.off("data", onData);
        reject(error);
      };

      this.socket!.on("data", onData);
      this.socket!.on("error", onError);
    });
  }

  async stop(): Promise<void> {
    console.log("[TestRunner] Stopping test harness");

    // Send shutdown request
    if (this.socket) {
      try {
        await this.invoke({ type: "shutdown" });
      } catch {
        // Ignore errors during shutdown
      }

      this.socket.destroy();
      this.socket = null;
    }

    // Kill the harness process if still running
    if (this.process) {
      this.process.kill("SIGTERM");

      // Wait for process to exit
      await new Promise<void>((resolve) => {
        if (this.process) {
          this.process.on("exit", () => resolve());
          setTimeout(resolve, 5000); // Timeout after 5s
        } else {
          resolve();
        }
      });

      this.process = null;
    }

    // Kill the vite dev server if running
    if (this.devServerProcess) {
      this.devServerProcess.kill("SIGTERM");

      await new Promise<void>((resolve) => {
        if (this.devServerProcess) {
          this.devServerProcess.on("exit", () => resolve());
          setTimeout(resolve, 3000);
        } else {
          resolve();
        }
      });

      this.devServerProcess = null;
    }

    console.log("[TestRunner] Test harness stopped");
  }

  // Convenience methods

  async createTempDir(label?: string): Promise<string> {
    const response = await this.invoke({ type: "create_temp_dir", label });
    if (response.type === "temp_dir_created") {
      return response.path;
    }
    throw new Error(`Failed to create temp dir: ${JSON.stringify(response)}`);
  }

  async loadFixture(name: string, tempDir: string): Promise<string> {
    const response = await this.invoke({ type: "load_fixture", name, temp_dir: tempDir });
    if (response.type === "fixture_loaded") {
      return response.path;
    }
    throw new Error(`Failed to load fixture: ${JSON.stringify(response)}`);
  }

  async listFixtures(): Promise<string[]> {
    const response = await this.invoke({ type: "list_fixtures" });
    if (response.type === "fixture_list") {
      return response.fixtures;
    }
    throw new Error(`Failed to list fixtures: ${JSON.stringify(response)}`);
  }

  async getState(): Promise<unknown> {
    const response = await this.invoke({ type: "get_state" });
    if (response.type === "state") {
      return response.state;
    }
    throw new Error(`Failed to get state: ${JSON.stringify(response)}`);
  }

  async resetState(): Promise<void> {
    const response = await this.invoke({ type: "reset_state" });
    if (response.type !== "ok") {
      throw new Error(`Failed to reset state: ${JSON.stringify(response)}`);
    }
  }

  async openProject(path: string): Promise<void> {
    const response = await this.invoke({ type: "open_project", path });
    if (response.type !== "ok") {
      throw new Error(`Failed to open project: ${JSON.stringify(response)}`);
    }
  }

  async completeTask(taskName: string): Promise<void> {
    const response = await this.invoke({ type: "complete_task", task_name: taskName });
    if (response.type !== "ok") {
      throw new Error(`Failed to complete task: ${JSON.stringify(response)}`);
    }
  }

  async changeState(state: "planning" | "working" | "break"): Promise<void> {
    const response = await this.invoke({ type: "change_state", state });
    if (response.type !== "ok") {
      throw new Error(`Failed to change state: ${JSON.stringify(response)}`);
    }
  }

  async cleanupAll(): Promise<void> {
    const response = await this.invoke({ type: "cleanup_all" });
    if (response.type !== "ok") {
      throw new Error(`Failed to cleanup: ${JSON.stringify(response)}`);
    }
  }

  async cleanupTempDir(path: string): Promise<void> {
    const response = await this.invoke({ type: "cleanup_temp_dir", path });
    if (response.type !== "ok") {
      throw new Error(`Failed to cleanup temp dir: ${JSON.stringify(response)}`);
    }
  }

  /**
   * Wait for state to satisfy a condition
   * Polls getState() until the predicate returns true or timeout
   */
  async waitForState(
    predicate: (state: unknown) => boolean,
    options: { timeout?: number; pollInterval?: number } = {},
  ): Promise<unknown> {
    const { timeout = 5000, pollInterval = 100 } = options;
    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      try {
        const state = await this.getState();
        if (predicate(state)) {
          return state;
        }
      } catch {
        // State not available yet, keep polling
      }
      await new Promise((resolve) => setTimeout(resolve, pollInterval));
    }

    throw new Error(`Timeout waiting for state condition after ${timeout}ms`);
  }

  /**
   * Wait for a project to be loaded
   */
  async waitForProject(timeout = 5000): Promise<unknown> {
    return this.waitForState(
      (state) => {
        const s = state as { projectFile?: unknown } | null;
        return s?.projectFile != null;
      },
      { timeout },
    );
  }

  // Clock control methods (deterministic time for tests)

  /**
   * Advance the TestClock by specified milliseconds
   * This triggers any interval-based logic (e.g., timer checks)
   */
  async advanceClock(ms: number): Promise<void> {
    const response = await this.invoke({ type: "advance_clock", ms });
    if (response.type !== "ok") {
      throw new Error(`Failed to advance clock: ${JSON.stringify(response)}`);
    }
  }

  /**
   * Set the TestClock to a specific timestamp
   * Use this to jump to specific moments (e.g., "5 minutes before timer ends")
   */
  async setClockTime(timestamp: number): Promise<void> {
    const response = await this.invoke({ type: "set_clock_time", timestamp });
    if (response.type !== "ok") {
      throw new Error(`Failed to set clock time: ${JSON.stringify(response)}`);
    }
  }

  /**
   * Get the current TestClock time
   */
  async getClockTime(): Promise<number> {
    const response = await this.invoke({ type: "get_clock_time" });
    if (response.type === "ok" && typeof response.data === "number") {
      return response.data;
    }
    throw new Error(`Failed to get clock time: ${JSON.stringify(response)}`);
  }

  // Event history methods (event-driven testing)

  /**
   * Get all events emitted via the EventBus since last clear
   * Use for asserting on side effects (sounds, notifications, etc.)
   */
  async getEventHistory(): Promise<AppEvent[]> {
    const response = await this.invoke({ type: "get_event_history" });
    if (response.type === "ok" && Array.isArray(response.data)) {
      return response.data as AppEvent[];
    }
    throw new Error(`Failed to get event history: ${JSON.stringify(response)}`);
  }

  /**
   * Clear the event history
   * Call at the start of each test for isolation
   */
  async clearEventHistory(): Promise<void> {
    const response = await this.invoke({ type: "clear_event_history" });
    if (response.type !== "ok") {
      throw new Error(`Failed to clear event history: ${JSON.stringify(response)}`);
    }
  }
}

// Re-export event types from the EventBus
// These match the actual types in src/lib/events.ts
export type { AppEvent } from "../../src/lib/events";
