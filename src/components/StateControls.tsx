import { IconCoffee, IconMaximize, IconMinimize, IconPlayerPlay, IconSquare } from "@tabler/icons-react";
import type { LoadedProjectState, WorkState } from "../lib/project";
import { cn } from "./utils/cn";

interface StateControlsProps {
  project: LoadedProjectState;
  onStateChange: (newState: WorkState) => void;
  /** If true, shows only icons without text */
  compact?: boolean;
  toggleCompact: () => void;
}

const buttonBase =
  "px-2 py-1.5 bg-gray-600 text-white text-sm rounded hover:bg-gray-700 transition-colors flex items-center gap-2 cursor-pointer";
const buttonBaseCompact = buttonBase;

export function StateControls({ project, onStateChange, compact, toggleCompact }: StateControlsProps) {
  const handleStateChange = (newState: WorkState) => {
    onStateChange(newState);
  };

  return (
    <div className="flex items-center gap-2">
      {project.workState === "planning" && (
        <button
          onClick={() => handleStateChange("working")}
          className={cn(compact ? buttonBaseCompact : buttonBase, "bg-blue-600 text-white rounded hover:bg-blue-700")}
        >
          <IconPlayerPlay size={16} />
          {!compact && "Start"}
        </button>
      )}
      {project.workState === "working" && (
        <button
          onClick={() => handleStateChange("break")}
          className={cn(compact ? buttonBaseCompact : buttonBase, "bg-blue-600 text-white rounded hover:bg-blue-700")}
        >
          <IconCoffee size={16} />
          {!compact && "Break"}
        </button>
      )}
      {project.workState === "break" && (
        <button
          onClick={() => handleStateChange("working")}
          className={cn(compact ? buttonBaseCompact : buttonBase, "bg-blue-600 text-white rounded hover:bg-blue-700")}
        >
          <IconPlayerPlay size={16} />
          {!compact && "Resume"}
        </button>
      )}
      {project.workState !== "planning" && (
        <button
          onClick={() => handleStateChange("planning")}
          className={cn(compact ? buttonBaseCompact : buttonBase, "bg-gray-600 text-white rounded hover:bg-gray-700")}
          title="End Session"
        >
          <IconSquare size={16} />
          {!compact && "End Session"}
        </button>
      )}
      <button
        onClick={toggleCompact}
        className="p-1 cursor-pointer text-gray-600 hover:text-gray-900 bg-gray-200 hover:bg-gray-300 rounded transition-colors"
        title="Toggle View"
      >
        {compact ? <IconMaximize size={12} strokeWidth={2.5} /> : <IconMinimize size={12} strokeWidth={2.5} />}
      </button>
    </div>
  );
}
