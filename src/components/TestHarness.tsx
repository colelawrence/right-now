// Test harness component for E2E testing
// Renders both Planner and Compact views side-by-side with a debug panel

import { useAtom } from "jotai";
import { useEffect, useState } from "react";
import type { ProjectMarkdown } from "../lib/ProjectStateEditor";
import type { Clock } from "../lib/clock";
import type { LoadedProjectState, WorkState } from "../lib/project";
import type { AppControllers } from "../main-test";
import { StateControls } from "./StateControls";
import { TaskList } from "./TaskList";
import { Timer } from "./Timer";
import { MarkdownProvider } from "./markdown";

interface TestHarnessProps {
  controllers?: AppControllers;
  startupError?: Error;
}

type ViewMode = "split" | "planner" | "compact";

export function TestHarness({ controllers, startupError }: TestHarnessProps) {
  if (startupError) {
    return (
      <main className="h-screen flex items-center justify-center bg-red-100 p-4">
        <div className="max-w-lg">
          <h1 className="text-lg font-semibold text-red-700 mb-2">Test Harness Error</h1>
          <pre className="bg-red-50 p-3 rounded text-sm text-red-700 whitespace-pre-wrap overflow-auto">
            {startupError.message}
          </pre>
        </div>
      </main>
    );
  }

  if (!controllers) {
    return (
      <main className="h-screen flex items-center justify-center bg-gray-50">
        <div className="text-sm text-gray-600">Initializing test harness...</div>
      </main>
    );
  }

  return <TestHarnessReady controllers={controllers} />;
}

