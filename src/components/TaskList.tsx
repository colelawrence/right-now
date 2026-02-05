import {
  IconCheckbox,
  IconChevronDown,
  IconChevronUp,
  IconList,
  IconPlayerPause,
  IconPlayerPlay,
  IconPlayerSkipForward,
  IconSquare,
} from "@tabler/icons-react";
import { useState } from "react";
import type { ProjectMarkdown } from "../lib/ProjectStateEditor";
import { SessionClient } from "../lib/SessionClient";
import { SessionsPanel } from "./SessionsPanel";
import { Markdown } from "./markdown";

interface TaskListProps {
  tasks: ProjectMarkdown[];
  onCompleteTask: (task: ProjectMarkdown & { type: "task" }) => void;
  onMoveHeadingSection?: (headingIndex: number, direction: "up" | "down") => void;
  projectFullPath?: string;
}

export function TaskList({ tasks, onCompleteTask, onMoveHeadingSection, projectFullPath }: TaskListProps) {
  const sessionClient = new SessionClient();
  const [showSessionsPanel, setShowSessionsPanel] = useState(false);

  // Group tasks under their most recent heading, tracking the original headingIndex
  const sections: {
    heading?: ProjectMarkdown & { type: "heading" };
    headingIndex?: number;
    items: ProjectMarkdown[];
  }[] = [];
  let currentSection: (typeof sections)[0] = { items: [] };

  for (let i = 0; i < tasks.length; i++) {
    const item = tasks[i];
    if (item.type === "heading") {
      if (currentSection.items.length > 0 || currentSection.heading) {
        sections.push(currentSection);
      }
      currentSection = { heading: item, headingIndex: i, items: [] };
    } else {
      currentSection.items.push(item);
    }
  }
  // Add the last section
  if (currentSection.items.length > 0 || currentSection.heading) {
    sections.push(currentSection);
  }

  // Filter out sections with only headings (keep them for UI consistency)
  const headingSections = sections.filter((s) => s.heading !== undefined);

  return (
    <>
      {showSessionsPanel && <SessionsPanel onClose={() => setShowSessionsPanel(false)} />}

      <div className="space-y-6">
        {/* Sessions button */}
        <div className="flex justify-end">
          <button
            type="button"
            onClick={() => setShowSessionsPanel(true)}
            className="flex items-center gap-2 px-3 py-1.5 text-sm text-gray-700 bg-white border border-gray-300 rounded hover:bg-gray-50 transition-colors"
          >
            <IconList size={16} />
            Sessions
          </button>
        </div>

        {sections.map((section, i) => {
          const itemElements = section.items
            .map((item, j) => {
              if (item.type === "task") {
                return (
                  <TaskRow
                    key={j}
                    task={item}
                    onCompleteTask={onCompleteTask}
                    sessionClient={sessionClient}
                    projectFullPath={projectFullPath}
                  />
                );
              }
              return null;
            })
            .filter(Boolean);
          if (!section.heading && itemElements.length === 0) return null;

          // Determine if this section can move up or down
          const sectionPosition = section.heading ? headingSections.indexOf(section) : -1;
          const canMoveUp = sectionPosition > 0;
          const canMoveDown = sectionPosition >= 0 && sectionPosition < headingSections.length - 1;

          return (
            <div key={i} className="space-y-2">
              {section.heading && (
                <div className="flex items-center gap-2 group">
                  <h2
                    className={`font-medium ${section.heading.level === 1 ? "text-xl" : "text-lg"} pb-2 border-b border-gray-200 mt-0 flex-1`}
                  >
                    <Markdown inline>{section.heading.text}</Markdown>
                  </h2>
                  {onMoveHeadingSection && section.headingIndex !== undefined && (
                    <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity pb-2">
                      <button
                        onClick={() => onMoveHeadingSection(section.headingIndex!, "up")}
                        disabled={!canMoveUp}
                        className="p-1 rounded hover:bg-gray-100 text-gray-600 disabled:opacity-30 disabled:cursor-not-allowed"
                        title="Move section up"
                      >
                        <IconChevronUp size={16} />
                      </button>
                      <button
                        onClick={() => onMoveHeadingSection(section.headingIndex!, "down")}
                        disabled={!canMoveDown}
                        className="p-1 rounded hover:bg-gray-100 text-gray-600 disabled:opacity-30 disabled:cursor-not-allowed"
                        title="Move section down"
                      >
                        <IconChevronDown size={16} />
                      </button>
                    </div>
                  )}
                </div>
              )}
              <div className="space-y-2 pl-1">{itemElements}</div>
            </div>
          );
        })}
      </div>
    </>
  );
}

