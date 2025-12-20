// Test setup for E2E integration tests
// Manages test harness lifecycle across test files

import { afterAll, afterEach, beforeAll, beforeEach } from "bun:test";
import { TauriTestRunner } from "./runner";

let runner: TauriTestRunner | null = null;
let tempDir: string | null = null;

/**
 * Initialize the test harness before all tests.
 * Call this in your test file's top-level beforeAll.
 */
export async function setupTestHarness(): Promise<TauriTestRunner> {
  if (runner) {
    return runner;
  }

  // Use tauri dev mode which properly handles the dev server and binary communication
  // The tauri dev command handles starting vite and connecting the binary to devUrl
  runner = new TauriTestRunner({
    // Special mode: use tauri dev instead of running binary directly
    useTauriDev: true,
    startupTimeout: 120000, // 120s for harness + frontend initialization (tauri dev takes longer)
    // Don't start dev server separately - tauri dev does this
    startDevServer: false,
    devServerPort: 1421, // Match vite.config.test.ts port
  });

  await runner.start();
  return runner;
}

/**
 * Shutdown the test harness after all tests.
 * Call this in your test file's top-level afterAll.
 */
export async function teardownTestHarness(): Promise<void> {
  if (runner) {
    await runner.stop();
    runner = null;
  }
}

/**
 * Get the current test runner instance.
 * Throws if harness not initialized.
 */
export function getRunner(): TauriTestRunner {
  if (!runner) {
    throw new Error("Test harness not initialized. Call setupTestHarness() first.");
  }
  return runner;
}

/**
 * Create a temp directory for the current test.
 * Returns the path to the temp directory.
 */
export async function createTestTempDir(): Promise<string> {
  const r = getRunner();
  tempDir = await r.createTempDir();
  return tempDir;
}

/**
 * Cleanup the current test's temp directory.
 */
export async function cleanupTestTempDir(): Promise<void> {
  if (tempDir) {
    const r = getRunner();
    await r.invoke({ type: "cleanup_temp_dir", path: tempDir });
    tempDir = null;
  }
}

/**
 * Load a fixture into the current test's temp directory.
 * Creates a temp directory if one doesn't exist.
 */
export async function loadTestFixture(name: string): Promise<string> {
  const r = getRunner();
  if (!tempDir) {
    tempDir = await createTestTempDir();
  }
  return await r.loadFixture(name, tempDir);
}

// Standard test lifecycle hooks that can be imported
export const standardHooks = {
  beforeAll: async () => {
    await setupTestHarness();
  },

  afterAll: async () => {
    await teardownTestHarness();
  },

  beforeEach: async () => {
    const r = getRunner();
    await r.resetState();
    await r.cleanupAll();
  },

  afterEach: async () => {
    await cleanupTestTempDir();
  },
};

/**
 * Apply standard test hooks in one call.
 * Usage:
 *   import { applyStandardHooks } from "./setup";
 *   applyStandardHooks();
 */
export function applyStandardHooks(): void {
  beforeAll(standardHooks.beforeAll);
  afterAll(standardHooks.afterAll);
  beforeEach(standardHooks.beforeEach);
  afterEach(standardHooks.afterEach);
}
