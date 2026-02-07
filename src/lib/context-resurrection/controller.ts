/**
 * Context Resurrection Controller
 *
 * Pure orchestration logic (no React imports) for CR state + actions.
 * Implements cancellation semantics: last-call-wins per task ID.
 */

import { SessionClient } from "../SessionClient";
import type { CrClient } from "./client";
import { forgetProjectContext, forgetTaskContext } from "./forget";
import { loadResurrectionState } from "./load";
import { saveNoteSnapshot } from "./note";
import type { ContextSnapshotV1 } from "./types";

export type CrControllerState = {
  daemonUnavailable: boolean;
  taskHasContext: Record<string, boolean>;
  cardSnapshot: ContextSnapshotV1 | null;
  cardPinned: boolean;
  dismissedSnapshotId: string | null;
};

export type LoadParams = {
  projectPath: string;
  activeTaskId?: string | null;
  tasks: Array<{ taskId?: string | null }>;
};

export type CrControllerDeps = {
  crClient: CrClient;
  sessionClient: SessionClient;
  onStateChange: (state: CrControllerState) => void;
  onActiveTaskChange?: (taskId: string) => Promise<void>;
};

/**
 * Pure CR controller with explicit cancellation semantics.
 * Thread-safe for single-threaded JS: last-call-wins per task.
 */
export class CrController {
  private state: CrControllerState = {
    daemonUnavailable: false,
    taskHasContext: {},
    cardSnapshot: null,
    cardPinned: false,
    dismissedSnapshotId: null,
  };

  // Track in-flight load requests per task id (last-call-wins).
  private loadAbortControllers = new Map<string, AbortController>();

  constructor(private deps: CrControllerDeps) {}

  getState(): Readonly<CrControllerState> {
    return this.state;
  }

  /**
   * Reset ephemeral state (call when switching projects).
   */
  resetForProject(): void {
    // Cancel all in-flight requests.
    for (const controller of this.loadAbortControllers.values()) {
      controller.abort();
    }
    this.loadAbortControllers.clear();

    this.setState({
      daemonUnavailable: false,
      taskHasContext: {},
      cardSnapshot: null,
      cardPinned: false,
      dismissedSnapshotId: null,
    });
  }

  /**
   * Load context resurrection state for the current project/task.
   * Cancels any previous load for the same task (last-call-wins).
   */
  async load(params: LoadParams): Promise<void> {
    const requestKey = `${params.projectPath}:${params.activeTaskId ?? ""}`;

    // Cancel previous request for this task (if any).
    const prev = this.loadAbortControllers.get(requestKey);
    if (prev) {
      prev.abort();
    }

    const controller = new AbortController();
    this.loadAbortControllers.set(requestKey, controller);

    try {
      const result = await loadResurrectionState({
        client: this.deps.crClient,
        projectPath: params.projectPath,
        activeTaskId: params.activeTaskId,
        tasks: params.tasks,
      });

      // If this request was cancelled, ignore results.
      if (controller.signal.aborted) {
        return;
      }

      this.setState({
        ...this.state,
        daemonUnavailable: result.daemonUnavailable,
        taskHasContext: result.taskHasContext,
      });

      // Only update snapshot if card is not pinned.
      if (!this.state.cardPinned) {
        const nextSnapshot = result.selected?.snapshot ?? null;
        if (nextSnapshot && nextSnapshot.id === this.state.dismissedSnapshotId) {
          this.setState({ ...this.state, cardSnapshot: null });
        } else {
          this.setState({ ...this.state, cardSnapshot: nextSnapshot });
        }
      }
    } catch (error) {
      if (controller.signal.aborted) {
        return;
      }
      console.error("Failed to load resurrection state:", error);
    } finally {
      this.loadAbortControllers.delete(requestKey);
    }
  }

  /**
   * Open resurrection card for a specific task.
   * Pins the card and loads the latest snapshot.
   */
  async openForTask(projectPath: string, taskId: string): Promise<void> {
    // Pin the card immediately to prevent auto-load from replacing it.
    this.setState({ ...this.state, cardPinned: true });

    // Update the active-task pointer (best effort).
    if (this.deps.onActiveTaskChange) {
      await this.deps.onActiveTaskChange(taskId);
    }

    const latest = await this.deps.crClient.latest(projectPath, taskId);

    if (!latest.ok) {
      if (latest.error.type === "daemon_unavailable") {
        this.setState({ ...this.state, daemonUnavailable: true });
        throw new Error("Context Resurrection is unavailable (daemon not running)");
      }
      throw new Error(
        `Failed to load snapshot: ${"message" in latest.error ? latest.error.message : latest.error.type}`,
      );
    }

    if (!latest.value) {
      this.setState({ ...this.state, cardSnapshot: null });
      throw new Error("No snapshots found for this task");
    }

    this.setState({
      ...this.state,
      cardSnapshot: latest.value,
      dismissedSnapshotId: null,
    });
  }

