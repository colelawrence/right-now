// SessionClient - TypeScript bridge to right-now-daemon
// Provides methods to interact with terminal sessions via Tauri commands

import { invoke } from "@tauri-apps/api/core";

/**
 * Session status enum matching the Rust protocol
 */
export enum SessionStatus {
  Running = "Running",
  Waiting = "Waiting",
  Stopped = "Stopped",
}

/**
 * Attention type enum
 */
export enum AttentionType {
  InputRequired = "input_required",
  DecisionPoint = "decision_point",
  Completed = "completed",
  Error = "error",
}

/**
 * Attention summary
 */
export interface AttentionSummary {
  profile: string;
  attention_type: AttentionType;
  preview: string;
  triggered_at: string; // ISO 8601 timestamp
}

/**
 * Session metadata
 */
export interface Session {
  id: number;
  task_key: string;
  task_id?: string; // Stable task ID from TODO.md (e.g., "abc.derived-label")
  project_path: string;
  status: SessionStatus;
  pty_pid?: number;
  shell_command?: string[];
  created_at: string; // ISO 8601 timestamp
  updated_at: string; // ISO 8601 timestamp
  exit_code?: number;
  last_attention?: AttentionSummary;
}

/**
 * Result from continue_session
 */
export interface ContinueSessionResult {
  session: Session;
  tail?: number[]; // Raw bytes as array of numbers
}

/**
 * SessionClient provides methods to interact with the right-now-daemon
 */
export class SessionClient {
  /**
   * List all sessions, optionally filtered by project path
   */
  async listSessions(projectPath?: string): Promise<Session[]> {
    try {
      const sessions = await invoke<Session[]>("session_list", {
        projectPath: projectPath ?? null,
      });
      return sessions;
    } catch (error) {
      console.error("Failed to list sessions:", error);
      throw new Error(`Failed to list sessions: ${error}`);
    }
  }

  /**
   * Start a new session for a task
   */
  async startSession(taskKey: string, projectPath: string, taskId?: string, shell?: string[]): Promise<Session> {
    try {
      const session = await invoke<Session>("session_start", {
        taskKey,
        taskId: taskId ?? null,
        projectPath,
        shell: shell ?? null,
      });
      console.log(`Session started: ${session.id} (${session.task_key})`);
      return session;
    } catch (error) {
      console.error("Failed to start session:", error);
      throw new Error(`Failed to start session: ${error}`);
    }
  }

  /**
   * Stop a running session
   */
  async stopSession(sessionId: number): Promise<Session> {
    try {
      const session = await invoke<Session>("session_stop", {
        sessionId,
      });
      console.log(`Session stopped: ${sessionId}`);
      return session;
    } catch (error) {
      console.error("Failed to stop session:", error);
      throw new Error(`Failed to stop session: ${error}`);
    }
  }

  /**
   * Continue/attach to an existing session
   */
  async continueSession(sessionId: number, tailBytes?: number): Promise<ContinueSessionResult> {
    try {
      const result = await invoke<ContinueSessionResult>("session_continue", {
        sessionId,
        tailBytes: tailBytes ?? null,
      });
      console.log(`Session continued: ${sessionId}`);
      return result;
    } catch (error) {
      console.error("Failed to continue session:", error);
      throw new Error(`Failed to continue session: ${error}`);
    }
  }

  /**
   * Helper: Convert tail bytes to UTF-8 string (best-effort)
   */
  static tailBytesToString(bytes?: number[]): string {
    if (!bytes || bytes.length === 0) {
      return "";
    }
    try {
      const uint8Array = new Uint8Array(bytes);
      return new TextDecoder("utf-8", { fatal: false }).decode(uint8Array);
    } catch (error) {
      console.warn("Failed to decode tail bytes as UTF-8:", error);
      return "";
    }
  }

  /**
   * Helper: Get the deep link URL for a session
   */
  static sessionDeepLink(sessionId: number): string {
    return `todos://session/${sessionId}`;
  }
}

// Export a singleton instance for convenience
export const sessionClient = new SessionClient();
