import { path } from "@tauri-apps/api";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { type DirEntry, readDir, readTextFile, stat, writeTextFile } from "@tauri-apps/plugin-fs";
import { type ProjectFile, ProjectStateEditor } from "./ProjectStateEditor";
import type { Clock } from "./clock";
import { realClock } from "./clock";
import type { ProjectStore } from "./store";
import { FileWatcher } from "./watcher";
import { withError } from "./withError";

export type { ProjectFile };

class ProjectError extends Error {}

export type WorkState = "planning" | "working" | "break";

export interface StateTransition {
  startedAt: number;
  endsAt?: number;
}

export type LoadedProjectState = {
  fullPath: string;
  projectFile: ProjectFile;
  textContent: string;
  virtual: boolean;
  // Ephemeral state that doesn't persist to the file
  workState: WorkState;
  stateTransitions: StateTransition;
};

type ProjectChangeCallback = (project: Readonly<LoadedProjectState> | undefined) => void | Promise<void>;

export class ProjectManager {
  private projectStore: ProjectStore;
  private clock: Clock;
  private watcher: FileWatcher;
  private currentFile?: Readonly<LoadedProjectState>;
  private changeListeners: Set<ProjectChangeCallback> = new Set();

  constructor(projectStore: ProjectStore, clock: Clock = realClock) {
    this.projectStore = projectStore;
    this.clock = clock;
    this.watcher = new FileWatcher();
  }

  async openProject(defaultProject?: string) {
    const selected = await open({
      multiple: false,
      title: "Select folder or TODO file",
      defaultPath: defaultProject,
      directory: true, // Allow folder selection
    });

    if (selected && !Array.isArray(selected)) {
      await this.handleFolderOrFile(selected).catch(withError(`Failed to handle selection (${selected})`));
    }
  }

  private async handleFolderOrFile(selectedPath: string): Promise<void> {
    try {
      const fileInfo = await stat(selectedPath);

      if (fileInfo.isDirectory) {
        // Search for existing TODO files
        const entries = await readDir(selectedPath);
        const todoFile = entries.find((entry: DirEntry) => {
          const filename = entry.name.toLowerCase();
          return (
            filename.endsWith(".md") &&
            (filename.startsWith("todo") || filename.startsWith("to-do") || filename.startsWith("the"))
          );
        });

        if (todoFile) {
          // Use existing TODO file
          await this.loadProject(await path.join(selectedPath, todoFile.name), "absolute");
        } else {
          // Create new TODO.md with template
          const todoPath = await path.join(selectedPath, "TODO.md");
          const template = `---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---

# Tasks

- [ ] First task
`;
          await writeTextFile(todoPath, template);
          await this.loadProject(todoPath, "absolute");
        }
      } else {
        // Direct file selection
        await this.loadProject(selectedPath, "absolute");
      }
    } catch (error) {
      console.error("Error handling folder or file:", error);
      throw error;
    }
  }

  async loadProject(filePath: string, type: "absolute" | "virtual"): Promise<void> {
    const fullPath = type === "absolute" ? filePath : await path.resolve(await path.appDataDir(), filePath);
    const reload = async () => {
      const textContent = await readTextFile(fullPath).catch(
        withError((err) => `Error reading file (${fullPath}): ${JSON.stringify(err)}`, ProjectError),
      );
      if (this.currentFile && this.currentFile.fullPath === fullPath && this.currentFile.textContent === textContent) {
        console.info("Skipping load of project", fullPath, "because it's already loaded");
        return;
      }
      const projectFile = ProjectStateEditor.parse(textContent);
      if (this.currentFile && this.currentFile.fullPath === fullPath) {
        // Initialize with planning state
        this.currentFile = {
          ...this.currentFile,
          textContent,
          projectFile,
        };
      } else {
        // Initialize with planning state
        this.currentFile = {
          fullPath,
          textContent,
          projectFile,
          virtual: type === "virtual",
          workState: "planning",
          stateTransitions: {
            startedAt: this.clock.now(),
          },
        };
      }
      await this.notifySubscribers(this.currentFile);
    };

    await reload();
    // Update recent projects in store
    await this.projectStore.addRecentProject(fullPath);

    // Set up file watcher for external changes
    await this.watcher.watchProject(fullPath, reload);

    if (type === "absolute") {
      // Record the current project path for CLI fallbacks
      try {
        await invoke("set_current_project_path", { path: fullPath });
      } catch (error) {
        console.warn("Failed to record current project path", error);
      }
    }
  }

  subscribe(callback: ProjectChangeCallback): () => void {
    callback(this.currentFile);
    this.changeListeners.add(callback);
    return () => void this.changeListeners.delete(callback);
  }

  async updateProject(fn: (project: ProjectFile) => void | boolean) {
    if (!this.currentFile) return;
    const project = structuredClone(this.currentFile.projectFile);
    if (fn(project) === false) return;
    this.currentFile = { ...this.currentFile, projectFile: project };
    const updatedContent = ProjectStateEditor.update(this.currentFile.textContent, project);
    await writeTextFile(this.currentFile.fullPath, updatedContent);
    await this.notifySubscribers(this.currentFile);
  }

  async updateWorkState(newState: WorkState): Promise<void> {
    if (!this.currentFile) return;

    // Only update if state is actually changing
    if (this.currentFile.workState === newState) return;

    const startedAt = this.clock.now();
    this.currentFile = {
      ...this.currentFile,
      workState: newState,
      stateTransitions: {
        startedAt,
        endsAt:
          newState === "working"
            ? startedAt + this.currentFile.projectFile.pomodoroSettings.workDuration * 60 * 1000
            : newState === "break"
              ? startedAt + this.currentFile.projectFile.pomodoroSettings.breakDuration * 60 * 1000
              : undefined,
      },
    };

    await this.notifySubscribers(this.currentFile);
  }

  async updateStateTransitions(transitions: Partial<StateTransition>): Promise<void> {
    if (!this.currentFile) return;

    this.currentFile = {
      ...this.currentFile,
      stateTransitions: { ...this.currentFile.stateTransitions, ...transitions },
    };

    await this.notifySubscribers(this.currentFile);
  }

  private async notifySubscribers(project: LoadedProjectState) {
    // Notify subscribers in sequence to avoid race conditions
    for (const listener of Array.from(this.changeListeners)) {
      await Promise.resolve(listener(project));
    }
  }
}
