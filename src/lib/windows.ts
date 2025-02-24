import { path } from "@tauri-apps/api";
import { invoke } from "@tauri-apps/api/core";
import { Image } from "@tauri-apps/api/image";
import { Menu, MenuItem } from "@tauri-apps/api/menu";
import { TrayIcon } from "@tauri-apps/api/tray";
import { LogicalSize, Window } from "@tauri-apps/api/window";
import { openPath } from "@tauri-apps/plugin-opener";
import { exit } from "@tauri-apps/plugin-process";
import { atom, getDefaultStore } from "jotai";
import { withError } from "./withError";

const res = (resPath: `resources/${string}`) => path.resolveResource(resPath);

const TRAY_IMAGES = {
  ZeroWidth: res("resources/tray-0width.png")
    .then((a) => Image.fromPath(a))
    .catch(withError(`Failed to load ZeroWidth tray icon from resources/tray-0width.png`)),
  Base: res("resources/tray-base.png")
    .then((a) => Image.fromPath(a))
    .catch(withError(`Failed to load Base tray icon from resources/tray-base.png`)),
  Base2x: res("resources/tray-base@2x.png")
    .then((a) => Image.fromPath(a))
    .catch(withError(`Failed to load Base2x tray icon from resources/tray-base@2x.png`)),
} as const;

interface TaskMenuItem {
  task: { name: string; complete?: string | boolean };
  menuItem: MenuItem;
}

interface TaskContext {
  task: { name: string; complete?: string | boolean };
  heading?: string;
}

export class AppWindows {
  // main window
  private window?: Window;
  // See https://v2.tauri.app/learn/system-tray/ for tray docs
  private tray?: TrayIcon;
  private trayMenu?: Menu;
  private showPlanner?: MenuItem;
  private showTracker?: MenuItem;
  private focus?: MenuItem;
  private editTodoFile?: MenuItem;
  private todoPath?: string;
  private _currentlyMiniAtom = atom<boolean | undefined>();
  public readonly currentlyMiniAtom = atom(
    (get) => get(this._currentlyMiniAtom),
    async (_get, set, mini: boolean) => {
      this._applyWindowAppearance(mini);
      await this.showPlanner?.setEnabled(mini);
      await this.showTracker?.setEnabled(!mini);
    },
  );
  private static MAX_NEXT_TASKS = 2;
  private taskMenuItems: TaskMenuItem[] = [];
  private tasksSeparatorStart?: MenuItem;
  private tasksSeparatorEnd?: MenuItem;

  private async _applyWindowAppearance(mini: boolean) {
    if (!this.window) return;
    const store = getDefaultStore();
    if (mini === store.get(this._currentlyMiniAtom)) return;
    store.set(this._currentlyMiniAtom, mini);
    const w = this.window;
    const miniHeight = 40;
    await Promise.all([w.setAlwaysOnTop(mini), w.setMaximizable(!mini), w.setSkipTaskbar(!mini)]);
    await invoke("toggle_mini_os_specific_styling", { mini });
    await w.setSize(mini ? new LogicalSize(400, miniHeight) : new LogicalSize(600, 400));
    await w.setMaxSize(mini ? new LogicalSize(3000, miniHeight) : undefined);
    await w.setMinSize(mini ? new LogicalSize(300, miniHeight) : new LogicalSize(300, 300));
    // Ensure window is visible when interacting with it
    await w.setFocus();
  }

  async initialize() {
    this.window = Window.getCurrent();

    // Create separators first so we can reference them
    this.tasksSeparatorStart = await MenuItem.new({ text: "", enabled: false });
    this.tasksSeparatorEnd = await MenuItem.new({ text: "", enabled: false });

    // Setup tray with current task
    this.tray = await TrayIcon.new({
      id: "main-tray",
      icon: await TRAY_IMAGES.Base,
      title: undefined,
      menu: (this.trayMenu = await Menu.new({
        id: "tray-menu",
        items: [
          (this.focus = await MenuItem.new({ text: "Focus", action: () => this.window?.setFocus() })),
          (this.editTodoFile = await MenuItem.new({
            text: "Edit TODO File",
            enabled: false,
            action: () => this.todoPath && openPath(this.todoPath),
          })),
          this.tasksSeparatorStart,
          // Task items will be inserted here dynamically
          this.tasksSeparatorEnd,
          (this.showPlanner = await MenuItem.new({ text: "Show Planner", action: () => this.expandToPlanner() })),
          (this.showTracker = await MenuItem.new({ text: "Show Tracker", action: () => this.collapseToTracker() })),
          await MenuItem.new({ text: "Quit", action: () => exit(0) }),
        ],
      })),
    });
  }

