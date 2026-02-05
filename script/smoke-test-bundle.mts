#!/usr/bin/env bun
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

/**
 * Post-build smoke test for the Right Now app bundle.
 *
 * Validates:
 * 1. App bundle exists
 * 2. Required sidecars are present (rn-desktop-2, right-now-daemon, todo, todo-shim)
 * 3. Test harness is NOT present (rn-test-harness)
 * 4. Deep link scheme "todos" is configured in Info.plist
 */

const PRODUCT_NAME = "Right Now";
const REQUIRED_SIDECARS = ["rn-desktop-2", "right-now-daemon", "todo", "todo-shim"];
const FORBIDDEN_BINARIES = ["rn-test-harness"];
const REQUIRED_URL_SCHEME = "todos";

type Result = { ok: true } | { ok: false; error: string };

function validateMacOSBundle(): Result {
  const appPath = join(process.cwd(), "target", "release", "bundle", "macos", `${PRODUCT_NAME}.app`);
  const macOSDir = join(appPath, "Contents", "MacOS");
  const infoPlistPath = join(appPath, "Contents", "Info.plist");

  // 1. Check app bundle exists
  if (!existsSync(appPath)) {
    return { ok: false, error: `App bundle not found at: ${appPath}` };
  }

  if (!existsSync(macOSDir)) {
    return { ok: false, error: `MacOS directory not found at: ${macOSDir}` };
  }

  // 2. Check required sidecars are present
  for (const binary of REQUIRED_SIDECARS) {
    const binaryPath = join(macOSDir, binary);
    if (!existsSync(binaryPath)) {
      return { ok: false, error: `Required sidecar missing: ${binary}` };
    }
  }

  // 3. Check forbidden binaries are NOT present
  for (const binary of FORBIDDEN_BINARIES) {
    const binaryPath = join(macOSDir, binary);
    if (existsSync(binaryPath)) {
      return { ok: false, error: `Forbidden binary found (should be excluded): ${binary}` };
    }
  }

  // 4. Check Info.plist for deep link scheme
  if (!existsSync(infoPlistPath)) {
    return { ok: false, error: `Info.plist not found at: ${infoPlistPath}` };
  }

  try {
    const plistContent = readFileSync(infoPlistPath, "utf8");
    if (!plistContent.includes(`<string>${REQUIRED_URL_SCHEME}</string>`)) {
      return { ok: false, error: `Deep link scheme "${REQUIRED_URL_SCHEME}" not found in Info.plist` };
    }
  } catch (err) {
    return { ok: false, error: `Failed to read Info.plist: ${err}` };
  }

  // 5. Verify todo CLI can execute (quick sanity check)
  const todoPath = join(macOSDir, "todo");
  const todoResult = spawnSync(todoPath, ["help"], {
    timeout: 5000,
    encoding: "utf8",
  });

  if (todoResult.error) {
    return { ok: false, error: `Failed to execute 'todo help': ${todoResult.error.message}` };
  }

  if (todoResult.status !== 0) {
    return {
      ok: false,
      error: `'todo help' exited with code ${todoResult.status}\nstderr: ${todoResult.stderr}`,
    };
  }

  return { ok: true };
}

function main() {
  const args = process.argv.slice(2);
  const shouldBuild = args.includes("--build");

  console.log("🧪 Right Now Bundle Smoke Test\n");

  // Platform check
  if (process.platform !== "darwin") {
    console.log("⏭️  Skipping: macOS-only test (current platform: %s)", process.platform);
    process.exit(0);
  }

  // Optional: build first
  if (shouldBuild) {
    console.log("🔨 Building app bundle (this may take several minutes)...");
    const buildResult = spawnSync("bun", ["run", "tauri", "build"], {
      stdio: "inherit",
      timeout: 1800_000, // 30 minutes
    });

    if (buildResult.status !== 0) {
      console.error("\n❌ Build failed");
      process.exit(1);
    }
    console.log();
  }

  // Validate bundle
  console.log("🔍 Validating bundle contents...\n");

  const result = validateMacOSBundle();

  if (!result.ok) {
    console.error("❌ Smoke test failed:");
    console.error("   %s\n", result.error);
    process.exit(1);
  }

  // Success summary
  console.log("✅ All checks passed:");
  console.log("   • App bundle exists");
  console.log("   • Sidecars present: %s", REQUIRED_SIDECARS.join(", "));
  console.log("   • Test harness excluded: %s", FORBIDDEN_BINARIES.join(", "));
  console.log('   • Deep link scheme configured: "%s"', REQUIRED_URL_SCHEME);
  console.log("   • CLI executable: 'todo help' runs successfully");
  console.log();
}

main();
