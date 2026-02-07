import { CrResult, type CrResult as CrResultType } from "./types";

export type CrForgetClient = {
  deleteTask: (projectPath: string, taskId: string) => Promise<CrResultType<number>>;
  deleteProject: (projectPath: string) => Promise<CrResultType<number>>;
};

export type TaskHasContext = Record<string, boolean>;

export async function forgetTaskContext(
  client: CrForgetClient,
  projectPath: string,
  taskId: string,
  current: TaskHasContext,
): Promise<CrResultType<{ deletedCount: number; next: TaskHasContext }>> {
  const res = await client.deleteTask(projectPath, taskId);

  if (!res.ok) return CrResult.err(res.error);

  return CrResult.ok({
    deletedCount: res.value,
    next: { ...current, [taskId]: false },
  });
}

export async function forgetProjectContext(
  client: CrForgetClient,
  projectPath: string,
): Promise<CrResultType<{ deletedCount: number; next: TaskHasContext }>> {
  const res = await client.deleteProject(projectPath);

  if (!res.ok) return CrResult.err(res.error);

  return CrResult.ok({
    deletedCount: res.value,
    next: {},
  });
}
