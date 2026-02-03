import { path as tauriPath } from "@tauri-apps/api";
import { type UnwatchFn, type WatchEventKind, watch } from "@tauri-apps/plugin-fs";

/**
 * Watches a single project file for external changes.
 *
 * Notes:
 * - Watches the *parent directory* instead of the file itself to better handle
 *   atomic writes (write-to-temp + rename) from editors and the daemon.
 * - Coalesces bursts of events so we don't run overlapping reloads.
 */
export class FileWatcher {
  private unwatch?: UnwatchFn;
  private watchedFile?: string;
  private inFlight = false;
  private pending = false;

  async watchProject(filePath: string, onChange: () => Promise<void>) {
    // Stop any existing watcher (we only watch one project at a time)
    this.cleanup();

    this.watchedFile = filePath;

    const dir = await tauriPath.dirname(filePath);
    const targetBasename = basename(filePath);
    if (!targetBasename) {
      throw new Error(`[FileWatcher] Invalid file path: ${filePath}`);
    }

    const runCoalesced = async () => {
      // If a reload is already running, just mark a pending run.
      if (this.inFlight) {
        this.pending = true;
        return;
      }

      this.inFlight = true;
      try {
        do {
          this.pending = false;

          // If the watch target changed while we were waiting, abort.
          if (this.watchedFile !== filePath) return;

          try {
            await onChange();
          } catch (error) {
            // Never let exceptions break the watcher loop.
            console.error("[FileWatcher] onChange error:", error);
          }
        } while (this.pending);
      } finally {
        this.inFlight = false;
      }
    };

    this.unwatch = await watch(
      dir,
      (event) => {
        // Ignore events for an old watcher that hasn't fully shut down.
        if (this.watchedFile !== filePath) return;

        // Ignore noisy access events.
        if (!isMeaningfulEvent(event.type)) return;

        // If we're watching the parent directory, filter to just this file.
        const relevant = event.paths?.some((p) => basename(p) === targetBasename);
        if (!relevant) return;

        void runCoalesced();
      },
      {
        recursive: false,
        // Small debounce to avoid event storms from safe-save editors.
        delayMs: 200,
      },
    );
  }

  cleanup() {
    if (this.unwatch) {
      this.unwatch();
      this.unwatch = undefined;
    }
    this.watchedFile = undefined;
    this.inFlight = false;
    this.pending = false;
  }
}

function normalizeEventPath(p: string): string {
  return p.startsWith("file://") ? p.slice("file://".length) : p;
}

function basename(p: string): string {
  const normalized = normalizeEventPath(p);
  const parts = normalized.split(/[\\/]/);
  return parts[parts.length - 1] ?? normalized;
}

function isMeaningfulEvent(kind: WatchEventKind): boolean {
  // 'any'/'other' generally indicate something interesting happened.
  if (kind === "any" || kind === "other") return true;

  // We only care about create/modify/remove. Access events are extremely noisy.
  if (typeof kind === "object") {
    return "modify" in kind || "create" in kind || "remove" in kind;
  }

  return false;
}
