import { IconCoffee, IconPlayerPlay, IconSquare } from "@tabler/icons-react";
import type { LoadedProjectState, WorkState } from "../lib/project";

interface StateControlsProps {
  project: LoadedProjectState;
  onStateChange: (newState: WorkState) => void;
  /** If true, shows only icons without text */
  compact?: boolean;
}

export function StateControls({ project, onStateChange, compact }: StateControlsProps) {
  const handleStateChange = (newState: WorkState) => {
    onStateChange(newState);
  };

  return (
    <div className="flex items-center gap-2">
      {project.workState === "planning" && (
        <button
          onClick={() => handleStateChange("working")}
          className="px-2 py-1.5 bg-blue-600 text-white text-sm rounded hover:bg-blue-700 transition-colors flex items-center gap-2 cursor-pointer"
        >
          <IconPlayerPlay size={16} />
          {!compact && "Start"}
        </button>
      )}
      {project.workState === "working" && (
        <button
          onClick={() => handleStateChange("break")}
          className="px-2 py-1.5 bg-blue-600 text-white text-sm rounded hover:bg-blue-700 transition-colors flex items-center gap-2 cursor-pointer"
        >
          <IconCoffee size={16} />
          {!compact && "Break"}
        </button>
      )}
      {project.workState === "break" && (
        <button
          onClick={() => handleStateChange("working")}
          className="px-2 py-1.5 bg-blue-600 text-white text-sm rounded hover:bg-blue-700 transition-colors flex items-center gap-2 cursor-pointer"
        >
          <IconPlayerPlay size={16} />
          {!compact && "Resume"}
        </button>
      )}
      {project.workState !== "planning" && (
        <button
          onClick={() => handleStateChange("planning")}
          className={`px-2 py-1.5 bg-gray-600 text-white text-sm rounded hover:bg-gray-700 transition-colors flex items-center gap-2 cursor-pointer`}
          title="End Session"
        >
          <IconSquare size={16} />
          {!compact && "End Session"}
        </button>
      )}
    </div>
  );
}
