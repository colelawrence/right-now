# Jotai Quick Guide

## Core Concepts

Jotai is a primitive, flexible state management solution for React that uses an atomic model. It provides a simple API similar to `useState` while enabling complex state graphs.

## Basic Usage

### Primitive Atoms

The simplest form is a primitive atom with an initial value:

```ts
import { atom } from "jotai";

const countAtom = atom(0); // PrimitiveAtom<number>
const textAtom = atom("hello"); // PrimitiveAtom<string>
const objAtom = atom({ count: 0 }); // PrimitiveAtom<{ count: number }>
```

### Important Rules on Interfaces

When using atoms with interfaces, proper typing is critical:

```ts
interface InputModel {
  valueAtom: Atom<string>;
  setValue(value: number | string): void;
  validationErrorAtom: Atom<null | string>;
}
```

**WRONG approach** - defining atoms inside the returned object:
```ts
// WRONG: Atoms defined inside the returned object lose their proper types
const model: InputModel = {
  valueAtom: atom(""), // Type becomes constrained by the interface
  validationErrorAtom: atom((get) => /* ... */),
  setValue(value) {
    // ERROR: Type mismatch when trying to set the atom
    store.set(model.valueAtom, String(value));
  },
};
```

**CORRECT approach** - define atoms outside the returned object:
```ts
// CORRECT: Atoms defined outside maintain their proper types
const valueAtom = atom<string>("");
const model: InputModel = {
  valueAtom, // Proper PrimitiveAtom<string> type preserved
  validationErrorAtom: atom((get) => 
    isNaN(Number(get(valueAtom))) ? "Invalid number" : null
  ),
  setValue(value) {
    store.set(valueAtom, String(value)); // Works correctly
  },
};
```

> **Key Rule**: Always declare atoms outside the returned object to ensure proper TypeScript typing, then return them as part of the controller interface.

### Using Atoms in Components

```tsx
import { useAtom, useAtomValue, useSetAtom } from 'jotai'

// Three ways to use atoms in components:
const [count, setCount] = useAtom(countAtom) // Read & write
const text = useAtomValue(textAtom) // Read-only
const setText = useSetAtom(textAtom) // Write-only
```

## Atom Types

### Read-Only (Derived) Atoms
```ts
const doubleAtom = atom((get) => get(countAtom) * 2); // Atom<number>
```

### Read-Write Atoms
```ts
const readWriteAtom = atom(
  (get) => get(countAtom),
  (_get, set, newValue: number) => set(countAtom, newValue),
);
```

### Write-Only Atoms
```ts
// Useful for side effects, validation, or coordinating multiple atom updates
const writeOnlyAtom = atom(
  null,
  (_get, set, newValue: number) => {
    // Validation, side effects, or updating multiple atoms
    set(countAtom, newValue);
    // Other operations...
  }
);
```

> **Key Pattern**: Write-only atoms are valuable when you need to add side effects, validation, or coordinate multiple atom updates when a value changes.

## TypeScript Usage

### Explicit Typing

```ts
const numberAtom = atom<number>(0);
const asyncAtom = atom<Promise<string>>(async () => "data");

// Write-only atom with multiple parameters
const writeOnlyAtom = atom<null, [string, number], void>(
  null,
  (_get, set, str: string, num: number) => {
    // Implementation
  },
);
```

### TypeScript Utilities

```ts
import type { ExtractAtomValue, ExtractAtomArgs, ExtractAtomResult } from "jotai";

type CountValue = ExtractAtomValue<typeof countAtom>; // number
type CountSetArgs = ExtractAtomArgs<typeof writableCountAtom>; // [number]
type CountSetResult = ExtractAtomResult<typeof writableCountAtom>; // void
```
