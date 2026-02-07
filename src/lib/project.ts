import { path } from "@tauri-apps/api";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { type DirEntry, readDir, readTextFile, rename, stat, writeTextFile } from "@tauri-apps/plugin-fs";
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

  /** Serialize state/file operations to avoid races between watcher reloads and user actions. */
  private opChain: Promise<void> = Promise.resolve();

  /** Incremented whenever a new project load is requested. Used to ignore stale reloads. */
  private loadToken = 0;

  constructor(projectStore: ProjectStore, clock: Clock = realClock) {
    this.projectStore = projectStore;
    this.clock = clock;
    this.watcher = new FileWatcher();
  }

  private enqueueOp<T>(op: () => Promise<T>): Promise<T> {
    const next = this.opChain.then(op, op);
    this.opChain = next.then(
      () => undefined,
      () => undefined,
    );
    return next;
  }

  async openProject(defaultProject?: string) {
    // Try file picker first (more direct for TODO.md selection)
    const selected = await open({
      multiple: false,
      title: "Select TODO file",
      defaultPath: defaultProject,
      directory: false,
      filters: [
        {
          name: "Markdown",
          extensions: ["md"],
        },
      ],
    });

    if (selected && !Array.isArray(selected)) {
      await this.handleFolderOrFile(selected).catch(withError(`Failed to handle selection (${selected})`));
    }
  }

  async openProjectFolder(defaultProject?: string) {
    const selected = await open({
      multiple: false,
      title: "Select project folder",
      defaultPath: defaultProject,
      directory: true,
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

    // Bump token and stop the previous watcher *immediately* to prevent old files from "winning" later.
    const token = ++this.loadToken;
    this.watcher.cleanup();

    return await this.enqueueOp(async () => {
      if (token !== this.loadToken) return;

      const reloadImpl = async () => {
        if (token !== this.loadToken) return;

        const textContent = await readTextFile(fullPath).catch(
          withError((err) => `Error reading file (${fullPath}): ${JSON.stringify(err)}`, ProjectError),
        );

        if (token !== this.loadToken) return;

        if (this.currentFile?.fullPath === fullPath && this.currentFile.textContent === textContent) {
          return;
        }

        let projectFile: ProjectFile;
        try {
          projectFile = ProjectStateEditor.parse(textContent);
        } catch (error) {
          // This can happen while the user is mid-edit (invalid YAML/frontmatter, partial saves, etc).
          // Keep the last known-good state and try again on the next watcher event.
          console.error("Failed to parse project file", { fullPath, error });
          return;
        }

        if (token !== this.loadToken) return;

        // Restore workState/stateTransitions from file when valid; otherwise keep current in-memory state.
        const prev = this.currentFile?.fullPath === fullPath ? this.currentFile : undefined;

        const workStateFromFile = projectFile.workState;
        const workState: WorkState =
          workStateFromFile === "planning" || workStateFromFile === "working" || workStateFromFile === "break"
            ? workStateFromFile
            : (prev?.workState ?? "planning");

        const transitionsFromFile = projectFile.stateTransitions;
        const stateTransitions: StateTransition =
          transitionsFromFile && typeof transitionsFromFile.startedAt === "number"
            ? {
                startedAt: transitionsFromFile.startedAt,
                endsAt: typeof transitionsFromFile.endsAt === "number" ? transitionsFromFile.endsAt : undefined,
              }
            : (prev?.stateTransitions ?? {
                startedAt: this.clock.now(),
              });

        if (this.currentFile?.fullPath === fullPath) {
          this.currentFile = {
            ...this.currentFile,
            textContent,
            projectFile,
            workState,
            stateTransitions,
          };
        } else {
          this.currentFile = {
            fullPath,
            textContent,
            projectFile,
            virtual: type === "virtual",
            workState,
            stateTransitions,
          };
        }

        await this.notifySubscribers(this.currentFile);
      };

      // Initial load
      await reloadImpl();

      if (token !== this.loadToken) return;

      // Update recent projects in store
      await this.projectStore.addRecentProject(fullPath);

      if (token !== this.loadToken) return;

      // Set up file watcher for external changes.
      // Queue reloads so they don't race with other state transitions/writes.
      await this.watcher.watchProject(fullPath, () => this.enqueueOp(reloadImpl));

      // If a different project load was requested while setting up the watcher, undo it.
      if (token !== this.loadToken) {
        this.watcher.cleanup();
        return;
      }

      if (type === "absolute") {
        // Record the current project path for CLI fallbacks
        try {
          await invoke("set_current_project_path", { path: fullPath });
        } catch (error) {
          console.warn("Failed to record current project path", error);
        }
      }
    });
  }

  subscribe(callback: ProjectChangeCallback): () => void {
    callback(this.currentFile);
    this.changeListeners.add(callback);
    return () => void this.changeListeners.delete(callback);
  }

  async updateProject(fn: (project: ProjectFile) => void | boolean) {
    return await this.enqueueOp(() => this.updateProjectImpl(fn));
  }

  /**
   * Core update logic for mutating and persisting the project file.
   *
   * IMPORTANT: This must not call enqueueOp() internally.
   * - Use updateProject() to run it through the opChain.
   * - Or call it directly from within an already-enqueued op.
   */
  private async updateProjectImpl(fn: (project: ProjectFile) => void | boolean): Promise<void> {
    if (!this.currentFile) return;

    const fullPath = this.currentFile.fullPath;
    const token = this.loadToken;

    // Always read fresh before writing to avoid clobbering edits from the user, editor, or daemon.
    const freshContent = await readTextFile(fullPath).catch(
      withError((err) => `Error reading file (${fullPath}): ${JSON.stringify(err)}`, ProjectError),
    );

    // If a different project load was requested mid-operation, bail.
    if (token !== this.loadToken) return;

    let freshProjectFile: ProjectFile;
    try {
      freshProjectFile = ProjectStateEditor.parse(freshContent);
    } catch (error) {
      // The file might be mid-edit (invalid YAML/frontmatter, partial saves, etc).
      // Avoid clobbering it; wait for the next reload to restore a valid state.
      console.error("Failed to parse fresh project file before write", { fullPath, error });
      return;
    }

    const draft = structuredClone(freshProjectFile);

    if (fn(draft) === false) return;

    let updatedContent: string;
    try {
      updatedContent = ProjectStateEditor.update(freshContent, draft);
    } catch (error) {
      console.error("Failed to update project file content", { fullPath, error });
      return;
    }

    // No-op write: still refresh in-memory content if it drifted.
    if (updatedContent === freshContent) {
      if (this.currentFile?.fullPath === fullPath && this.currentFile.textContent !== freshContent) {
        this.currentFile = {
          ...this.currentFile,
          textContent: freshContent,
          projectFile: freshProjectFile,
        };
        await this.notifySubscribers(this.currentFile);
      }
      return;
    }

    await this.writeTextFileAtomic(fullPath, updatedContent);

    // Update in-memory state to match what we just wrote (prevents a redundant watcher reload).
    if (token !== this.loadToken) return;
    if (!this.currentFile || this.currentFile.fullPath !== fullPath) return;

    let projectFile: ProjectFile;
    try {
      projectFile = ProjectStateEditor.parse(updatedContent);
    } catch {
      // Shouldn't happen since we generated the content, but be defensive.
      projectFile = draft;
    }

    this.currentFile = {
      ...this.currentFile,
      textContent: updatedContent,
      projectFile,
    };

    await this.notifySubscribers(this.currentFile);
  }

  async updateWorkState(newState: WorkState): Promise<void> {
    return await this.enqueueOp(async () => {
      if (!this.currentFile) return;

      // Only update if state is actually changing
      if (this.currentFile.workState === newState) return;

      const startedAt = this.clock.now();
      const stateTransitions = {
        startedAt,
        endsAt:
          newState === "working"
            ? startedAt + this.currentFile.projectFile.pomodoroSettings.workDuration * 60 * 1000
            : newState === "break"
              ? startedAt + this.currentFile.projectFile.pomodoroSettings.breakDuration * 60 * 1000
              : undefined,
      };

      // Update in-memory state
      this.currentFile = {
        ...this.currentFile,
        workState: newState,
        stateTransitions,
      };

      await this.notifySubscribers(this.currentFile);

      // Persist to file
      await this.updateProjectImpl((draft) => {
        draft.workState = newState;
        draft.stateTransitions = stateTransitions;
      });
    });
  }

  async updateStateTransitions(transitions: Partial<StateTransition>): Promise<void> {
    return await this.enqueueOp(async () => {
      if (!this.currentFile) return;

      const newTransitions = { ...this.currentFile.stateTransitions, ...transitions };

      // Update in-memory state
      this.currentFile = {
        ...this.currentFile,
        stateTransitions: newTransitions,
      };

      await this.notifySubscribers(this.currentFile);

      // Persist to file
      await this.updateProjectImpl((draft) => {
        draft.stateTransitions = newTransitions;
      });
    });
  }

  async setActiveTask(taskId: string | undefined): Promise<void> {
    return await this.enqueueOp(async () => {
      if (!this.currentFile) return;

      // Update in-memory state
      this.currentFile = {
        ...this.currentFile,
        projectFile: {
          ...this.currentFile.projectFile,
          activeTaskId: taskId,
        },
      };

      await this.notifySubscribers(this.currentFile);

      // Persist to file
      await this.updateProjectImpl((draft) => {
        draft.activeTaskId = taskId;
      });
    });
  }

  /**
   * Move a heading section (heading + all content until next heading) up or down.
   * Uses ProjectStateEditor.moveHeadingSection() under the hood.
   */
  async moveHeadingSection(headingIndex: number, direction: "up" | "down"): Promise<void> {
    return await this.enqueueOp(async () => {
      if (!this.currentFile) return;

      const fullPath = this.currentFile.fullPath;
      const token = this.loadToken;

      // Read fresh content to avoid clobbering edits
      const freshContent = await readTextFile(fullPath).catch(
        withError((err) => `Error reading file (${fullPath}): ${JSON.stringify(err)}`, ProjectError),
      );

      if (token !== this.loadToken) return;

      // Attempt the move
      const updatedContent = ProjectStateEditor.moveHeadingSection(freshContent, headingIndex, direction);

      // If null, the move was invalid (e.g., already at boundary)
      if (!updatedContent) return;

      // No change means no-op
      if (updatedContent === freshContent) return;

      await this.writeTextFileAtomic(fullPath, updatedContent);

      // Update in-memory state
      if (token !== this.loadToken) return;
      if (!this.currentFile || this.currentFile.fullPath !== fullPath) return;

      let projectFile: ProjectFile;
      try {
        projectFile = ProjectStateEditor.parse(updatedContent);
      } catch (error) {
        console.error("Failed to parse project file after section move", { fullPath, error });
        return;
      }

      this.currentFile = {
        ...this.currentFile,
        textContent: updatedContent,
        projectFile,
      };

      await this.notifySubscribers(this.currentFile);
    });
  }

  private async notifySubscribers(project: LoadedProjectState) {
    // Notify subscribers in sequence to avoid race conditions
    for (const listener of Array.from(this.changeListeners)) {
      await Promise.resolve(listener(project));
    }
  }

  /**
   * Atomic write (write-to-temp + rename) to avoid partial reads and reduce watcher storms.
   */
  private async writeTextFileAtomic(targetPath: string, contents: string): Promise<void> {
    const dir = await path.dirname(targetPath);
    const base = await path.basename(targetPath);
    const tmpName = `.${base}.tmp.${Date.now()}.${Math.random().toString(16).slice(2)}`;
    const tmpPath = await path.join(dir, tmpName);

    await writeTextFile(tmpPath, contents);
    await rename(tmpPath, targetPath);
  }
}
