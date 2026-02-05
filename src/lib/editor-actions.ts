/**
 * Editor workflow utilities: open TODO file, reveal in Finder, copy path.
 */

import { dirname } from "@tauri-apps/api/path";
import { openPath } from "@tauri-apps/plugin-opener";

/**
 * Open the TODO file in the user's default editor.
 */
export async function openTodoFile(fullPath: string): Promise<void> {
  await openPath(fullPath);
}

/**
 * Reveal the TODO file in Finder (macOS) / File Explorer (Windows) / file manager (Linux).
 * Opens the containing directory.
 */
export async function revealTodoFile(fullPath: string): Promise<void> {
  const dir = await dirname(fullPath);
  await openPath(dir);
}

/**
 * Copy the TODO file path to the clipboard.
 */
export async function copyTodoFilePath(fullPath: string): Promise<void> {
  await navigator.clipboard.writeText(fullPath);
}

/**
 * Format a project path for display: show last 2 segments.
 * e.g. "/Users/cole/dev/my-app/TODO.md" → "my-app/TODO.md"
 */
export function formatDisplayPath(path: string): string {
  const segments = path.split("/").filter(Boolean);
  if (segments.length <= 2) return path;
  return segments.slice(-2).join("/");
}
