# Repository Guidelines

## Project Stage & Backwards Compatibility
- **This app is not currently in use by end users.** It is an internal/experimental codebase.
- **Do not spend time on backwards compatibility** for persisted data (settings, state files, caches, etc.) unless a task explicitly asks for it.
- It is OK to make breaking changes to on-disk formats (e.g. `ProjectStore.json`, session registries, TODO frontmatter). Prefer the simplest correct implementation.
- If a change would break existing local data, prefer one of these approaches:
  - require a clean slate (document which directory/file to delete/reset), or
  - fail fast with a clear error message instructing how to reset.
- Avoid adding migration layers, schema versioning, multi-format parsing, or “support old versions” code paths by default.

## Project Structure & Module Organization
- `src/` holds the React UI and shared frontend logic.
- `src-tauri/` is the Rust backend and Tauri configuration.
- `test/` contains Bun E2E tests and harness helpers.
- `public/` stores static assets for Vite.
- `docs/` contains additional project documentation.
- `dist/` and `dist-test/` are build outputs.

## Build, Test, and Development Commands
- `bun run dev`: start the Vite dev server for the UI.
- `bun run build`: typecheck and build the production bundle.
- `bun run preview`: serve the built Vite bundle locally.
- `bun run tauri`: run Tauri CLI commands (e.g., `bun run tauri dev`).
- `bun run typecheck`: run `tsc --noEmit`.
- `bun run test`: run all Bun tests.
- `bun run test:unit`: run unit tests in `src/**/*.test.ts`.
- `bun run test:e2e`: run E2E tests in `test/integration/**/*.test.ts`.
- `bun run tauri:test`: launch the test harness app.

## Coding Style & Naming Conventions
- Formatting and linting use Biome (`biome.json`).
- Indentation is 2 spaces; max line width is 120.
- Prefer `*.test.ts` naming for tests.
- Use TypeScript + React patterns for UI code; Rust modules live under `src-tauri/src/`.

## Testing Guidelines
- Unit tests run via Bun; E2E tests talk to the real Tauri app through the test harness.
- The harness uses `~/rightnow-test/` for temp data and a Unix socket at `$TMPDIR/rightnow-test-harness.sock`.
- Clear event history per test and use the TestClock for deterministic time control (see `TESTING.md`).

## Commit & Pull Request Guidelines
- Commit messages are short, imperative, and sentence-cased (e.g., “Add …”, “Fix …”, “Refactor …”).
- Keep commits focused; avoid mixing refactors with feature changes when possible.
- PRs should describe what changed and why, include test commands run, and attach UI screenshots when visuals change.

## Configuration Notes
- `lint-staged` runs Biome on JS/TS/CSS/HTML/JSON and Rust checks/formatting on `.rs` files.
- Tauri tests use a special config at `src-tauri/tauri.test.conf.json`.
