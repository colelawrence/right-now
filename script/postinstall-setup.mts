import { chmodSync, existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const logPrefix = `[${import.meta.filename.split("/").slice(-2).join("/")}]`;
let nextStep = "Creating git hooks";
try {
  nextStep = "creating git hooks directory";
  const hookDir = join(".git", "hooks");
  if (!existsSync(hookDir)) mkdirSync(hookDir, { recursive: true });

  nextStep = "writing git hooks precommit file";
  // The script you want to run before commit
  const preCommitScript = "bunx lint-staged";

  const hookFile = join(hookDir, "pre-commit");
  writeFileSync(hookFile, `#!/usr/bin/env sh\n${preCommitScript}`);
  nextStep = "setting executable permissions for git hooks precommit file";
  chmodSync(hookFile, 0o755);
} catch (error) {
  console.error(logPrefix, `Error setting up Git hooks while ${nextStep}:`, error);
  process.exit(1);
}

console.log(logPrefix, "Git hooks setup complete.");