  /**
   * Resume from a snapshot: start or continue the session.
   */
  async resume(projectPath: string, snapshot: ContextSnapshotV1): Promise<void> {
    // Ensure the active task pointer is set.
    if (this.deps.onActiveTaskChange) {
      await this.deps.onActiveTaskChange(snapshot.task_id);
    }

    try {
      const terminal = snapshot.terminal;

      // If the session is still running/waiting, attach/continue.
      if (terminal && terminal.status !== "Stopped") {
        const result = await this.deps.sessionClient.continueSession(terminal.session_id, 512);
        const tail = SessionClient.tailBytesToString(result.tail);
        if (tail) {
          alert(`Session output (last 512 bytes):\n\n${tail}`);
        } else {
          alert(`Session ${terminal.session_id} continued (no recent output)`);
        }
      } else {
        // Otherwise, start a new session. Use task_id as the key so the daemon matches by stable ID.
        await this.deps.sessionClient.startSession(snapshot.task_id, projectPath, snapshot.task_id);
      }

      // Hide the card after resuming.
      this.setState({
        ...this.state,
        cardSnapshot: null,
        cardPinned: false,
      });
    } catch (error) {
      console.error("Failed to resume from snapshot:", error);
      throw error;
    }
  }

  /**
   * Save a note (capture now) for the current snapshot's task.
   */
  async saveNote(projectPath: string, note: string): Promise<void> {
    if (!this.state.cardSnapshot) {
      throw new Error("No snapshot to save note for");
    }

    const result = await saveNoteSnapshot(this.deps.crClient, projectPath, this.state.cardSnapshot.task_id, note);

    if (!result.ok) {
      if (result.error.type === "daemon_unavailable") {
        this.setState({ ...this.state, daemonUnavailable: true });
      }
      throw new Error(result.error.message ?? result.error.type);
    }

    // Update UI state immediately (snapshot store changes are not watched by the UI).
    this.setState({
      ...this.state,
      taskHasContext: {
        ...this.state.taskHasContext,
        [result.value.task_id]: true,
      },
      cardPinned: true,
      dismissedSnapshotId: null,
      cardSnapshot: result.value,
    });
  }

  /**
   * Dismiss the resurrection card (remember dismissed snapshot ID).
   */
  dismissCard(): void {
    const dismissedId = this.state.cardSnapshot?.id ?? null;
    this.setState({
      ...this.state,
      cardSnapshot: null,
      cardPinned: false,
      dismissedSnapshotId: dismissedId,
    });
  }

  /**
   * Forget (delete) context for the current task.
   */
  async forgetTask(projectPath: string): Promise<number> {
    if (!this.state.cardSnapshot) {
      throw new Error("No snapshot to forget");
    }

    const taskId = this.state.cardSnapshot.task_id;
    const taskTitle = this.state.cardSnapshot.task_title_at_capture;

    const confirmed = confirm(`Forget this task's context?\n\nThis deletes all stored snapshots for:\n${taskTitle}`);
    if (!confirmed) {
      return 0;
    }

    const result = await forgetTaskContext(this.deps.crClient, projectPath, taskId, this.state.taskHasContext);

    if (!result.ok) {
      if (result.error.type === "daemon_unavailable") {
        this.setState({ ...this.state, daemonUnavailable: true });
      }
      throw new Error(result.error.message ?? result.error.type);
    }

    this.setState({
      ...this.state,
      taskHasContext: result.value.next,
      cardSnapshot: null,
      cardPinned: false,
      dismissedSnapshotId: null,
    });

    return result.value.deletedCount;
  }

  /**
   * Forget (delete) context for the entire project.
   */
  async forgetProject(projectPath: string): Promise<number> {
    const confirmed = confirm("Forget project context?\n\nThis deletes ALL stored snapshots for this project.");
    if (!confirmed) {
      return 0;
    }

    const result = await forgetProjectContext(this.deps.crClient, projectPath);

    if (!result.ok) {
      if (result.error.type === "daemon_unavailable") {
        this.setState({ ...this.state, daemonUnavailable: true });
      }
      throw new Error(result.error.message ?? result.error.type);
    }

    this.setState({
      ...this.state,
      taskHasContext: result.value.next,
      cardSnapshot: null,
      cardPinned: false,
      dismissedSnapshotId: null,
    });

    return result.value.deletedCount;
  }

  private setState(newState: CrControllerState): void {
    this.state = newState;
    this.deps.onStateChange(newState);
  }
}
