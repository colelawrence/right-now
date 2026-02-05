import { IconCheck, IconClipboard, IconEdit, IconTerminal } from "@tabler/icons-react";
import { Window } from "@tauri-apps/api/window";
import { openPath } from "@tauri-apps/plugin-opener";
import { useAtom } from "jotai";
import { forwardRef, useEffect, useRef, useState } from "react";
import { SessionsDebugPanel } from "./components/SessionsDebugPanel";
import { StateControls } from "./components/StateControls";
import { TaskList } from "./components/TaskList";
import { Timer } from "./components/Timer";
import { Markdown, MarkdownProvider } from "./components/markdown";
import { type AppWindows, type ISoundManager, type ProjectManager, type ProjectStore, useDeepLink } from "./lib";
import type { ProjectMarkdown } from "./lib/ProjectStateEditor";
import type { Clock } from "./lib/clock";
import type { EventBus } from "./lib/events";
import type { LoadedProjectState, ProjectFile, WorkState } from "./lib/project";
import {
  type TimerState,
  computeStateChangeEvents,
  computeTaskCompletedEvents,
  computeTimerEvents,
} from "./lib/timer-logic";

interface AppControllers {
  projectManager: ProjectManager;
  appWindows: AppWindows;
  projectStore: ProjectStore;
  soundManager: ISoundManager;
  clock: Clock;
  eventBus: EventBus;
}

interface StartupWarning {
  message: string;
  details?: string;
}

interface AppProps {
  controllers?: AppControllers;
  startupError?: Error;
  startupWarning?: StartupWarning;
}

function AppOuter({ controllers, startupError, startupWarning }: AppProps) {
  // If we have a startup error, show error UI
  if (startupError) {
    return (
      <main className="h-screen flex items-center justify-center bg-red-100">
        <div className="px-6 py-4">
          <h1 className="text-lg font-semibold text-red-700 mb-2">Startup Error</h1>
          <div className="bg-red-50 px-3 py-2 rounded">
            <p className="text-red-700 font-mono text-sm whitespace-pre-wrap">{startupError.message}</p>
          </div>
          <button
            onClick={() => window.location.reload()}
            className="mt-3 px-3 py-1.5 bg-red-600 text-white text-sm rounded hover:bg-red-700 transition-colors"
          >
            Retry
          </button>
        </div>
      </main>
    );
  }

  // If we don't have controllers (but no error), show loading state
  if (!controllers) {
    return (
      <main className="h-screen flex items-center justify-center bg-gray-50">
        <div className="text-sm text-gray-600">Initializing...</div>
      </main>
    );
  }

  return <AppReady controllers={controllers} startupWarning={startupWarning} />;
}

function useLoadedProject(projectManager: ProjectManager) {
  const [loaded, setLoaded] = useState<LoadedProjectState>();
  useEffect(() => {
    const unsubscribe = projectManager.subscribe((newProject) => {
      // Ensure we're not setting state if it hasn't changed
      setLoaded((current) => {
        if (current === newProject) return current;
        return newProject;
      });
    });
    return unsubscribe;
  }, [projectManager]);
  return loaded;
}

