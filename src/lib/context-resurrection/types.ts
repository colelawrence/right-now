// Context Resurrection (CR) protocol + snapshot schema types
// Mirrors Rust daemon protocol in `src-tauri/src/session/protocol.rs` and
// snapshot schema in `src-tauri/src/context_resurrection/models.rs`.

// ---------------------------------------------------------------------------
// Snapshot schema (ContextSnapshotV1)
// ---------------------------------------------------------------------------

export type SnapshotId = string;

export type CaptureReason = "session_stopped" | "session_waiting" | "session_running" | "idle_timeout" | "manual";

// Note: Rust `SessionStatus` for CR serializes as the enum variant name.
export type CrSessionStatus = "Running" | "Waiting" | "Stopped";

export type CrAttentionType = "input_required" | "decision_point" | "completed" | "error";

export type CrAttentionSummary = {
  attention_type: CrAttentionType;
  preview: string;
  triggered_at: string; // ISO8601
};

export type TerminalContext = {
  session_id: number;
  status: CrSessionStatus;
  exit_code?: number;
  last_attention?: CrAttentionSummary;
  tail_inline?: string;
  tail_path?: string;
};

export type ContextSnapshotV1 = {
  id: SnapshotId;
  version: number;

  project_path: string;
  task_id: string;
  task_title_at_capture: string;

  captured_at: string; // ISO8601
  capture_reason: CaptureReason;

  terminal?: TerminalContext;
  user_note?: string;
  editor?: unknown;
};

// ---------------------------------------------------------------------------
// Daemon protocol (CR subset)
// ---------------------------------------------------------------------------

export type CrLatestRequest = {
  type: "cr_latest";
  project_path: string;
  task_id?: string;
};

export type CrListRequest = {
  type: "cr_list";
  project_path: string;
  task_id: string;
  limit?: number;
};

export type CrGetRequest = {
  type: "cr_get";
  project_path: string;
  task_id: string;
  snapshot_id: string;
};

export type CrCaptureNowRequest = {
  type: "cr_capture_now";
  project_path: string;
  task_id: string;
  user_note?: string;
};

export type CrDeleteTaskRequest = {
  type: "cr_delete_task";
  project_path: string;
  task_id: string;
};

export type CrDeleteProjectRequest = {
  type: "cr_delete_project";
  project_path: string;
};

export type CrDaemonRequest =
  | CrLatestRequest
  | CrListRequest
  | CrGetRequest
  | CrCaptureNowRequest
  | CrDeleteTaskRequest
  | CrDeleteProjectRequest;

export type CrSnapshotResponse = {
  type: "cr_snapshot";
  snapshot: ContextSnapshotV1 | null;
};

export type CrSnapshotsResponse = {
  type: "cr_snapshots";
  snapshots: ContextSnapshotV1[];
};

export type CrDeletedResponse = {
  type: "cr_deleted";
  deleted_count: number;
};

export type DaemonErrorCode =
  | "not_found"
  | "skipped"
  | "invalid_request"
  | "store_unavailable"
  | "internal"
  | "daemon_unavailable"
  | "timeout";

export type DaemonErrorResponse = {
  type: "error";
  code: DaemonErrorCode;
  message: string;
};

export type CrDaemonResponse = CrSnapshotResponse | CrSnapshotsResponse | CrDeletedResponse | DaemonErrorResponse;

// ---------------------------------------------------------------------------
// Client-facing Result + Error types
// ---------------------------------------------------------------------------

export type CrError =
  | { type: "daemon_unavailable"; message?: string }
  | { type: "not_found"; message?: string }
  | { type: "skipped"; message?: string }
  | { type: "daemon_error"; message: string };

export type CrResult<T> = { ok: true; value: T } | { ok: false; error: CrError };

export const CrResult = {
  ok<T>(value: T): CrResult<T> {
    return { ok: true, value };
  },
  err<T = never>(error: CrError): CrResult<T> {
    return { ok: false, error };
  },
};
