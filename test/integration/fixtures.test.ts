// Integration tests for fixture loading
// Tests the temp directory and fixture loading functionality

import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import {
  cleanupTestTempDir,
  createTestTempDir,
  getRunner,
  loadTestFixture,
  setupTestHarness,
  teardownTestHarness,
} from "../harness/setup";

describe("Fixtures", () => {
  beforeAll(async () => {
    await setupTestHarness();
  });

  afterAll(async () => {
    await teardownTestHarness();
  });

  beforeEach(async () => {
    const runner = getRunner();
    await runner.cleanupAll();
  });

  afterEach(async () => {
    await cleanupTestTempDir();
  });

  it("should list available fixtures", async () => {
    const runner = getRunner();
    const fixtures = await runner.listFixtures();

    expect(fixtures).toBeArray();
    expect(fixtures).toContain("minimal");
    expect(fixtures).toContain("complex");
    expect(fixtures).toContain("with-sessions");
    expect(fixtures).toContain("empty");
  });

  it("should create a temp directory", async () => {
    const tempDir = await createTestTempDir();

    expect(tempDir).toBeString();
    expect(tempDir).toStartWith("/");
  });

  it("should load a fixture into temp directory", async () => {
    const fixturePath = await loadTestFixture("minimal");

    expect(fixturePath).toBeString();
    expect(fixturePath).toEndWith("minimal.md");
  });

  it("should load multiple fixtures", async () => {
    const tempDir = await createTestTempDir();
    const runner = getRunner();

    const minimal = await runner.loadFixture("minimal", tempDir);
    const complex = await runner.loadFixture("complex", tempDir);

    expect(minimal).toEndWith("minimal.md");
    expect(complex).toEndWith("complex.md");
  });
});