function AppReady({ controllers, startupWarning }: { controllers: AppControllers; startupWarning?: StartupWarning }) {
  const { projectManager, appWindows, projectStore, soundManager, clock, eventBus } = controllers;
  const loaded = useLoadedProject(projectManager);
  const project = loaded?.projectFile;
  const [isCompact, setIsCompact] = useAtom(appWindows.currentlyMiniAtom);
  const timerStateRef = useRef<Partial<TimerState>>({});

  // Get the directory containing the project file for resolving relative paths
  const projectDir = loaded?.fullPath?.split("/").slice(0, -1).join("/");

  // Handle incoming deep links (todos:// protocol)
  useDeepLink(projectDir);

  // Timer warning effect using pure computeTimerEvents function
  useEffect(() => {
    if (!loaded || loaded.workState === "planning") return;
    const endsAt = loaded.stateTransitions.endsAt;
    if (!endsAt) return;

    const checkTimer = () => {
      const now = clock.now();
      const timerState: TimerState = {
        workState: loaded.workState,
        startedAt: loaded.stateTransitions.startedAt,
        endsAt,
        lastWarningAt: timerStateRef.current.lastWarningAt,
      };

      const result = computeTimerEvents(timerState, now);

      // Emit all computed events via EventBus
      for (const event of result.events) {
        eventBus.emit(event);
      }

      // Apply state updates (for warning deduplication)
      if (result.nextState.lastWarningAt !== undefined) {
        timerStateRef.current.lastWarningAt = result.nextState.lastWarningAt;
      }
    };

    // Check every 5 seconds
    const intervalId = clock.setInterval(checkTimer, 5000);
    return () => clock.clearInterval(intervalId);
  }, [loaded, eventBus, clock]);

  const handleCompleteTask = (task: ProjectMarkdown & { type: "task" }, draft: ProjectFile) => {
    // Find the completion mark style from other completed tasks
    const completionMark =
      draft.markdown.find((m: ProjectMarkdown): m is ProjectMarkdown & { type: "task" } =>
        Boolean(m.type === "task" && m.complete),
      )?.complete || "x";

    // Find and update the task
    const taskToComplete = draft.markdown.find(
      (m: ProjectMarkdown): m is ProjectMarkdown & { type: "task" } => m.type === "task" && m.name === task.name,
    );

    if (taskToComplete) {
      taskToComplete.complete = taskToComplete.complete ? false : completionMark;
      if (taskToComplete.complete) {
        // Emit task completion events via EventBus
        const events = computeTaskCompletedEvents(task.name, clock.now());
        for (const event of events) {
          eventBus.emit(event);
        }
      }
      return true;
    }
    return false;
  };

  const handleStateChange = async (newState: WorkState) => {
    let shouldCollapse = false;
    let shouldExpand = false;

    // Handle window state changes based on current state
    if (!loaded) return;

    if (loaded.workState === "planning") {
      shouldCollapse = true;
      setIsCompact(true);
    } else if (newState === "planning") {
      shouldExpand = true;
      setIsCompact(false);
    }

    // Reset warning time when state changes
    timerStateRef.current.lastWarningAt = undefined;

    // Update the work state and wait for it to propagate
    await projectManager.updateWorkState(newState);

    // Wait a tick for React state to update
    await new Promise((resolve) => setTimeout(resolve, 0));

    // Emit state change events via EventBus (including sound events)
    const stateChangeEvents = computeStateChangeEvents(loaded.workState, newState, clock.now());
    for (const event of stateChangeEvents) {
      eventBus.emit(event);
    }

    // Update window format based on state
    if (shouldCollapse) {
      await appWindows.collapseToTracker();
    } else if (shouldExpand) {
      await appWindows.expandToPlanner();
    }
  };

  const handleTimeAdjust = async (ms: number) => {
    await projectManager.updateStateTransitions({
      endsAt: clock.now() + ms,
    });
  };

  // If no project is loaded, show the choose project UI
  if (!loaded || !project) {
    return (
      <AppNoProject
        onOpenProject={() => projectManager.openProject()}
        onOpenFolder={() => projectManager.openProjectFolder()}
        startupWarning={startupWarning}
      />
    );
  }

  const endTime =
    loaded.stateTransitions.endsAt ??
    (loaded.workState === "working"
      ? loaded.stateTransitions.startedAt + project.pomodoroSettings.workDuration * 60 * 1000
      : loaded.stateTransitions.startedAt + project.pomodoroSettings.breakDuration * 60 * 1000);

  const commonProps = {
    project: loaded,
    loaded,
    endTime,
    clock,
    onStateChange: handleStateChange,
    onTimeAdjust: handleTimeAdjust,
    onOpenProject: () => projectManager.openProject(),
    onOpenFolder: () => projectManager.openProjectFolder(),
    onCompleteTask: async (task: ProjectMarkdown & { type: "task" }) => {
      await projectManager.updateProject((draft) => handleCompleteTask(task, draft));
    },
    toggleCompact: () => setIsCompact(!isCompact),
  };

  return (
    <MarkdownProvider basePath={projectDir}>
      {isCompact ? <AppCompact {...commonProps} /> : <AppPlanner {...commonProps} />}
    </MarkdownProvider>
  );
}

interface AppViewProps {
  project: LoadedProjectState;
  loaded: LoadedProjectState | undefined;
  endTime: number;
  clock: Clock;
  onStateChange: (newState: WorkState) => void;
  onTimeAdjust: (ms: number) => void;
  onOpenProject: () => void;
  onOpenFolder: () => void;
  onCompleteTask: (task: ProjectMarkdown & { type: "task" }) => void;
  toggleCompact: () => void;
}

function useCurrentTask(project: LoadedProjectState) {
  const [currentTask, setCurrentTask] = useState<ProjectMarkdown & { type: "task" }>();
  useEffect(() => {
    const task = project.projectFile.markdown.find(
      (m): m is ProjectMarkdown & { type: "task" } => m.type === "task" && !m.complete,
    );
    setCurrentTask(task);
  }, [project]);
  return currentTask;
}

const NON_DRAG_TARGETS = [HTMLInputElement, HTMLTextAreaElement, HTMLParagraphElement];

