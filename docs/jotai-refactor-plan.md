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
export const projectControllerAtom = atom<ProjectController>({
  pathAtom: atom<string | null>(null),
  contentAtom: atom<ProjectFile | null>(null),
  workStateAtom: atom<WorkState>("planning"),
  stateTransitionsAtom: atom({ startedAt: Date.now() }),
});
```

### Window State Atoms

```typescript
// src/atoms/window.ts
export const windowControllerAtom = atom<WindowController>({
  isCompactAtom: atom(false),
  toggleCompact: (force?: boolean) => {
    // Implementation
  },
});
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
