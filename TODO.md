---
pomodoro_settings:
  work_duration: 25
  break_duration: 5
---

# Perf

- [ ] Use jotai atoms to control more of the state with MVVM pattern

# Tray & Menu

- [x] Add "Edit" menu item for opening the TODO file
- [ ] Add "Break"/"Resume working" menu item (dep on jotai/mvvm)
- [ ] Add current task controls as a submenu with "complete" (dep on jotai/mvvm)
- [x] Set the task bar to the section heading

# Cues

- [x] Play audio cue when work starts
- [x] Play audio cues for start, and warning before end
- [x] Notify user when time is up
- [x] Play audio cue when the user manually checks off a TODO item
First get the sound on fal.ai
- [ ] Control OS music playback when starting a break
- [ ] Show a flashing state when the timer goes past the allocated time
- [ ] Play with adaptive soundtracks like synthet
https://youtu.be/8WK02utnjj8


# Load Screen

- [ ] Simplify & show recent TODO lists to open

# Tracker Styling

- [ ] Resize text to fit the available area
- [ ] Make a Tauri window appear above fullscreen apps in macOS like Zoom
      we understand how to do this, but it's finicky... so it's not in place
      Use https://github.com/ahkohd/tauri-nspanel to swizzle the window in Rust
      https://chatgpt.com/share/67b74c68-a140-8009-820d-f1dfa7b3a4b3does
- [x] Copy text of current task
- [ ] Edit text of current task (dep on jotai/mvvm)
- [ ] Render markdown from details in the scroll down
- Is it possible to enable some kind of "scroll" to increase the window size?
- [x] Disable double click to maximize
      You can add an event listener on the relevant element which then calls `startDragging()`. The tauri-drag-region attribute does the same under the hood (but with the addition of the double-click handler).
      See https://v2.tauri.app/learn/window-customization/#manual-implementation-of-data-tauri-drag-region

# Window management

- [ ] Expand the app without stopping the session
- [ ] Compact mode should stay aligned to bottom coords
- [ ] Prevent making window larger and not reverting when there is a scrollbar issue

# TODO file

- [ ] Only set the values in frontmatter if they are different from the default (e.g. for pomodoro_settings)
- [ ] Quick switch between TODO files and toggle between them (dep on jotai atoms)
- [ ] Customize "in-progress theme" per TODO list
- [ ] Customize the "editor" app
- [ ] Customize the Soundpack
- [ ] Customize the shortcuts to specific apps in the frontmatter? (e.g. open Framer?)
- [ ] #Hmm: Set a todo list to pick TODOs randomly

# Customization

- [ ] List Soundpacks

# Renata

- [ ] Create new virtual TODO lists
Can be stored in the AppData folder and named uniquely with a two part name ISO Date to day precision + "TODO"
e.g. 2025-02-13-TODOs.md
- [ ] Ability to rename TODO lists, attempts to rename them on disk before confirming available name
- [ ] Ability to add, retitle, and reorder TODO items from UI
Show unrecognized markdown inbetween items while reordering?

# DONE

- [x] Don't lose the newlines between tasks and headings
- [x] Load Soundpacks from zip files
- [x] Guide on Soundpack development
- [x] Soundpack generation scripts with fal.ai
- [x] Investigate if tabler icons is being included entirely in dev
- Consider making vite do some tree shaking in development mode?
- [x] Synchronize "compact" state with the window state (derive some reactive varaible?)
- [x] Make it not go smaller when it resets to next todo item
- [x] Fix nested TODO items (create tests for it?)
- [x] Fix focus jank
- [x] Enable completing a task item from the UI
