import { spawnSync } from "node:child_process";
import { readFileSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";

function getConfigPath(args: string[]): string {
  const idx = args.indexOf("--config");
  if (idx !== -1 && idx + 1 < args.length) {
    return args[idx + 1];
  }
  return "src-tauri/tauri.conf.json";
}

function readProductName(configPath: string): string | undefined {
  try {
    const raw = readFileSync(resolve(configPath), "utf8");
    const json = JSON.parse(raw) as { productName?: string };
    return json.productName;
  } catch {
    return undefined;
  }
}

function safeRm(path: string) {
  try {
    rmSync(path, { force: true });
  } catch {
    // ignore
  }
}

const args = process.argv.slice(2);
const configPath = getConfigPath(args);
const isBuild = args[0] === "build";
const isTestConfig = /tauri\.test\.conf\.json$/.test(configPath);

const tauriBin = join(process.cwd(), "node_modules", ".bin", process.platform === "win32" ? "tauri.cmd" : "tauri");

const result = spawnSync(tauriBin, args, {
  stdio: "inherit",
  env: { ...process.env },
});

const exitCode = result.status ?? 1;

if (exitCode === 0 && isBuild && !isTestConfig) {
  // Tauri currently bundles all cargo binaries declared in src-tauri/Cargo.toml.
  // rn-test-harness is a dev/test-only binary, but Tauri still attempts to bundle it.
  // We generate a stub binary at build time (src-tauri/build.rs) to satisfy the bundler,
  // then remove it from the final app bundle as a post-step.

  const productName = readProductName(configPath) ?? "Right Now";

  // Remove from the app bundle output (macOS only).
  if (process.platform === "darwin") {
    const bundledHarness = join(
      process.cwd(),
      "target",
      "release",
      "bundle",
      "macos",
      `${productName}.app`,
      "Contents",
      "MacOS",
      "rn-test-harness",
    );
    safeRm(bundledHarness);
  }

  // Also remove the stub from the cargo target dir to reduce confusion.
  const harnessBinName = process.platform === "win32" ? "rn-test-harness.exe" : "rn-test-harness";
  safeRm(join(process.cwd(), "target", "release", harnessBinName));
}

process.exit(exitCode);
