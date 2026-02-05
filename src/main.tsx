import { Buffer } from "buffer";
import { Provider as JotaiProvider } from "jotai/react";
import React from "react";
import ReactDOM from "react-dom/client";

import "./styles.css";
import { getDefaultStore } from "jotai";
import AppReady from "./App";
import { type Clock, realClock } from "./lib/clock";
import { AppEventBus, type EventBus } from "./lib/events";
import { ProjectManager } from "./lib/project";
import { createSoundPlayer } from "./lib/sound-player";
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
  clock: Clock;
  eventBus: EventBus;
}

interface StartupWarning {
  message: string;
  details?: string;
}

// Initialize app controllers
async function initializeApp() {
  const jotaiStore = getDefaultStore();
  let controllers: AppControllers | undefined;
  let error: Error | undefined;
  let warning: StartupWarning | undefined;

  try {
    console.info("Initializing app");
    // Initialize core services
    const projectStore = await ProjectStore.initialize();
    console.info("Store initialized");
    const projectManager = new ProjectManager(projectStore, realClock);
    console.info("Project manager initialized");
    const appWindows = new AppWindows();
    await appWindows.initialize();
    console.info("App windows initialized");
    const soundManager = await ISoundManager.initialize(jotaiStore);
    console.info("Sound manager initialized");

    // Create EventBus and wire up SoundPlayer
    const eventBus = new AppEventBus();
    const soundPlayer = createSoundPlayer(eventBus, soundManager);
    console.info("EventBus and SoundPlayer initialized");

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

    controllers = { projectManager, appWindows, projectStore, soundManager, clock: realClock, eventBus };

    // Try to auto-load last active project without prompting
    const lastProject = await projectStore.getLastActiveProject();
    if (lastProject) {
      console.info("Auto-loading last project", { lastProject });
      try {
        await projectManager.loadProject(lastProject, "absolute");
        console.info("Auto-load successful");
      } catch (loadError) {
        console.warn("Failed to auto-load project, falling back to Welcome screen", loadError);
        const errorMsg = loadError instanceof Error ? loadError.message : String(loadError);
        warning = {
          message: `Could not load previous project: ${lastProject.split("/").pop()}`,
          details: errorMsg,
        };
      }
    } else {
      console.info("No last active project, starting with Welcome screen");
    }
  } catch (e) {
    console.error("Failed to initialize app:", e);
    error = e instanceof Error ? e : new Error(String(e));
  }

  console.info("Rendering app", { controllers, error, warning });
  // Render React app with controllers or error
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <JotaiProvider store={getDefaultStore()}>
        <AppReady controllers={controllers} startupError={error} startupWarning={warning} />
      </JotaiProvider>
    </React.StrictMode>,
  );
}

// Start the app
initializeApp();
