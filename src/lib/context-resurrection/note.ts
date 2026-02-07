import { type ContextSnapshotV1, CrResult, type CrResult as CrResultType } from "./types";

export type CrNoteClient = {
  captureNow: (
    projectPath: string,
    taskId: string,
    userNote?: string,
  ) => Promise<CrResultType<ContextSnapshotV1 | null>>;
};

/**
 * Save a "note to future self" by creating a new manual snapshot with user_note set.
 *
 * This is a thin helper that converts the `captureNow()` contract into a stricter
 * "must return a snapshot" result.
 */
export async function saveNoteSnapshot(
  client: CrNoteClient,
  projectPath: string,
  taskId: string,
  note: string,
): Promise<CrResultType<ContextSnapshotV1>> {
  const trimmed = note.trim();
  if (!trimmed) {
    return CrResult.err({ type: "daemon_error", message: "Note must not be empty" });
  }

  const res = await client.captureNow(projectPath, taskId, trimmed);

  if (!res.ok) {
    return CrResult.err(res.error);
  }

  if (!res.value) {
    return CrResult.err({ type: "skipped", message: "Capture was skipped (dedup/rate-limit)" });
  }

  return CrResult.ok(res.value);
}
