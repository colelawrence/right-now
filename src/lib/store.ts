import { Store } from "@tauri-apps/plugin-store";

interface TimingDetails {
  tenSecondIncrements: number[];
  lastUpdateTime: number;
}

// for reference
interface StoreSchema {
  recentProjects: string[];
  lastActiveProject?: string;
  hasSeenWalkthrough?: boolean;
  timing: Record<string, TimingDetails>;
}

/**
 * Pure helper: compute next recentProjects list given an existing list and a new path.
 * - Deduplicates and moves `path` to the front.
 * - Limits to `limit` items (default 10).
 */
export function nextRecentProjects(existing: string[], path: string, limit = 10): string[] {
  return [path, ...existing.filter((p) => p !== path)].slice(0, limit);
}

export class ProjectStore {
  private constructor(private store: Store) {}

  static async initialize(): Promise<ProjectStore> {
    try {
      // Try to get existing store first
      let store = await Store.get("ProjectStore.json");
      if (!store) {
        // Create new store with defaults if it doesn't exist
        store = await Store.load("ProjectStore.json", {
          autoSave: true, // Enable autosave with default 100ms debounce
        });
      }

      return new ProjectStore(store);
    } catch (error) {
      throw new Error(`Failed to initialize store: ${error}`);
    }
  }

  async getRecentProjects(): Promise<string[]> {
    return (await this.store.get("recentProjects")) ?? [];
  }

  async getLastActiveProject(): Promise<string | undefined> {
    return await this.store.get("lastActiveProject");
  }

  async addRecentProject(path: string): Promise<void> {
    const projects = await this.getRecentProjects();
    const updated = nextRecentProjects(projects, path);
    await this.store.set("recentProjects", updated);
    await this.store.set("lastActiveProject", path);
  }

  async getHasSeenWalkthrough(): Promise<boolean> {
    return (await this.store.get("hasSeenWalkthrough")) ?? false;
  }

  async setHasSeenWalkthrough(seen: boolean): Promise<void> {
    await this.store.set("hasSeenWalkthrough", seen);
  }

  async getTaskTiming(taskId: string): Promise<TimingDetails | undefined> {
    return await this.store.get(`timing.${taskId}`);
  }

  async updateTaskTiming(taskId: string, timing: TimingDetails): Promise<void> {
    await this.store.set(`timing.${taskId}`, timing);
  }

  async clearTimings(): Promise<void> {
    await this.store.set("timing", {});
  }

  // Optional: Subscribe to store changes
  async onTimingChange(taskId: string, callback: (timing: TimingDetails | undefined) => void): Promise<() => void> {
    return await this.store.onKeyChange(`timing.${taskId}`, callback);
  }
}
