# CLI Installation and Usage

Right Now includes a `todo` command-line interface for managing terminal sessions from your shell. This document describes how the CLI works, how to install it, and how the bundled binaries interact.

## Overview

The Right Now app bundle includes four command-line binaries:

- **`rn-desktop-2`** — The main Tauri desktop app
- **`right-now-daemon`** — Background daemon that manages terminal sessions
- **`todo`** — The main CLI for interacting with sessions
- **`todo-shim`** — A small bootstrap binary that finds and executes `todo`

## How It Works

```
User runs 'todo'
    ↓
todo-shim (installed in ~/.local/bin/)
    ↓ (reads cli-paths.json or searches fallback locations)
Real 'todo' binary (inside app bundle)
    ↓ (connects to Unix socket or starts daemon)
right-now-daemon (manages PTY sessions)
```

### Binary Discovery

When you run a `todo` command, the system uses a multi-layer discovery strategy:

1. **todo-shim** (installed in `~/.local/bin/todo`) is found via your `$PATH`
2. **Shim reads** `~/Library/Application Support/Right Now/cli-paths.json` (written by the app on startup)
3. **Shim executes** the real `todo` binary inside the app bundle
4. **todo binary** connects to the daemon or starts it using:
   - Binary next to `current_exe()` (typical for bundled releases)
   - Path from `cli-paths.json`
   - Platform-specific fallback locations (`/Applications/Right Now.app/Contents/MacOS/`, etc.)

This design ensures:
- The CLI works even if the app is moved or updated
- No hard-coded paths to dev directories (`cargo run`)
- Clean separation between user-facing command name and implementation

## Installing the CLI

### Via the App (Recommended)

The Right Now desktop app includes a **Tools** menu item to install/uninstall the CLI:

1. Open Right Now
2. Click **Tools → Install 'todo' CLI...** (or **Tools → Uninstall 'todo' CLI...** if already installed)
3. The app installs `todo-shim` to `~/.local/bin/todo`
4. Add `~/.local/bin` to your `$PATH` if not already present

### Manual Installation

You can also install directly from the app bundle:

```bash
# Copy the shim to your local bin directory
mkdir -p ~/.local/bin
cp "/Applications/Right Now.app/Contents/MacOS/todo-shim" ~/.local/bin/todo
chmod +x ~/.local/bin/todo

# Add to PATH (if needed)
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

### Verifying Installation

```bash
# Check that 'todo' is found
which todo
# Should output: /Users/your-username/.local/bin/todo

# Run help to verify it works
todo help

# List sessions (starts daemon if needed)
todo list
```

## Uninstalling the CLI

### Via the App

1. Open Right Now
2. Click **Tools → Uninstall CLI**
3. The shim is removed from `~/.local/bin/`

### Manual Uninstall

```bash
rm ~/.local/bin/todo
```

This removes only the shim; the app bundle remains intact.

## Custom CLI Names

The app supports customizing the CLI command name (e.g., `td`, `tasks`, `rn`) via the Tools menu. When you change the name:

1. The old shim binary is removed
2. A new shim is installed with the chosen name
3. Your shell sessions pick up the new command immediately (if `~/.local/bin` is in `$PATH`)

Configuration is stored in:
```
~/Library/Application Support/Right Now/shim-config.json
```

## Daemon Management

The `todo` CLI automatically starts `right-now-daemon` if it's not running. The daemon:

- Listens on a Unix socket (macOS default): `~/.right-now/daemon.sock`
- Manages PTY sessions for active tasks
- Persists session state across restarts
- Emits notifications for attention events

You don't need to manually start or stop the daemon; the CLI and desktop app handle this automatically.

## Shell Integration (Optional)

To show the active session in your shell prompt:

```bash
todo shell-integration --install
```

This adds a snippet to your shell RC file (`~/.zshrc`, `~/.bashrc`, etc.) that displays:

```
[#42: Task name] > your-command
```

Uninstall with:

```bash
todo shell-integration --uninstall
```

## Troubleshooting

### "Could not find right-now-daemon binary"

If you see this error, the CLI can't locate the daemon. Check:

1. Is the Right Now app installed in `/Applications/` or `~/Applications/`?
2. Does `~/Library/Application Support/Right Now/cli-paths.json` exist and point to valid binaries?
3. Try running the app once to regenerate `cli-paths.json`

### "Daemon not running, attempting to start..."

This is normal on first use. The CLI auto-starts the daemon and waits for the socket to appear.

If the daemon fails to start:
- Check console logs: `log show --predicate 'process == "right-now-daemon"' --last 1m`
- Verify the binary is executable: `ls -l "/Applications/Right Now.app/Contents/MacOS/right-now-daemon"`

### CLI works but sessions don't persist

Sessions are stored in:
```
~/.right-now/sessions.json
```

If this file (or its parent directory) is deleted or permissions are wrong, session tracking will fail.

## Platform Notes

### macOS

- Default shim location: `~/.local/bin/`
- CLI config directory (cli-paths.json, shim-config.json): `~/Library/Application Support/Right Now/`
- Daemon data directory (socket, pid, sessions.json): `~/.right-now/`
- App bundle: `/Applications/Right Now.app/`

### Windows

- Default shim location: `%USERPROFILE%\bin\`
- CLI config directory (cli-paths.json, shim-config.json): `%APPDATA%\Right Now\`
- Daemon data directory (socket, pid, sessions.json): `%APPDATA%\Right Now\` (default)
- Shim binary: `todo.exe`

### Linux

- Default shim location: `~/.local/bin/`
- CLI config directory (cli-paths.json, shim-config.json): `~/.config/right-now/`
- Daemon data directory (socket, pid, sessions.json): `$XDG_RUNTIME_DIR/right-now/` (if set) or `~/.right-now/`
- App bundle: `/opt/Right Now/` or `~/.local/share/Right Now/`

## Development Notes

When running from source (via `cargo`), the CLI binaries are **not** bundled. You can still test the `todo` CLI:

```bash
# Build the todo binary
cargo build --bin todo

# Run directly (daemon auto-start works by spawning the sibling binary)
./target/debug/todo list
```

For release testing, use the smoke test:

```bash
bun run smoke:bundle
```

This verifies all binaries are present and the `todo` CLI can execute successfully.
