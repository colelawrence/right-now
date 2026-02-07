import { shouldShowCard } from "./selectors";
import type { ContextSnapshotV1, CrResult } from "./types";

export type CrLatestClient = {
  latest: (projectPath: string, taskId?: string) => Promise<CrResult<ContextSnapshotV1 | null>>;
};

export type LoadResurrectionStateInput = {
  client: CrLatestClient;
  projectPath: string;
  activeTaskId?: string | null;
  lastActivityMs?: number | null;
  tasks: Array<{ taskId?: string | null }>;
};

export type LoadResurrectionStateOutput = {
  daemonUnavailable: boolean;
  taskHasContext: Record<string, boolean>;
  latestByTaskId: Record<string, ContextSnapshotV1 | null>;
  selected: { taskId: string; snapshot: ContextSnapshotV1 } | null;
};

function snapshotTimeMs(snapshot: ContextSnapshotV1): number | null {
  const ms = Date.parse(snapshot.captured_at);
  return Number.isFinite(ms) ? ms : null;
}

export async function loadResurrectionState(input: LoadResurrectionStateInput): Promise<LoadResurrectionStateOutput> {
  const taskIds = Array.from(
    new Set(
      input.tasks.map((t) => t.taskId).filter((id): id is string => typeof id === "string" && id.trim().length > 0),
    ),
  );

  if (taskIds.length === 0) {
    return {
      daemonUnavailable: false,
      taskHasContext: {},
      latestByTaskId: {},
      selected: null,
    };
  }

  const results = await Promise.all(
    taskIds.map(async (taskId) => ({ taskId, res: await input.client.latest(input.projectPath, taskId) })),
  );

  // If daemon is unavailable, bail out entirely (UI should degrade gracefully).
  if (results.some((r) => !r.res.ok && r.res.error.type === "daemon_unavailable")) {
    return {
      daemonUnavailable: true,
      taskHasContext: {},
      latestByTaskId: {},
      selected: null,
    };
  }

  const latestByTaskId: Record<string, ContextSnapshotV1 | null> = {};
  const taskHasContext: Record<string, boolean> = {};

  for (const { taskId, res } of results) {
    if (res.ok) {
      latestByTaskId[taskId] = res.value;
      taskHasContext[taskId] = res.value != null;
    } else {
      // Non-daemon-unavailable errors are treated as "no context".
      latestByTaskId[taskId] = null;
      taskHasContext[taskId] = false;
    }
  }

  const active =
    typeof input.activeTaskId === "string" && input.activeTaskId.trim().length > 0 ? input.activeTaskId : null;

  let candidate: ContextSnapshotV1 | null = null;

  if (active) {
    candidate = latestByTaskId[active] ?? null;
  }

  if (!candidate) {
    // Fallback: choose the most recent snapshot across all tasks.
    for (const snap of Object.values(latestByTaskId)) {
      if (!snap) continue;
      if (!candidate) {
        candidate = snap;
        continue;
      }
      const a = snapshotTimeMs(snap);
      const b = snapshotTimeMs(candidate);
      if (a != null && b != null && a > b) {
        candidate = snap;
      }
    }
  }

  if (!candidate) {
    return {
      daemonUnavailable: false,
      taskHasContext,
      latestByTaskId,
      selected: null,
    };
  }

  const eligible = shouldShowCard({ ok: true, value: candidate }, input.lastActivityMs);

  return {
    daemonUnavailable: false,
    taskHasContext,
    latestByTaskId,
    selected: eligible ? { taskId: candidate.task_id, snapshot: candidate } : null,
  };
}
