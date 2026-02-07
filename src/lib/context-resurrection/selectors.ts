import type { ContextSnapshotV1, CrResult, TerminalContext } from "./types";

export const DEFAULT_RESURRECTION_THRESHOLD_MS = 60 * 60 * 1000; // 60 minutes

export type CardDisplayData = {
  snapshotId: string;
  taskId: string;
  taskTitle: string;
  capturedAt: string;
  capturedAtMs: number | null;
  captureReason: ContextSnapshotV1["capture_reason"];

  userNote?: string;

  terminal?: {
    sessionId: number;
    status: TerminalContext["status"];
    exitCode?: number;
    lastAttention?: TerminalContext["last_attention"];
    tailExcerpt?: string;
    tailPath?: string;
  };
};

function parseIsoToMs(iso: string): number | null {
  const ms = Date.parse(iso);
  return Number.isFinite(ms) ? ms : null;
}

export function tailToExcerpt(tail: string, maxLines = 20): string {
  const lines = tail.split(/\r?\n/);
  const excerpt = lines.slice(Math.max(0, lines.length - maxLines)).join("\n");
  return excerpt.trim();
}

/**
 * Decide whether the Resurrection Card should show.
 *
 * Contract (v1): show when a snapshot exists AND the user has been away longer
 * than the threshold (default 60 minutes).
 *
 * `lastActivityMs` is optional; when provided, it is treated as a hint for the
 * most-recent project activity time (and can suppress the card if recent).
 */
export function shouldShowCard(
  latestSnapshot: CrResult<ContextSnapshotV1 | null> | null | undefined,
  lastActivityMs?: number | null,
): boolean {
  if (!latestSnapshot) return false;
  if (!latestSnapshot.ok) return false;

  const snapshot = latestSnapshot.value;
  if (!snapshot) return false;

  const capturedAtMs = parseIsoToMs(snapshot.captured_at);
  if (capturedAtMs == null) return false;

  const activityMs = typeof lastActivityMs === "number" && Number.isFinite(lastActivityMs) ? lastActivityMs : null;
  const referenceMs = activityMs == null ? capturedAtMs : Math.max(capturedAtMs, activityMs);

  return Date.now() - referenceMs > DEFAULT_RESURRECTION_THRESHOLD_MS;
}

export function selectCardData(snapshot: ContextSnapshotV1): CardDisplayData {
  const capturedAtMs = parseIsoToMs(snapshot.captured_at);

  const data: CardDisplayData = {
    snapshotId: snapshot.id,
    taskId: snapshot.task_id,
    taskTitle: snapshot.task_title_at_capture,
    capturedAt: snapshot.captured_at,
    capturedAtMs,
    captureReason: snapshot.capture_reason,
    userNote: snapshot.user_note,
  };

  const t = snapshot.terminal;
  if (!t) return data;

  data.terminal = {
    sessionId: t.session_id,
    status: t.status,
    exitCode: t.exit_code,
    lastAttention: t.last_attention,
    tailExcerpt: t.tail_inline ? tailToExcerpt(t.tail_inline) : undefined,
    tailPath: t.tail_path,
  };

  return data;
}
