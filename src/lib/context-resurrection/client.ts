import {
  type ContextSnapshotV1,
  type CrDaemonRequest,
  type CrDaemonResponse,
  type CrError,
  CrResult,
  type DaemonErrorCode,
} from "./types";

export type CrTransport = (request: CrDaemonRequest) => Promise<CrDaemonResponse>;

function errorFromDaemonResponse(code: DaemonErrorCode, message: string): CrError {
  switch (code) {
    case "not_found":
      return { type: "not_found", message };
    case "skipped":
      return { type: "skipped", message };
    case "daemon_unavailable":
    case "timeout":
      return { type: "daemon_unavailable", message };
    default:
      return { type: "daemon_error", message };
  }
}

export class CrClient {
  constructor(private transport: CrTransport) {}

  async latest(projectPath: string, taskId?: string): Promise<CrResult<ContextSnapshotV1 | null>> {
    try {
      const res = await this.transport({
        type: "cr_latest",
        project_path: projectPath,
        task_id: taskId,
      });

      if (res.type === "cr_snapshot") {
        return CrResult.ok(res.snapshot);
      }

      if (res.type === "error") {
        const err = errorFromDaemonResponse(res.code, res.message);
        // For latest(), not_found and skipped are not exceptional: treat as "no snapshot".
        if (err.type === "not_found") return CrResult.ok(null);
        return CrResult.err(err);
      }

      return CrResult.err({
        type: "daemon_error",
        message: `Unexpected response type: ${res.type}`,
      });
    } catch (error) {
      return CrResult.err({
        type: "daemon_unavailable",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async list(projectPath: string, taskId: string, limit?: number): Promise<CrResult<ContextSnapshotV1[]>> {
    try {
      const res = await this.transport({
        type: "cr_list",
        project_path: projectPath,
        task_id: taskId,
        limit,
      });

      if (res.type === "cr_snapshots") return CrResult.ok(res.snapshots);

      if (res.type === "error") return CrResult.err(errorFromDaemonResponse(res.code, res.message));

      return CrResult.err({
        type: "daemon_error",
        message: `Unexpected response type: ${res.type}`,
      });
    } catch (error) {
      return CrResult.err({
        type: "daemon_unavailable",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async get(projectPath: string, taskId: string, snapshotId: string): Promise<CrResult<ContextSnapshotV1 | null>> {
    try {
      const res = await this.transport({
        type: "cr_get",
        project_path: projectPath,
        task_id: taskId,
        snapshot_id: snapshotId,
      });

      if (res.type === "cr_snapshot") return CrResult.ok(res.snapshot);

      if (res.type === "error") {
        const err = errorFromDaemonResponse(res.code, res.message);
        if (err.type === "not_found") return CrResult.ok(null);
        return CrResult.err(err);
      }

      return CrResult.err({
        type: "daemon_error",
        message: `Unexpected response type: ${res.type}`,
      });
    } catch (error) {
      return CrResult.err({
        type: "daemon_unavailable",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async captureNow(
    projectPath: string,
    taskId: string,
    userNote?: string,
  ): Promise<CrResult<ContextSnapshotV1 | null>> {
    try {
      const res = await this.transport({
        type: "cr_capture_now",
        project_path: projectPath,
        task_id: taskId,
        user_note: userNote,
      });

      if (res.type === "cr_snapshot") return CrResult.ok(res.snapshot);

      if (res.type === "error") {
        const err = errorFromDaemonResponse(res.code, res.message);
        // For captureNow(), treat skipped as success with no snapshot.
        if (err.type === "skipped") return CrResult.ok(null);
        return CrResult.err(err);
      }

      return CrResult.err({
        type: "daemon_error",
        message: `Unexpected response type: ${res.type}`,
      });
    } catch (error) {
      return CrResult.err({
        type: "daemon_unavailable",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async deleteTask(projectPath: string, taskId: string): Promise<CrResult<number>> {
    try {
      const res = await this.transport({
        type: "cr_delete_task",
        project_path: projectPath,
        task_id: taskId,
      });

      if (res.type === "cr_deleted") return CrResult.ok(res.deleted_count);

      if (res.type === "error") return CrResult.err(errorFromDaemonResponse(res.code, res.message));

      return CrResult.err({
        type: "daemon_error",
        message: `Unexpected response type: ${res.type}`,
      });
    } catch (error) {
      return CrResult.err({
        type: "daemon_unavailable",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async deleteProject(projectPath: string): Promise<CrResult<number>> {
    try {
      const res = await this.transport({
        type: "cr_delete_project",
        project_path: projectPath,
      });

      if (res.type === "cr_deleted") return CrResult.ok(res.deleted_count);

      if (res.type === "error") return CrResult.err(errorFromDaemonResponse(res.code, res.message));

      return CrResult.err({
        type: "daemon_error",
        message: `Unexpected response type: ${res.type}`,
      });
    } catch (error) {
      return CrResult.err({
        type: "daemon_unavailable",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }
}
