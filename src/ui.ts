import type { Atom, WritableAtom } from "jotai";

export interface TaskController {
  titleAtom: WritableAtom<string, [string], void>;
  completeAtom: WritableAtom<boolean, [boolean], void>;
}

export interface WindowTaskSummary {
  completedTasks: TaskController[];
  currentHeading: string | null;
  currentTask: TaskController | null;
  nextTasks: TaskController[];
}

/**
 * Provides a handle to the compact/expanded state of
 * a window and makes toggling easier.
 */
export interface WindowController {
  isCompactAtom: Atom<boolean>;
  toggleCompact(force?: boolean): void;
}

/**
 * Specifies the data needed by the window to render tasks,
 * primarily the entire TaskSummary as a reactive value.
 */
export interface WindowOptions {
  taskSummaryAtom: Atom<WindowTaskSummary>;
}

/**
 * Describes a single sound pack. You might parallel this
 * with your @sounds.ts logic, so each sound pack can have
 * its own name, whether it's active, etc.
 */
export interface SoundPackController {
  nameAtom: Atom<string>;
  isDefaultAtom: WritableAtom<boolean, [boolean], void>;
  // FUTURE: Support saving the pack to the user's system
  // saveAs(): Promise<void>;
}

/**
 * Options for configuring a sound manager at initialization:
 * - defaultSoundPackPath: e.g. your baked-in path if there's no user preference
 * - soundPackDirectory: top-level directory for user packs
 * - projectSoundPackIdAtom: reactive ID for whichever pack is in use
 *   for the current project file.
 */
export interface SoundManagerOptions {
  defaultSoundPackPath: string;
  soundPackDirectory: string;
  projectSoundPackIdAtom: WritableAtom<string | null, [string | null], void>;
}