  async setTodoPath(path: string | null) {
    this.todoPath = path ?? undefined;
    await this.editTodoFile?.setEnabled(Boolean(path));
  }

  async setTitle(
    taskContext: TaskContext | null,
    todoPath?: string | null,
    tasks?: Array<{ name: string; complete?: string | boolean }>,
  ) {
    if (todoPath !== undefined) {
      await this.setTodoPath(todoPath);
    }

    if (tasks) {
      await this.updateTaskList(tasks);
    }

    if (taskContext) {
      await this.tray?.setIcon(await TRAY_IMAGES.ZeroWidth);
      // Show heading in title, fallback to task name if no heading
      const title = taskContext.heading ?? taskContext.task.name;
      await this.tray?.setTitle(truncateString(title, 15, "…"));
      await this.window?.setTitle("");
    } else {
      await this.tray?.setIcon(await TRAY_IMAGES.Base);
      await this.tray?.setTitle(null);
      await this.window?.setTitle("");
      // await this.window?.setTitle("Right Now");
    }
  }

  async collapseToTracker() {
    const store = getDefaultStore();
    store.set(this.currentlyMiniAtom, true);
  }

  async expandToPlanner() {
    const store = getDefaultStore();
    store.set(this.currentlyMiniAtom, false);
  }

  async updateTaskList(tasks: Array<{ name: string; complete?: string | boolean }>) {
    // Clean up existing task items
    await Promise.all(
      this.taskMenuItems.map(async ({ menuItem }) => {
        await this.trayMenu?.remove(menuItem);
        await menuItem.close();
      }),
    );
    this.taskMenuItems = [];

    // Find current task index
    const currentTaskIndex = tasks.findIndex((t) => !t.complete);
    if (currentTaskIndex === -1) return;

    const menuItems: TaskMenuItem[] = [];

    // Add previous completed task if exists
    if (currentTaskIndex > 0) {
      const prevTask = tasks[currentTaskIndex - 1];
      const menuItem = await MenuItem.new({
        text: `✓ ${prevTask.name}`,
        enabled: false,
      });
      menuItems.push({ task: prevTask, menuItem });
    }

    // Add current task
    const currentTask = tasks[currentTaskIndex];
    const currentMenuItem = await MenuItem.new({
      text: `→ ${currentTask.name}`,
      enabled: false,
    });
    menuItems.push({ task: currentTask, menuItem: currentMenuItem });

    // Add next tasks
    const nextTasks = tasks.slice(currentTaskIndex + 1, currentTaskIndex + 1 + AppWindows.MAX_NEXT_TASKS);
    for (const task of nextTasks) {
      const menuItem = await MenuItem.new({
        text: `  ${task.name}`,
        enabled: false,
      });
      menuItems.push({ task, menuItem });
    }

    // Insert all menu items at once between separators
    if (this.trayMenu && this.tasksSeparatorStart) {
      const afterId = this.tasksSeparatorStart.id;
      const startPosition = (await this.trayMenu?.items()).findIndex((item) => item.id === afterId);
      if (startPosition !== undefined && startPosition > 0) {
        await this.trayMenu.insert(
          menuItems.map(({ menuItem }) => menuItem),
          startPosition + 1,
        );
        this.taskMenuItems = menuItems;
      }
    }
  }
}

function truncateString(str: string, maxLength: number, ellipsis: string) {
  return str.length > maxLength + ellipsis.length ? str.slice(0, maxLength) + ellipsis : str;
}