function TestHarnessReady({ controllers }: { controllers: AppControllers }) {
  const { projectManager, appWindows, clock } = controllers;
  const [loaded, setLoaded] = useState<LoadedProjectState>();
  const [viewMode, setViewMode] = useState<ViewMode>("split");
  const [isCompact] = useAtom(appWindows.currentlyMiniAtom);

  useEffect(() => {
    const unsubscribe = projectManager.subscribe((project) => {
      setLoaded(project);
    });
    return unsubscribe;
  }, [projectManager]);

  const project = loaded?.projectFile;
  const endTime = loaded?.stateTransitions.endsAt ?? clock.now();

  const handleStateChange = async (newState: WorkState) => {
    await projectManager.updateWorkState(newState);
  };

  const handleTimeAdjust = async (ms: number) => {
    const currentEndsAt = loaded?.stateTransitions.endsAt ?? clock.now();
    await projectManager.updateStateTransitions({
      startedAt: loaded?.stateTransitions.startedAt ?? clock.now(),
      endsAt: currentEndsAt + ms,
    });
  };

  const handleCompleteTask = async (task: ProjectMarkdown & { type: "task" }) => {
    await projectManager.updateProject((draft) => {
      const taskIndex = draft.markdown.findIndex((m) => m.type === "task" && m.name === task.name);
      if (taskIndex !== -1) {
        (draft.markdown[taskIndex] as typeof task).complete = "x";
      }
    });
  };

  const handleOpenProject = () => projectManager.openProject();

  const projectDir = loaded?.fullPath?.split("/").slice(0, -1).join("/");

  return (
    <div className="h-screen flex flex-col bg-gray-100">
      {/* Header with view controls */}
      <header className="flex items-center justify-between p-2 bg-gray-800 text-white gap-4">
        <h1 className="text-sm font-mono">Right Now Test Harness</h1>
        <div className="flex gap-2">
          <button
            className={`px-3 py-1 text-xs rounded ${viewMode === "split" ? "bg-blue-500" : "bg-gray-600"}`}
            onClick={() => setViewMode("split")}
          >
            Split
          </button>
          <button
            className={`px-3 py-1 text-xs rounded ${viewMode === "planner" ? "bg-blue-500" : "bg-gray-600"}`}
            onClick={() => setViewMode("planner")}
          >
            Planner
          </button>
          <button
            className={`px-3 py-1 text-xs rounded ${viewMode === "compact" ? "bg-blue-500" : "bg-gray-600"}`}
            onClick={() => setViewMode("compact")}
          >
            Compact
          </button>
        </div>
        <div className="text-xs font-mono text-gray-400">
          {loaded ? `Project: ${loaded.fullPath?.split("/").pop()}` : "No project loaded"}
        </div>
      </header>

      {/* Main content area */}
      <div className="flex-1 flex overflow-hidden">
        {/* Views */}
        <div className="flex-1 flex gap-2 p-2 overflow-hidden">
          {(viewMode === "split" || viewMode === "planner") && (
            <div className="flex-1 bg-white rounded shadow overflow-hidden flex flex-col">
              <div className="bg-gray-100 px-2 py-1 text-xs text-gray-500 border-b">Planner View (600x400)</div>
              <div className="flex-1 overflow-auto">
                {loaded && project ? (
                  <MarkdownProvider basePath={projectDir}>
                    <PlannerView
                      project={loaded}
                      endTime={endTime}
                      clock={clock}
                      onStateChange={handleStateChange}
                      onTimeAdjust={handleTimeAdjust}
                      onCompleteTask={handleCompleteTask}
                      onOpenProject={handleOpenProject}
                    />
                  </MarkdownProvider>
                ) : (
                  <NoProjectView onOpenProject={handleOpenProject} />
                )}
              </div>
            </div>
          )}

          {(viewMode === "split" || viewMode === "compact") && (
            <div className="flex-1 bg-white rounded shadow overflow-hidden flex flex-col">
              <div className="bg-gray-100 px-2 py-1 text-xs text-gray-500 border-b">Compact View (400x40)</div>
              <div className="flex-1 flex items-center justify-center bg-gray-50 p-4">
                <div className="w-[400px] h-[40px] shadow-lg">
                  {loaded && project ? (
                    <MarkdownProvider basePath={projectDir}>
                      <CompactView
                        project={loaded}
                        endTime={endTime}
                        clock={clock}
                        onStateChange={handleStateChange}
                        onTimeAdjust={handleTimeAdjust}
                        onCompleteTask={handleCompleteTask}
                      />
                    </MarkdownProvider>
                  ) : (
                    <div className="h-full flex items-center justify-center bg-gray-200 text-sm text-gray-500">
                      No project
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Debug panel */}
        <div className="w-80 bg-gray-900 text-gray-100 overflow-auto flex-shrink-0">
          <DebugPanel loaded={loaded} isCompact={isCompact ?? false} />
        </div>
      </div>
    </div>
  );
}

interface ViewProps {
  project: LoadedProjectState;
  endTime: number;
  clock: Clock;
  onStateChange: (newState: WorkState) => void;
  onTimeAdjust: (ms: number) => void;
  onCompleteTask: (task: ProjectMarkdown & { type: "task" }) => void;
  onOpenProject?: () => void;
}

function PlannerView({
  project,
  endTime,
  clock,
  onStateChange,
  onTimeAdjust,
  onCompleteTask,
  onOpenProject,
}: ViewProps) {
  return (
    <div className="h-full flex flex-col bg-white">
      <header className="flex items-center justify-between border-b border-gray-200 px-4 py-2">
        <span className="text-sm font-medium text-gray-600">
          {project.workState === "planning" && "Planning"}
          {project.workState === "working" && "Working"}
          {project.workState === "break" && "Break"}
        </span>
        {project.workState !== "planning" && (
          <Timer
            startTime={project.stateTransitions.startedAt}
            endTime={endTime}
            className="text-sm font-mono"
            onAdjustTime={onTimeAdjust}
            clock={clock}
          />
        )}
        {onOpenProject && (
          <button onClick={onOpenProject} className="text-xs px-2 py-1 text-gray-600 hover:bg-gray-100 rounded">
            Open...
          </button>
        )}
      </header>

      <div className="flex-1 overflow-auto p-4">
        <TaskList tasks={project.projectFile.markdown} onCompleteTask={onCompleteTask} />
      </div>

      <footer className="p-2 border-t flex justify-center">
        <StateControls project={project} onStateChange={onStateChange} toggleCompact={() => {}} />
      </footer>
    </div>
  );
}

function CompactView({ project, endTime, clock, onStateChange, onTimeAdjust, onCompleteTask }: ViewProps) {
  const currentTask = project.projectFile.markdown.find(
    (m): m is ProjectMarkdown & { type: "task" } => m.type === "task" && !m.complete,
  );

  const workingOnTask = project.workState === "working" && currentTask != null;
  const colors = workingOnTask
    ? "bg-gradient-to-r from-amber-50 to-amber-100 border-amber-300"
    : "bg-gradient-to-r from-slate-50 to-slate-100 border-slate-300";

  return (
    <div className={`h-full flex items-center px-2 border ${colors} text-sm`}>
      <div className="flex-1 truncate font-medium">{currentTask?.name || "All done!"}</div>
      {project.workState !== "planning" && (
        <Timer
          startTime={project.stateTransitions.startedAt}
          endTime={endTime}
          className="text-xs font-mono mx-2"
          onAdjustTime={onTimeAdjust}
          clock={clock}
        />
      )}
      <StateControls project={project} onStateChange={onStateChange} compact toggleCompact={() => {}} />
    </div>
  );
}

function NoProjectView({ onOpenProject }: { onOpenProject: () => void }) {
  return (
    <div className="h-full flex flex-col items-center justify-center p-4">
      <p className="text-sm text-gray-500 mb-4">No project loaded</p>
      <button onClick={onOpenProject} className="px-4 py-2 bg-blue-600 text-white text-sm rounded hover:bg-blue-700">
        Open Project
      </button>
    </div>
  );
}

function DebugPanel({ loaded, isCompact }: { loaded?: LoadedProjectState; isCompact: boolean }) {
  return (
    <div className="p-3 space-y-4 text-xs font-mono">
      <div>
        <h3 className="text-gray-400 mb-1">State</h3>
        <div className="space-y-1">
          <div>
            workState: <span className="text-green-400">{loaded?.workState ?? "none"}</span>
          </div>
          <div>
            isCompact: <span className="text-green-400">{isCompact ? "true" : "false"}</span>
          </div>
          <div>
            fullPath: <span className="text-yellow-400 break-all">{loaded?.fullPath ?? "none"}</span>
          </div>
        </div>
      </div>

      <div>
        <h3 className="text-gray-400 mb-1">Pomodoro Settings</h3>
        <div className="space-y-1">
          <div>
            workDuration:{" "}
            <span className="text-green-400">{loaded?.projectFile.pomodoroSettings.workDuration ?? 0}m</span>
          </div>
          <div>
            breakDuration:{" "}
            <span className="text-green-400">{loaded?.projectFile.pomodoroSettings.breakDuration ?? 0}m</span>
          </div>
        </div>
      </div>

      <div>
        <h3 className="text-gray-400 mb-1">Transitions</h3>
        <div className="space-y-1">
          <div>
            startedAt:{" "}
            <span className="text-blue-400">
              {loaded?.stateTransitions.startedAt ? new Date(loaded.stateTransitions.startedAt).toISOString() : "none"}
            </span>
          </div>
          <div>
            endsAt:{" "}
            <span className="text-blue-400">
              {loaded?.stateTransitions.endsAt ? new Date(loaded.stateTransitions.endsAt).toISOString() : "none"}
            </span>
          </div>
        </div>
      </div>

      <div>
        <h3 className="text-gray-400 mb-1">
          Tasks ({loaded?.projectFile.markdown.filter((m) => m.type === "task").length ?? 0})
        </h3>
        <div className="space-y-1 max-h-48 overflow-auto">
          {loaded?.projectFile.markdown
            .filter((m): m is ProjectMarkdown & { type: "task" } => m.type === "task")
            .map((task, i) => (
              <div key={i} className="flex gap-2">
                <span className={task.complete ? "text-gray-500" : "text-green-400"}>
                  {task.complete ? "[x]" : "[ ]"}
                </span>
                <span className="truncate">{task.name}</span>
              </div>
            ))}
        </div>
      </div>

      <div>
        <h3 className="text-gray-400 mb-1">Test Bridge</h3>
        <div className="text-blue-400">window.__TEST_BRIDGE__</div>
        <div className="text-gray-500 mt-1">Use browser console to interact with the test bridge.</div>
      </div>
    </div>
  );
}
