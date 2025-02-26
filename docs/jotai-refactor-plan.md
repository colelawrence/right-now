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
import type { ProjectController, ProjectManagerOptions } from "../ui";

export function createProjectController(
  store: JotaiStore,
  options: ProjectManagerOptions,
): ProjectController {
  // Define atoms outside the returned object to ensure proper typing
  const pathAtom = atom<string | null>(null);
  const contentAtom = atom<ProjectFile | null>(null);
  const workStateAtom = atom<WorkState>("planning");
  const stateTransitionsAtom = atom({ startedAt: Date.now() });
  
  // Define write-only atoms for operations
  const updateContentAtom = atom(
    null,
    async (_get, set, fn: (project: ProjectFile) => void | boolean): Promise<void> => {
      const content = _get(contentAtom);
      if (!content) return;
      
      const newContent = structuredClone(content);
      if (fn(newContent) === false) return;
      
      set(contentAtom, newContent);
      // Additional side effects like file I/O would go here
    }
  );

  return {
    pathAtom,
    contentAtom,
    workStateAtom,
    stateTransitionsAtom,
    updateContent: (fn) => store.set(updateContentAtom, fn),
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
  // Define atoms outside the returned object
  const isCompactAtom = atom<boolean>(false);
  
  // Reference provided atoms
  const taskSummaryAtom = options.taskSummaryAtom;

  // Create a write-only atom for toggling compact state
  const toggleCompactAtom = atom(
    null,
    (_get, set, force?: boolean) => {
      const currentValue = _get(isCompactAtom);
      const newValue = force !== undefined ? force : !currentValue;
      set(isCompactAtom, newValue);
    }
  );

  return {
    isCompactAtom,
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
  // Define atoms outside the returned object
  const nameAtom = atom<string>("");
  const isDefaultAtom = atom<boolean>(false);
  
  // Reference provided atoms
  const soundPackIdAtom = options.projectSoundPackIdAtom;
  
  // Create a write-only atom for toggling default state
  const setIsDefaultAtom = atom(
    null,
    (_get, set, value: boolean) => {
      set(isDefaultAtom, value);
      // Additional side effects could go here
    }
  );

  return {
    nameAtom,
    isDefaultAtom,
    setIsDefault: (value: boolean) => store.set(setIsDefaultAtom, value),
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
  // Define atoms outside the returned object
  const titleAtom = atom<string>(initialTitle);
  const completeAtom = atom<boolean>(false);
  
  return {
    titleAtom,
    completeAtom,
    setTitle: (title: string) => store.set(titleAtom, title),
    setComplete: (complete: boolean) => store.set(completeAtom, complete),
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