const findParent = (element: HTMLElement, condition: (element: HTMLElement) => boolean): HTMLElement | null => {
  if (condition(element)) return element;
  if (element.parentElement) return findParent(element.parentElement, condition);
  return null;
};

function AppCompact({
  project,
  loaded,
  endTime,
  clock,
  onStateChange,
  onTimeAdjust,
  onCompleteTask,
  toggleCompact,
}: AppViewProps) {
  const currentTask = useCurrentTask(project);
  const workingOnTask = project.workState === "working" && currentTask != null;
  const colors = workingOnTask
    ? "bg-gradient-to-r from-amber-50 to-amber-100 border-amber-300/50 text-blue-900"
    : "bg-gradient-to-r from-slate-50 to-slate-100 border-blue-400/30 text-slate-900";

  const handleCompleteTask = () => {
    if (!currentTask) return;
    onCompleteTask(currentTask);
  };

  return (
    <main
      className={`h-screen flex items-center pl-1 pr-1.5 border-2 shadow-sm backdrop-blur-sm ${colors} transition-colors duration-300`}
      onMouseDown={(event) => {
        if (NON_DRAG_TARGETS.some((t) => event.target instanceof t)) return;
        if (event.target instanceof HTMLElement && findParent(event.target, (e) => e.dataset.noDrag != null)) return;
        Window.getCurrent().startDragging().catch(console.error);
        event.preventDefault();
        event.stopPropagation();
      }}
    >
      <div className="flex-1 flex items-center min-w-0 gap-1">
        {currentTask && (
          <button
            onClick={handleCompleteTask}
            className="shrink-0 p-1.5 text-gray-600 hover:bg-gray-200 rounded"
            title="Complete current task"
          >
            <IconCheck size={16} />
          </button>
        )}
        <div className="flex items-center gap-2 min-w-0 flex-1">
          <CurrentTaskName currentTask={currentTask} placeholder="All tasks completed!" />
        </div>
        {currentTask?.details?.trim() && (
          <div className="absolute top-full left-0 right-0 bg-white/90 p-2 text-sm" data-no-drag>
            <Markdown>{currentTask.details}</Markdown>
          </div>
        )}
        <div className="flex items-center gap-2 shrink-0">
          {loaded?.fullPath && (
            <button
              onClick={() => loaded.fullPath && openPath(loaded.fullPath)}
              className="text-xs p-1.5 text-gray-600 hover:bg-gray-200 rounded"
              title="Open project file in default application"
            >
              <IconEdit size={16} />
            </button>
          )}
        </div>
        <div className="flex items-center gap-2 shrink-0">
          {project.workState !== "planning" && (
            <Timer
              startTime={project.stateTransitions.startedAt}
              endTime={endTime}
              className="text-sm font-mono py-0"
              onAdjustTime={onTimeAdjust}
              clock={clock}
            />
          )}
          <StateControls project={project} onStateChange={onStateChange} compact toggleCompact={toggleCompact} />
        </div>
      </div>
    </main>
  );
}

function CurrentTaskName({
  currentTask,
  placeholder,
}: { currentTask: (ProjectMarkdown & { type: "task" }) | undefined; placeholder: string }) {
  return (
    <div className="text-lg tracking-wide font-medium truncate flex-1 min-w-0 rounded px-2 py-0.5 relative group select-none hover:bg-black/5 transition-colors cursor-default">
      {currentTask?.name ? (
        <Markdown inline>{currentTask.name}</Markdown>
      ) : (
        <span className="text-gray-500">{placeholder}</span>
      )}
      {currentTask?.name && (
        <CopyButton
          copyContent={currentTask.name}
          className="absolute right-2 top-1/2 -translate-y-1/2 opacity-0 group-hover:opacity-100 transition-opacity"
        />
      )}
    </div>
  );
}

const CopyButton = forwardRef<HTMLButtonElement, { copyContent: string; className?: string }>(
  ({ className, copyContent }, ref) => {
    const [copied, setCopied] = useState(false);
    const handleCopy = () => {
      navigator.clipboard.writeText(copyContent);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    };
    return (
      <button
        ref={ref}
        className={`cursor-copy text-xs p-1 rounded-md text-gray-600 bg-gray-100 transition-all ${className}`}
        onClick={handleCopy}
        children={copied ? "Copied!" : <IconClipboard size={12} />}
      />
    );
  },
);

CopyButton.displayName = "CopyButton";

