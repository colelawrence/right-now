# Jotai Refactor Plan

## Goals

- Move from imperative controller calls to reactive atom-based state
- Separate UI concerns from state management
- Make state changes more predictable and traceable
- Reduce prop drilling through component hierarchy

## Phase 1: Define Core Atoms

### Project State Atoms

```typescript
// src/atoms/project.ts
import { atom } from "jotai";
import type { ProjectFile, WorkState } from "../lib/project";

export function createProjectController(
  store: JotaiStore,
  options: ProjectManagerOptions,
): ProjectController {
  const pathAtom = atom<string | null>(null);
  const contentAtom = atom<ProjectFile | null>(null);
  const workStateAtom = atom<WorkState>("planning");
  const stateTransitionsAtom = atom({ startedAt: Date.now() });

  return {
    pathAtom,
    contentAtom,
    workStateAtom,
    stateTransitionsAtom,
    async updateContent(
      fn: (project: ProjectFile) => void | boolean,
    ): Promise<void> {
      // ...
    },
  };
}
```

### Window State Atoms

```typescript
// src/atoms/window.ts
import { atom } from "jotai";
import type { WindowController, WindowOptions } from "../ui";

export function createWindowController(
  store: JotaiStore,
  options: WindowOptions,
): WindowController {
  const isCompactAtom = atom<boolean>(false);
  const taskSummaryAtom = options.taskSummaryAtom;

  // Create derived atoms or computed values in the closure
  const toggleCompactAtom = atom(
    (get) => get(isCompactAtom),
    (get, set, force?: boolean) => {
      const newValue = force ?? !get(isCompactAtom);
      set(isCompactAtom, newValue);
    },
  );

  return {
    isCompactAtom,
    taskSummaryAtom,
    toggleCompact: (force?: boolean) => store.set(toggleCompactAtom, force),
  };
}
```

### Sound Manager Atoms

```typescript
// src/atoms/sounds.ts
import { atom } from "jotai";
import type { SoundManagerOptions, SoundPackController } from "../ui";

export function createSoundController(
  store: JotaiStore,
  options: SoundManagerOptions,
): SoundPackController {
  const nameAtom = atom<string>("");
  const isDefaultAtom = atom<boolean>(false);
  const soundPackIdAtom = options.projectSoundPackIdAtom;

  return {
    nameAtom,
    isDefaultAtom,
    soundPackIdAtom,
  };
}
```

### Task State Atoms

```typescript
// src/atoms/tasks.ts
import { atom } from "jotai";
import type { TaskController } from "../ui";

export function createTaskController(
  store: JotaiStore,
  initialTitle: string,
): TaskController {
  const titleAtom = atom<string>(initialTitle);
  const completeAtom = atom<boolean>(false);

  return {
    titleAtom,
    completeAtom,
  };
}
```

## Phase 2: Controller Refactor

1. Convert `ProjectManager` to use atoms internally

   - Replace direct state updates with atom updates
   - Keep file watching and disk I/O logic

2. Convert `AppWindows` to consume window atoms
   - Remove local state management
   - React to atom changes for window appearance

## Phase 3: Component Updates

1. Replace `useLoadedProject` with atom subscriptions:

```typescript
// Before
const loaded = useLoadedProject(projectManager);
// After
const [projectContent] = useAtom(projectController.contentAtom);
const [workState] = useAtom(projectController.workStateAtom);
```

2. Update state change handlers:

```typescript
// Before
const handleStateChange = async (newState: WorkState) => {
  await projectManager.updateWorkState(newState);
  // ... sound effects, window changes
};
// After
const handleStateChange = async (newState: WorkState) => {
  setWorkState(newState); // Atom update triggers subscribers
};
```

## Phase 4: Main.tsx Updates

1. Initialize atoms instead of controllers:

```tsx
async function initializeApp() {
  const store = getDefaultStore();
  // Set up initial atom values
  store.set(projectController.pathAtom, await projectStore.getLastActiveProject());
  // Initialize React with just the store
  ReactDOM.createRoot(...).render(
    <JotaiProvider store={store}>
      <AppReady />
    </JotaiProvider>
  );
}
```

2. Move controller initialization logic into custom hooks:

```typescript
function useProjectInitialization() {
  const [path] = useAtom(projectController.pathAtom);
  useEffect(() => {
    // File watching, disk I/O setup
  }, [path]);
}
```

## Benefits

- Clearer data flow: All state changes go through atoms
- Better testing: Can mock atoms instead of entire controllers
- Reduced complexity: Components only need to know about atoms
- TypeScript support: Better type inference for state updates

## Migration Strategy

1. Start with one controller (e.g., ProjectManager)
2. Add atoms alongside existing state
3. Gradually move components to use atoms
4. Remove old state management
5. Repeat for other controllers

## Considerations

- Keep file I/O and side effects in dedicated modules
- Use derived atoms for computed values
- Consider atomFamily for managing multiple windows
- Add debug logging through atom callbacks
