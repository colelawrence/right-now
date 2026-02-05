import { useEffect, useState } from "react";
import type { ProjectManager, ProjectStore } from "../lib";

interface StartupWarning {
  message: string;
  details?: string;
}

interface WelcomeProps {
  projectManager: ProjectManager;
  projectStore: ProjectStore;
  startupWarning?: StartupWarning;
  onShowWalkthrough: () => void;
}

export function Welcome({ projectManager, projectStore, startupWarning, onShowWalkthrough }: WelcomeProps) {
  const [recentProjects, setRecentProjects] = useState<string[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    projectStore.getRecentProjects().then(setRecentProjects).catch(console.error);
  }, [projectStore]);

  const handleOpenRecent = async (path: string) => {
    setLoadError(null);
    try {
      await projectManager.loadProject(path, "absolute");
    } catch (error) {
      setLoadError(`Failed to open ${formatProjectPath(path)}: ${error}`);
    }
  };

  const handleOpenProject = () => {
    setLoadError(null);
    projectManager.openProject();
  };

  const handleOpenFolder = () => {
    setLoadError(null);
    projectManager.openProjectFolder();
  };

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

      {loadError && (
        <div className="mb-6 max-w-md px-4 py-3 bg-red-50 border border-red-200 rounded">
          <p className="text-sm text-red-800 font-medium">{loadError}</p>
        </div>
      )}

      {recentProjects.length > 0 && (
        <div className="mb-6 w-full max-w-md">
          <h2 className="text-sm font-medium text-gray-700 mb-2 px-2">Recent Projects</h2>
          <div className="bg-white border border-gray-200 rounded-lg shadow-sm divide-y divide-gray-100">
            {recentProjects.map((path) => (
              <button
                key={path}
                onClick={() => handleOpenRecent(path)}
                className="w-full px-4 py-3 text-left hover:bg-gray-50 transition-colors flex flex-col gap-1"
              >
                <span className="text-sm font-medium text-gray-900">{formatProjectPath(path)}</span>
                <span className="text-xs font-mono text-gray-500 truncate">{path}</span>
              </button>
            ))}
          </div>
        </div>
      )}

      <div className="flex gap-3">
        <button
          onClick={handleOpenProject}
          className="px-5 py-2.5 bg-blue-600 text-white text-sm hover:bg-blue-700 transition-all hover:shadow-md active:scale-95"
        >
          Open File...
        </button>
        <button
          onClick={handleOpenFolder}
          className="px-5 py-2.5 bg-gray-600 text-white text-sm hover:bg-gray-700 transition-all hover:shadow-md active:scale-95"
        >
          Open Folder...
        </button>
      </div>

      <button
        onClick={onShowWalkthrough}
        className="mt-4 text-sm text-gray-600 hover:text-gray-800 underline decoration-dotted"
      >
        Show walkthrough
      </button>
    </main>
  );
}

/**
 * Format a project path for display: show last 2 segments as the title.
 * e.g. "/Users/cole/dev/my-app/TODO.md" → "my-app/TODO.md"
 */
function formatProjectPath(path: string): string {
  const segments = path.split("/").filter(Boolean);
  if (segments.length <= 2) return path;
  return segments.slice(-2).join("/");
}
