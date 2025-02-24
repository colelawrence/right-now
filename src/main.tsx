import { Buffer } from "buffer";
import { Provider as JotaiProvider } from "jotai/react";
import React from "react";
import ReactDOM from "react-dom/client";

import "./styles.css";
import { getDefaultStore } from "jotai";
import AppReady from "./App";
import { ProjectManager } from "./lib/project";
import { ISoundManager } from "./lib/sounds";
import { ProjectStore } from "./lib/store";
import { AppWindows } from "./lib/windows";

if (typeof globalThis.Buffer === "undefined") {
  globalThis.Buffer = Buffer;
}

interface AppControllers {
  projectManager: ProjectManager;
  appWindows: AppWindows;
  projectStore: ProjectStore;
  soundManager: ISoundManager;
}

// Initialize app controllers
async function initializeApp() {
  const jotaiStore = getDefaultStore();
  let controllers: AppControllers | undefined;
  let error: Error | undefined;

  try {
    console.info("Initializing app");
    // Initialize core services
    const projectStore = await ProjectStore.initialize();
    console.info("Store initialized");
    const projectManager = new ProjectManager(projectStore);
    console.info("Project manager initialized");
    const appWindows = new AppWindows();
    await appWindows.initialize();
    console.info("App windows initialized");
    const soundManager = await ISoundManager.initialize(jotaiStore);
    console.info("Sound manager initialized");

    // Wire up project change listeners
    projectManager.subscribe(async (loaded) => {
      console.log("Project changed", loaded);
      if (!loaded) {
        appWindows.setTitle(null, null);
        await appWindows.expandToPlanner(); // Always expand when no project
        return;
      }

      const { projectFile, fullPath, workState } = loaded;

      // Find current task and its context
      const tasks = projectFile.markdown?.filter((a): a is typeof a & { type: "task" } => a.type === "task") ?? [];
      const currentTask = tasks.find((t) => !t.complete);

      // Find the last heading before the current task
      let currentHeading: string | undefined;
      if (currentTask) {
        for (let i = 0; i < projectFile.markdown.length; i++) {
          const item = projectFile.markdown[i];
          if (item === currentTask) break;
          if (item.type === "heading") {
            currentHeading = item.text;
          }
        }
      }

      await appWindows.setTitle(
        workState === "working" && currentTask ? { task: currentTask, heading: currentHeading } : null,
        fullPath,
        tasks,
      );

      // Set initial window state based on project state
      if (workState === "planning") {
        await appWindows.expandToPlanner();
      } else {
        await appWindows.collapseToTracker();
      }
    });

    controllers = { projectManager, appWindows, projectStore, soundManager };

    // Try to load last active project
    const lastProject = await projectStore.getLastActiveProject();
    console.info("Opening project", { lastProject });
    await projectManager.openProject(lastProject);
  } catch (e) {
    console.error("Failed to initialize app:", e);
    error = e instanceof Error ? e : new Error(String(e));
  }

  console.info("Rendering app", { controllers, error });
  // Render React app with controllers or error
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <JotaiProvider store={getDefaultStore()}>
        <AppReady controllers={controllers} startupError={error} />
      </JotaiProvider>
    </React.StrictMode>,
  );
}

// Start the app
initializeApp();
