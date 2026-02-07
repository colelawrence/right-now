/**
 * React hook wrapper for CrController.
 * Thin adapter layer that manages controller lifecycle + state subscription.
 */

import { useMemo, useRef, useState } from "react";
import type { SessionClient } from "../SessionClient";
import type { CrClient } from "./client";
import { CrController, type CrControllerDeps, type CrControllerState, type LoadParams } from "./controller";
import type { ContextSnapshotV1 } from "./types";

export type UseCrControllerParams = {
  crClient: CrClient;
  sessionClient: SessionClient;
  onActiveTaskChange?: (taskId: string) => Promise<void>;
};

export type UseCrControllerResult = {
  state: CrControllerState;
  actions: {
    load: (params: LoadParams) => Promise<void>;
    openForTask: (projectPath: string, taskId: string) => Promise<void>;
    resume: (projectPath: string, snapshot: ContextSnapshotV1) => Promise<void>;
    saveNote: (projectPath: string, note: string) => Promise<void>;
    dismissCard: () => void;
    forgetTask: (projectPath: string) => Promise<number>;
    forgetProject: (projectPath: string) => Promise<number>;
    resetForProject: () => void;
  };
};

/**
 * React hook that manages CR controller instance and subscribes to state changes.
 */
export function useCrController(params: UseCrControllerParams): UseCrControllerResult {
  const [state, setState] = useState<CrControllerState>(() => ({
    daemonUnavailable: false,
    taskHasContext: {},
    cardSnapshot: null,
    cardPinned: false,
    dismissedSnapshotId: null,
  }));

  // Store controller in a ref to keep it stable across renders.
  const controllerRef = useRef<CrController | null>(null);

  // Create controller on mount (or when deps change).
  if (!controllerRef.current) {
    const deps: CrControllerDeps = {
      crClient: params.crClient,
      sessionClient: params.sessionClient,
      onStateChange: setState,
      onActiveTaskChange: params.onActiveTaskChange,
    };
    controllerRef.current = new CrController(deps);
  }

  const controller = controllerRef.current;

  // Memoize actions to prevent unnecessary re-renders.
  const actions = useMemo(
    () => ({
      load: (p: LoadParams) => controller.load(p),
      openForTask: (projectPath: string, taskId: string) => controller.openForTask(projectPath, taskId),
      resume: (projectPath: string, snapshot: ContextSnapshotV1) => controller.resume(projectPath, snapshot),
      saveNote: (projectPath: string, note: string) => controller.saveNote(projectPath, note),
      dismissCard: () => controller.dismissCard(),
      forgetTask: (projectPath: string) => controller.forgetTask(projectPath),
      forgetProject: (projectPath: string) => controller.forgetProject(projectPath),
      resetForProject: () => controller.resetForProject(),
    }),
    [controller],
  );

  return { state, actions };
}