function AppPlanner({
  project,
  loaded,
  endTime,
  clock,
  onStateChange,
  onTimeAdjust,
  onCompleteTask,
  onOpenProject,
  onOpenFolder,
  toggleCompact,
}: AppViewProps) {
  const sessionsDebugEnabled = import.meta.env.DEV;
  const [showSessionsDebug, setShowSessionsDebug] = useState(false);

  return (
    <main className="h-screen flex flex-col bg-gradient-to-br from-white to-gray-50">
      <header
        className="flex items-center justify-between border-b border-gray-200 px-4 bg-white/80 gap-4"
        onMouseDown={(event) => {
          Window.getCurrent().startDragging().catch(console.error);
          event.preventDefault();
          event.stopPropagation();
        }}
      >
        {/* spacer for native window controls */}
        <div className="flex-shrink-0 w-14 h-full border-r border-gray-200" />
        <div className="flex items-center gap-3 select-none text-gray-400 flex-grow">
          <span className="text-sm font-normal text-gray-600 pointer-events-none">
            {project.workState === "planning" && "Right Now"}
            {project.workState === "working" && "Working"}
            {project.workState === "break" && "Break"}
          </span>
          {project.workState !== "planning" && (
            <Timer
              startTime={project.stateTransitions.startedAt}
              endTime={endTime}
              className="text-sm font-mono text-gray-600 py-0"
              onAdjustTime={onTimeAdjust}
              clock={clock}
            />
          )}
        </div>
        <div className="flex items-center">
          {loaded && (
            <button
              onClick={() => loaded.fullPath && openPath(loaded.fullPath)}
              className="text-xs px-3 py-1.5 text-gray-600 hover:bg-gray-100 transition-colors flex gap-1 items-center"
              title="Open project file in default application"
              children={[loaded.fullPath.split("/").slice(-2).join("/"), <IconEdit size={12} />]}
            />
          )}
          <button
            onClick={onOpenProject}
            className="text-xs px-3 py-1.5 text-gray-600 hover:bg-gray-100 transition-colors"
            title="Open TODO file"
            children="Open File..."
          />
          <button
            onClick={onOpenFolder}
            className="text-xs px-3 py-1.5 text-gray-600 hover:bg-gray-100 transition-colors"
            title="Open project folder"
            children="Open Folder..."
          />
        </div>
      </header>

      <div className="flex-1 overflow-auto p-6 pb-16">
        <TaskList
          tasks={project.projectFile.markdown}
          onCompleteTask={onCompleteTask}
          projectFullPath={loaded?.fullPath}
        />
        {sessionsDebugEnabled && showSessionsDebug && <SessionsDebugPanel />}
      </div>
      <footer className="absolute bottom-0 right-4">
        <div className="flex justify-center p-2 gap-2">
          {sessionsDebugEnabled && (
            <button
              onClick={() => setShowSessionsDebug(!showSessionsDebug)}
              className="px-3 py-1.5 bg-gray-700 text-white text-xs rounded hover:bg-gray-800 transition-colors flex items-center gap-1"
              title="Toggle Sessions Debug Panel (Dev)"
            >
              <IconTerminal size={14} />
              {showSessionsDebug ? "Hide" : "Show"} Sessions
            </button>
          )}
          <StateControls project={project} onStateChange={onStateChange} toggleCompact={toggleCompact} />
        </div>
      </footer>
    </main>
  );
}

function AppNoProject({
  onOpenProject,
  onOpenFolder,
  startupWarning,
}: { onOpenProject: () => void; onOpenFolder: () => void; startupWarning?: StartupWarning }) {
  return (
    <main className="h-screen flex flex-col items-center justify-center bg-gradient-to-br from-gray-50 to-gray-100">
      <h1 className="text-xl font-semibold text-gray-800 mb-3">Welcome to Right Now</h1>
      <p className="text-sm text-gray-600 mb-6">Choose a project file or folder to begin</p>

      {startupWarning && (
        <div className="mb-6 max-w-md px-4 py-3 bg-yellow-50 border border-yellow-200 rounded">
          <p className="text-sm text-yellow-800 font-medium mb-1">{startupWarning.message}</p>
          {startupWarning.details && (
            <p className="text-xs text-yellow-700 font-mono break-all">{startupWarning.details}</p>
          )}
        </div>
      )}

      <div className="flex gap-3">
        <button
          onClick={onOpenProject}
          className="px-5 py-2.5 bg-blue-600 text-white text-sm hover:bg-blue-700 transition-all hover:shadow-md active:scale-95"
        >
          Open File...
        </button>
        <button
          onClick={onOpenFolder}
          className="px-5 py-2.5 bg-gray-600 text-white text-sm hover:bg-gray-700 transition-all hover:shadow-md active:scale-95"
        >
          Open Folder...
        </button>
      </div>
    </main>
  );
}

export default AppOuter;