interface TaskRowProps {
  task: ProjectMarkdown & { type: "task" };
  onCompleteTask: (task: ProjectMarkdown & { type: "task" }) => void;
  sessionClient: SessionClient;
  projectFullPath?: string;
}

function TaskRow({ task, onCompleteTask, sessionClient, projectFullPath }: TaskRowProps) {
  const [actionError, setActionError] = useState<string | null>(null);
  const [isActing, setIsActing] = useState(false);

  const handleStartSession = async () => {
    if (!projectFullPath) {
      setActionError("Project path not available");
      return;
    }

    setActionError(null);
    setIsActing(true);
    try {
      await sessionClient.startSession(task.name, projectFullPath);
      // Session badge will be added by daemon; file watcher will reload
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setActionError(message);
      console.error("Failed to start session:", error);
    } finally {
      setIsActing(false);
    }
  };

  const handleStopSession = async () => {
    if (!task.sessionStatus) return;

    setActionError(null);
    setIsActing(true);
    try {
      await sessionClient.stopSession(task.sessionStatus.sessionId);
      // Session badge will be updated by daemon; file watcher will reload
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setActionError(message);
      console.error("Failed to stop session:", error);
    } finally {
      setIsActing(false);
    }
  };

  const handleContinueSession = async () => {
    if (!task.sessionStatus) return;

    setActionError(null);
    setIsActing(true);
    try {
      const result = await sessionClient.continueSession(task.sessionStatus.sessionId, 512);
      const tail = SessionClient.tailBytesToString(result.tail);
      if (tail) {
        console.log(`Session ${task.sessionStatus.sessionId} tail:\n${tail}`);
        // For now, just alert; can be refined to a modal later
        alert(`Session output (last 512 bytes):\n\n${tail}`);
      } else {
        alert(`Session ${task.sessionStatus.sessionId} continued (no recent output)`);
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setActionError(message);
      console.error("Failed to continue session:", error);
    } finally {
      setIsActing(false);
    }
  };

  const showStartButton = !task.sessionStatus;
  const showContinueButton = !!task.sessionStatus;
  const showStopButton =
    task.sessionStatus && (task.sessionStatus.status === "Running" || task.sessionStatus.status === "Waiting");

  return (
    <div className="flex items-start gap-2 group">
      <button
        onClick={() => onCompleteTask(task)}
        className={`p-1 rounded hover:bg-gray-100 ${task.complete ? "text-green-600" : "text-gray-400"}`}
        title={task.complete ? "Mark incomplete" : "Mark complete"}
      >
        {task.complete ? <IconCheckbox size={16} /> : <IconSquare size={16} />}
      </button>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <div className={task.complete ? "line-through text-gray-500" : ""}>
            <Markdown inline>{task.name}</Markdown>
          </div>
          {task.sessionStatus && (
            <span
              className={`text-xs px-2 py-0.5 rounded-full font-medium ${
                task.sessionStatus.status === "Running"
                  ? "bg-green-100 text-green-700"
                  : task.sessionStatus.status === "Waiting"
                    ? "bg-yellow-100 text-yellow-700"
                    : "bg-gray-100 text-gray-700"
              }`}
              title={`Session ${task.sessionStatus.sessionId}`}
            >
              {task.sessionStatus.status}
            </span>
          )}
        </div>
        {task.details && (
          <div className="text-sm text-gray-600 mt-1">
            <Markdown>{task.details}</Markdown>
          </div>
        )}
        {actionError && <div className="text-xs text-red-600 mt-1 bg-red-50 px-2 py-1 rounded">{actionError}</div>}
      </div>
      <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
        {showStartButton && (
          <button
            onClick={handleStartSession}
            disabled={isActing}
            className="p-1.5 rounded hover:bg-blue-100 text-blue-600 disabled:opacity-50"
            title="Start session"
          >
            <IconPlayerPlay size={16} />
          </button>
        )}
        {showContinueButton && (
          <button
            onClick={handleContinueSession}
            disabled={isActing}
            className="p-1.5 rounded hover:bg-green-100 text-green-600 disabled:opacity-50"
            title="Continue session"
          >
            <IconPlayerSkipForward size={16} />
          </button>
        )}
        {showStopButton && (
          <button
            onClick={handleStopSession}
            disabled={isActing}
            className="p-1.5 rounded hover:bg-red-100 text-red-600 disabled:opacity-50"
            title="Stop session"
          >
            <IconPlayerPause size={16} />
          </button>
        )}
      </div>
    </div>
  );
}
