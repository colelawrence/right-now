# Context Resurrection

**Pick up exactly where you left off—even when you've forgotten where that was.**

---

## The Problem

It's Monday morning. You're staring at your screen, coffee in hand, trying to remember what the hell you were doing on Friday.

There's a terminal window with output from a test run. Did it pass? Did it fail? You scroll up. The command that triggered it is buried somewhere in the history. You ran `cargo test` four times—but which run matters? What were you testing?

Your editor shows five tabs open. You don't remember which file had the breakthrough. Or the bug. You click through them like a detective at your own crime scene.

Your task list says "fix API timeout bug." Thanks, past-you. Very helpful. What did you *try*? What *almost* worked?

You spend the next twenty minutes excavating your own work. Scrolling. Re-reading. Piecing together fragments. By the time you're back in the zone, half your morning focus is gone.

**You've been here before. We all have.**

Gloria Mark's research at UC Irvine quantified what we all feel: after an interruption, it takes an average of **23 minutes and 15 seconds** to fully return to a task. Not just to resume typing—to *reconstruct the mental model* you had before the interruption shattered it.

For complex debugging sessions or multi-step refactoring, that number climbs even higher. Some engineers report losing entire mornings just trying to "find where they were."

> **The cruelest irony**: You did the hard thinking. You made progress. You *knew* what to do next. Then your brain garbage-collected the entire state overnight.

The problem isn't the interruption. It's that nothing preserved what you knew.

- Your terminal doesn't remember *why* you ran those commands
- Your editor doesn't know which file mattered when inspiration struck
- Your task list says *what*, but not *where you were in the process*

Modern tools assume continuity. They assume you'll remember. They're wrong.

---

## The Vision

Now imagine a different Monday morning.

You open Right Now. Before the dread can set in—before you start the familiar archaeology of your own work—a card appears. It knows exactly where you were:

```
┌────────────────────────────────────────────────────────────────────┐
│  🔮 CONTEXT RESURRECTION                                           │
│                                                                    │
│  You were working on: "Fix API timeout bug in checkout service"   │
│  Last active: Friday 5:47pm (2 days ago)                          │
│  Duration: 47 minutes of focused work                             │
│                                                                    │
│  ────────────────────────────────────────────────────────────────  │
│                                                                    │
│  📟 Terminal Session #42                                           │
│  "You ran cargo test 4 times. First three failed with a timeout   │
│   in checkout_handler. After adding #[tokio::timeout], the fourth │
│   run passed 23/24 tests—one flaky test remains."                 │
│                                                                    │
│  📝 Your note from Friday:                                         │
│  "The timeout was 30s, trying 60s. If that works, need to         │
│   refactor the retry logic next."                                 │
│                                                                    │
│  💻 Editor state: 3 files open                                     │
│   → checkout_handler.rs:142 (cursor position)                     │
│   → config.toml                                                   │
│   → test_checkout.rs                                              │
│                                                                    │
│  ┌──────────────────┐  ┌──────────────────┐                       │
│  │  ▶ Resume Work   │  │  📄 View Details │                       │
│  └──────────────────┘  └──────────────────┘                       │
└────────────────────────────────────────────────────────────────────┘
```

You read it. And you *remember*. Not because you forced yourself to—the card does the remembering for you. The dread dissolves into recognition: *Oh right, the retry logic. I was so close.*

You click "Resume Work."

The terminal session reattaches. Your test output from Friday scrolls into view. VS Code opens with your cursor blinking at line 142 of `checkout_handler.rs`—exactly where it was. The Pomodoro timer starts. You type your first keystroke within thirty seconds.

Not 23 minutes. Thirty seconds.

**That shift—from dread to relief, from archaeology to action—is what Context Resurrection delivers.**

---

## How It Works

Context Resurrection operates in three layers: **Breadcrumbs** (capturing state), **Briefings** (summarizing what happened), and **Editor Snapshots** (preserving workspace).

### 1. Context Breadcrumbs: Automatic State Capture

Right Now continuously tracks your working state. When you transition away from a task—whether explicitly or through inactivity—it snapshots everything relevant.

**Triggers for snapshot capture:**
| Trigger | Type | What happens |
|---------|------|--------------|
| You click "End Session" | Explicit | Full snapshot with optional note prompt |
| You switch to a different task | Explicit | Snapshot current, load snapshot for new task |
| Pomodoro timer ends (break starts) | Explicit | Snapshot at natural transition point |
| No terminal input/output for 10 minutes | Implicit | Background snapshot, no UI interruption |
| App goes to background for 30+ seconds | Implicit | Lightweight snapshot of current state |
| System sleep/shutdown detected | Implicit | Emergency snapshot before process ends |

**What gets captured:**

```typescript
interface ContextSnapshot {
  // Core identification
  taskKey: string;                    // e.g., "fix-api-timeout"
  projectPath: string;                // /Users/cole/projects/checkout
  capturedAt: number;                 // Unix timestamp
  captureReason: CaptureReason;       // "explicit" | "timeout" | "task_switch" | "break" | "sleep"

  // Time tracking (already in Right Now)
  workDuration: number;               // ms of focused time on this task
  pomodoroState: WorkState;           // "working" | "break" | "planning"
  
  // Terminal state (from existing SessionClient)
  terminalSession?: {
    sessionId: number;
    status: SessionStatus;            // Running | Stopped | Waiting
    tailOutput: string;               // Last 10KB of terminal output
    lastCommand?: string;             // Most recent command entered
  };
  
  // User context
  userNote?: string;                  // Optional note at capture time
  
  // Editor state (new capability)
  editorSnapshot?: EditorWorkspaceSnapshot;
}
```

**Storage**: Snapshots live in `~/.rightnow/snapshots/<project-hash>/<task-key>/`, stored as JSON with optional compressed terminal output. Each task retains the last 5 snapshots to enable history browsing.

### 2. Resumption Briefings: AI-Powered Summary

When you return to a task with terminal history, Right Now generates a human-readable briefing. This isn't a dump of logs—it's a narrative of what you did and what happened.

**The briefing prompt structure:**

```
Given this terminal session history:
- Session started: [timestamp]
- Commands run: [list with outputs]
- Final status: [Running/Stopped/Waiting]
- Last attention signal: [if any]

Generate a 2-3 sentence summary that tells the developer:
1. What they were trying to accomplish
2. What succeeded or failed
3. Where they should pick up

Be specific about error messages, test counts, and build results.
Keep it conversational and actionable.
```

**Example briefings:**

> "You ran `cargo build` 3 times. First two failed with missing `serde` feature flag. After adding it to Cargo.toml, the third build succeeded. No tests were run."

> "You started a database migration (`rails db:migrate`), which completed successfully. Then you ran the test suite—142 passed, 3 failed in `UserAuthSpec`. The failures look like timezone-related assertions."

> "Session was interrupted while waiting for `npm install`. The process was killed before completion. You'll need to run it again."

**Privacy and locality:**
- All AI summarization happens **on-device** using a small local model (Llama 3 8B or similar via llama.cpp)
- Terminal output never leaves your machine
- Users can disable AI briefings entirely; they'll still see raw tail output

### The Secret Weapon: Your Note to Future Self

This is the feature that transforms Context Resurrection from "nice automation" to "I can't live without this."

AI can summarize your terminal output. It can tell you "3 tests failed" and "build succeeded." What it *cannot* do is read your mind. It doesn't know:

- "I think the real problem is in the retry logic, not the timeout"
- "Asked Sarah about this—waiting for her response in Slack"
- "Try disabling caching next, I have a hunch"
- "IGNORE THE FAILING TEST—it's flaky and unrelated"

**This is human context. Only you have it. And it's the most valuable context of all.**

Right Now prompts you for a quick note when you end a session or step away. It takes 10 seconds. It saves 10 minutes.

**The Note Prompt Flow:**

When you explicitly end a session or a Pomodoro ends, a small modal appears:

```
┌────────────────────────────────────────────────────────────────┐
│  📝 Quick note before you go? (optional)                       │
│                                                                │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ Try 60s timeout next. If that works, refactor retry     │  │
│  │ logic. Check Sarah's Slack reply first.                 │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                │
│  ┌─────────────┐  ┌─────────────┐                             │
│  │   Skip      │  │  Save note  │                             │
│  └─────────────┘  └─────────────┘                             │
└────────────────────────────────────────────────────────────────┘
```

You don't have to write a novel. One sentence is enough. "Try X next" or "Problem is in Y" or "Waiting on Z."

**Why this matters more than AI summaries:**

The AI briefing tells you *what happened*. Your note tells you *what to do next*. The combination is complete: machine memory + human insight.

Users who adopt the note habit report that returning to work feels like picking up a conversation with a smarter version of themselves. They left breadcrumbs only they could leave.

**Note visibility:**
- Notes appear prominently in the Resurrection Card (not buried in details)
- Notes are searchable across all your tasks and projects
- Notes can be edited after the fact if you remember something later

This is the difference between "Right Now summarized my terminal" and "Right Now understands my work."

### 3. Editor Workspace Snapshots

When you leave a task, Right Now captures your editor state. When you return, it can restore exactly where you were.

**Captured editor state:**

```typescript
interface EditorWorkspaceSnapshot {
  editor: "vscode" | "cursor" | "zed" | "neovim";  // Detected editor
  capturedAt: number;
  
  // Open files and their state
  openFiles: Array<{
    path: string;           // Relative to project root
    cursorLine: number;
    cursorColumn: number;
    scrollPosition: number; // Line at top of viewport
    isDirty: boolean;       // Unsaved changes
  }>;
  
  // Active file (the one with focus)
  activeFile?: string;
  
  // Editor layout (for split panes)
  layout?: EditorLayout;
  
  // Optional: visible selections/highlights
  selections?: Array<{
    file: string;
    startLine: number;
    endLine: number;
  }>;
}
```

**VS Code integration (first-class support):**

Right Now ships a companion VS Code extension that:
1. Listens for workspace changes and reports state to Right Now via IPC
2. On resurrection, opens the project and restores file positions
3. Preserves unsaved changes (marks files as dirty at correct lines)

The extension communicates over a local Unix socket, with zero network access.

**Fallback for other editors:**
- For editors with session restore (Cursor, Zed): Right Now triggers their native restore command
- For terminal editors (Neovim): Right Now can write a Session.vim file
- For unsupported editors: Right Now shows "Open in editor" with a list of files to manually restore

### The Resurrection Card UI

The Resurrection Card appears when:
1. You return to the app after an extended absence (>1 hour)
2. You manually switch to a task that has a saved snapshot
3. You click "Resume" on a stopped terminal session

**Card states:**

```
┌─────────────────────────────────────────────────────────────────┐
│ State: FULL CONTEXT                                             │
│ Shows: Task name, time since last active, duration, terminal    │
│        summary, user note, editor files, Resume/Details buttons │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ State: MINIMAL CONTEXT                                          │
│ Shows: Task name, time since last active, duration              │
│ (No terminal session or editor state to show)                   │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│ State: GENERATING BRIEFING                                      │
│ Shows: Spinner while AI summarizes terminal output              │
│ (Typically <3 seconds with local model)                         │
└─────────────────────────────────────────────────────────────────┘
```

**Interactions:**
- **Resume Work**: Starts Pomodoro timer, reattaches terminal, triggers editor restore
- **View Details**: Expands to show raw terminal tail, full file list, snapshot history
- **Add Note**: Opens a quick input to annotate before resuming
- **Dismiss**: Closes card, keeps snapshot available in task drawer

---

## User Scenarios

### Scenario 1: Monday Morning After the Weekend

**The situation**: You've been away since Friday afternoon. Right Now was the last thing you closed before leaving.

**What happens**:
1. You open Right Now on Monday morning
2. A Resurrection Card immediately appears for "Refactor payment validation"
3. The card shows:
   - "Last active Friday 5:12pm (3 days ago)"
   - "62 minutes of focused work"
   - AI briefing: "You were adding Stripe validation to the checkout flow. `cargo test` passed after fixing the signature mismatch. You left a note: 'Need to add error handling for declined cards next.'"
   - Editor state: `payment_validator.rs:87`, `checkout_handler.rs`, `stripe_client.rs`
4. You click "Resume Work"
5. Terminal session #23 reattaches with scroll-back. VS Code opens with cursor at line 87.
6. You're back in the flow within a minute.

### Scenario 2: Returning from a Meeting Mid-Debug

**The situation**: You were deep in a debugging session when a calendar notification pulled you into a 1-hour meeting.

**What happens**:
1. Before the meeting, Right Now detected 30 seconds of inactivity and created an implicit snapshot
2. During the meeting, you jotted down a note on your phone (via mobile companion or just mental note)
3. Returning, you open the compact tracker view
4. The task shows a subtle "Resume from snapshot" indicator
5. You click it. A compact Resurrection Card slides up:
   - "Terminal #45 was running. Last output: 'test user_auth::test_token_refresh FAILED'"
   - "3 files were open. Active: `auth_service.rs:221`"
6. You type a quick note: "Try mocking the clock instead of using real time"
7. Resume. Your debugging environment reconstitutes instantly.

### Scenario 3: Context Switching Between Two Active Tasks

**The situation**: You're juggling two urgent tasks—a production bug and a feature deadline. You need to switch between them throughout the day.

**What happens**:
1. You're working on "Fix production memory leak" with terminal session #12
2. An urgent Slack asks you to review "Add OAuth provider" before deploy
3. You click on the OAuth task in Right Now's task list
4. Right Now:
   - Snapshots current task (memory leak) with terminal state
   - Shows a compact resurrection view for OAuth (you worked on it yesterday)
   - "You ran `npm test` 3 times, all passing. Last change was to `oauth_config.ts`."
5. You do the review, add a note: "Approved, just needs error string fix"
6. You click back to "Fix production memory leak"
7. Resurrection Card shows your memory leak context. Terminal reattaches. You're exactly where you were.

**The power**: Context switching cost drops from 20+ minutes to under 60 seconds. The app remembers so you don't have to.

### Scenario 4: A Writer Returning to a Draft

**The situation**: You're a technical writer working on API documentation. You stepped away Wednesday to handle a urgent support escalation. Now it's Friday and you need to finish the draft.

**What happens**:
1. You open Right Now and see the Resurrection Card for "Write v2 Migration Guide"
2. The card shows:
   - "Last active Wednesday 3:15pm (2 days ago)"
   - "94 minutes of focused work across 3 sessions"
   - Your note from Wednesday: "Stuck on the auth section. Need to verify the token refresh flow matches the actual implementation—asked Sarah in #platform"
   - Terminal: "You were running `mdbook serve` to preview. Last file edited: `docs/migration/authentication.md`"
   - Editor state: 4 markdown files open, cursor at line 47 of authentication.md
3. You click "Resume Work"
4. The preview server restarts. Your editor opens to exactly where you were—mid-sentence in the auth section.
5. You check Slack, find Sarah's answer from Wednesday, and keep writing.

**The power**: Context Resurrection isn't just for code. Any focused work that involves files, terminals, and the passage of time benefits. Writers, designers running build tools, PMs using CLIs—anyone who works in a project directory can pick up where they left off.

---

## Why Right Now Is Uniquely Positioned

**What returning to work looks like today vs. with Context Resurrection:**

| Today (without CR) | With Context Resurrection |
|--------------------|---------------------------|
| "What was I working on?" → Check task list | Task + context card appears automatically |
| "Where was I in the terminal?" → Scroll through history | AI briefing: "You ran 4 tests, 3 failed, 1 passed" |
| "What did I already try?" → Re-read notes, hope you documented | Your note from Friday: "Trying 60s timeout next" |
| "Which file had the bug?" → Click through 5 open tabs | Editor restores cursor to line 142 |
| **Time to resume: 15-30 minutes** | **Time to resume: 30 seconds** |

Context Resurrection isn't a standalone tool waiting to be built. It requires a specific intersection of capabilities that Right Now already has:

### 1. Task Awareness
Right Now knows *what you're working on*—not just which files are open, but the semantic task ("Fix API timeout bug"). This grounds the snapshot in meaning. A generic session manager captures files; Right Now captures *intent*.

**Code reference**: `ProjectStateEditor.ts` (lines 1-50) defines tasks as first-class markdown blocks with names, completion status, and session linkage.

### 2. Terminal Session Ownership
The `right-now-daemon` manages PTY sessions tied to tasks. Right Now already captures terminal output and supports detach/reattach. Context Resurrection extends this with snapshot-triggered captures and AI summarization.

**Code reference**: `SessionClient.ts` (lines 112-130) shows the existing `continueSession` method with tail replay—the primitive that powers resumption briefings.

### 3. Timing Data
Right Now tracks work duration, Pomodoro cycles, and state transitions. This data feeds the Resurrection Card—"47 minutes of focused work" means something. It's not just clock time; it's attention time.

**Code reference**: `store.ts` (lines 5-12) defines `TimingDetails` with per-task time tracking already in place.

### 4. Cross-Concern Orchestration
A standalone tool would need to:
- Integrate with your task manager
- Integrate with your terminal
- Integrate with your editor
- Integrate with your time tracker

Right Now already ties these together. The resurrection logic lives at the intersection, with natural trigger points (task switch, Pomodoro end, session stop).

**Why others can't easily replicate this:**
- **Terminal emulators** (iTerm, Warp) don't know your tasks
- **Task managers** (Linear, Jira) don't know your terminal state
- **Editors** (VS Code) don't know your work duration or session history
- **Pomodoro apps** don't know anything technical

Right Now is the only app that sees the full picture.

---

## Technical Architecture

### Data Model

```
~/.rightnow/
├── snapshots/
│   └── <project-hash>/
│       └── <task-key>/
│           ├── snapshot-1738793421.json     # Snapshot metadata
│           ├── snapshot-1738793421.term.gz  # Compressed terminal output
│           └── latest.json                  # Symlink to most recent
├── briefings/
│   └── <snapshot-id>.txt                    # Cached AI briefings
└── settings.json                            # User preferences for CR
```

**Snapshot retention policy:**
- Keep last 5 snapshots per task (configurable)
- Auto-prune snapshots older than 30 days for completed tasks
- Manual "pin" option to preserve important snapshots indefinitely
- Estimated storage: ~50KB per snapshot (terminal output dominates)

### Integration Points

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Right Now Core                             │
│                                                                     │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐            │
│  │ ProjectStore│    │SessionClient│    │ TimerLogic │            │
│  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘            │
│         │                  │                  │                    │
│         └──────────────────┼──────────────────┘                    │
│                            │                                       │
│                   ┌────────▼────────┐                              │
│                   │ ContextCapture  │  ← New module                │
│                   │    Service      │                              │
│                   └────────┬────────┘                              │
│                            │                                       │
│         ┌──────────────────┼──────────────────┐                    │
│         │                  │                  │                    │
│  ┌──────▼──────┐    ┌──────▼──────┐    ┌──────▼──────┐            │
│  │ Snapshot    │    │ Briefing    │    │ Editor      │            │
│  │ Storage     │    │ Generator   │    │ Integration │            │
│  └─────────────┘    └─────────────┘    └─────────────┘            │
│                            │                                       │
│                   ┌────────▼────────┐                              │
│                   │  Local LLM      │  (llama.cpp)                 │
│                   │  (on-device)    │                              │
│                   └─────────────────┘                              │
└─────────────────────────────────────────────────────────────────────┘

External:
  ┌─────────────────┐
  │ VS Code         │  ← Extension communicates via Unix socket
  │ Extension       │
  └─────────────────┘
```

### Privacy Guarantees

Context Resurrection is designed local-first:

1. **No network for snapshots**: All data stays on-device. No telemetry, no sync.
2. **On-device AI**: Briefings generated by a local model (no API calls).
3. **Explicit data locations**: Users can find and delete their data easily.
4. **Encryption at rest**: Sensitive terminal output can be encrypted with user password (opt-in).
5. **Audit log**: Optional log of what was captured and when, for transparency.

**User controls:**
- Toggle AI briefings on/off
- Choose what to capture (terminal, editor, notes—any combination)
- Set per-project capture sensitivity
- "Forget this task" button to delete all snapshots

### Local AI Approach

For the briefing generator:

**Model**: Llama 3 8B Instruct (or similar ~4GB model)  
**Runtime**: llama.cpp with Metal acceleration (macOS) or CUDA (Linux)  
**Latency target**: <3 seconds for typical terminal history (<50KB)  
**Fallback**: If model isn't loaded, show raw terminal tail + "Generate summary" button

**Prompt engineering considerations:**
- Keep context window small (terminal output is trimmed to last 50 commands)
- Few-shot examples for consistent formatting
- Explicit instruction to avoid hallucinating file contents not in output

---

## What Success Looks Like

### Quantitative Signals

| Metric | Target | Measurement |
|--------|--------|-------------|
| Resurrection Card engagement | >60% of shown cards result in "Resume" click | Analytics (local-only) |
| Time-to-resume | <60 seconds from app open to active work | Measure from card appearance to first terminal input |
| Snapshot capture success | >95% of task transitions have valid snapshots | Error tracking |
| Briefing quality rating | >4/5 average on "Was this helpful?" prompt | Occasional in-app survey |
| Daily active usage | Users see CR cards on >50% of return-to-work events | Usage patterns |

### Qualitative Signals

- Users report feeling "safe to step away" from complex tasks
- Reduction in "where was I?" moments (self-reported)
- Users actively add notes before leaving tasks (engagement with capture flow)
- Feature requests for more context types (git state, browser tabs, etc.)

### Anti-Metrics (Things to Avoid)

- Resurrection Cards feeling intrusive or blocking workflow
- AI summaries that hallucinate or confuse users
- Excessive storage usage (monitor snapshot growth)
- Slow app launch due to snapshot loading

---

## Open Questions

### Design Tensions

1. **Automatic vs. explicit capture**  
   How aggressive should implicit capture be? Too frequent = noise. Too infrequent = missed context. Current thinking: conservative implicit (10-min timeout) + generous explicit (every state transition).

2. **Briefing length**  
   How long should AI summaries be? Current target is 2-3 sentences. Some users might want more detail; others might want just "3 test failures."

3. **Editor integration depth**  
   How much editor state is enough? File positions are clear wins. What about unsaved changes? Git staging? Debug breakpoints? Where's the line?

4. **Multi-device scenarios**  
   Right Now is local-first, but what if someone uses two Macs? Snapshot sync is a future consideration but adds complexity and privacy concerns.

### Technical Unknowns

1. **Local LLM reliability**  
   Can we ship a model that works well across Mac hardware? M1 vs Intel? 8GB vs 16GB RAM? May need adaptive model selection.

2. **VS Code extension stability**  
   Extensions have lifecycle challenges (when does Right Now know VS Code closed?). Need robust heartbeat/reconnection logic.

3. **Terminal output semantics**  
   Parsing terminal output for "what commands ran" is imperfect (ANSI codes, interactive apps, etc.). How much can we reliably extract?

4. **Session storage limits**  
   With many projects and tasks, snapshot storage could grow. Need clear cleanup UX and maybe cloud backup (opt-in).

### Product Questions

1. **Onboarding**  
   How do users discover this feature? It requires returning after an absence. Maybe a walkthrough that simulates a "return"?

2. **Power user customization**  
   Should users be able to configure capture triggers? Or is simplicity better (it just works)?

3. **Mobile companion**  
   Would a read-only mobile view of your last context be valuable? "Check where you left off" from your phone?

---

## Inspiration & References

### Research

- **Gloria Mark, UC Irvine**: "The Cost of Interrupted Work" — foundational research on context switching cost. The 23-minute figure is from her 2008 study.
- **Mary Czerwinski, Microsoft Research**: Work on task-centric computing and activity-based workspaces.
- **Piotr Wozniak**: Spaced repetition research (not directly applicable, but the idea of externalizing memory is core).

### Prior Art

- **Time Machine (macOS)**: The mental model of "go back to a previous state" is intuitive. Context Resurrection is Time Machine for your working memory.
- **Tmux session persistence**: Developers who use tmux-resurrect already value this for terminals. CR extends the concept to the full context.
- **Notion's "Last edited by" breadcrumbs**: Shows that even small context cues ("you edited this 2 days ago") reduce cognitive load.
- **Warp's AI command search**: Demonstrates that AI can usefully summarize terminal activity. CR applies this to task-level narratives.
- **VS Code's workspace trust & session restore**: VS Code already saves window state. CR orchestrates this across apps.

### The Core Insight

> "Your mind is for having ideas, not holding them." — David Allen

Context Resurrection applies this wisdom to technical work: your mind is for *solving problems*, not *remembering where you were in the process*.

---

## What This Is NOT

Context Resurrection is a focused feature. To prevent scope creep—in readers' minds and in development—here's what we're explicitly *not* building:

**Not a full IDE.** Right Now doesn't edit code. It orchestrates your *return* to coding. The editor integration is lightweight (file positions, not syntax awareness). Your editor is your editor; Right Now just remembers where you were in it.

**Not a git client.** We don't track branches, diffs, or commits. Git already does that. If you want to see "what changed since Friday," use `git log`. Context Resurrection tells you "where you were *in the process* on Friday."

**Not a wiki or knowledge base.** Snapshots are ephemeral context, not permanent documentation. They're for *returning to work*, not *sharing work*. If you need to document how something works, write docs. If you need to remember where you were, use CR.

**Not session recording/playback.** We don't record your entire terminal session as a video or replayable script. We capture *enough* context to orient you—tail output, last commands, AI summary. Full terminal replay is out of scope.

**Not a backup system.** Snapshots are optimization, not disaster recovery. If you lose your code, restore from git. If you lose your *mental state*, restore from Context Resurrection.

---

## Appendix: Implementation Phases

### Phase 1: Foundation (4-6 weeks)
- [ ] Implement `ContextCapture` service with snapshot storage
- [ ] Add capture triggers to existing state transitions
- [ ] Basic Resurrection Card UI (no AI, no editor)
- [ ] Snapshot viewer in task details

### Phase 2: Terminal Intelligence (4-6 weeks)
- [ ] Integrate local LLM (llama.cpp binding in Rust)
- [ ] Implement briefing generation with prompt engineering
- [ ] Add "Generate Summary" button for existing sessions
- [ ] Briefing caching and regeneration

### Phase 3: Editor Integration (6-8 weeks)
- [ ] VS Code extension: state capture and restoration
- [ ] IPC protocol between Right Now and extension
- [ ] Fallback handling for unsupported editors
- [ ] Editor state in Resurrection Card

### Phase 4: Polish (4 weeks)
- [ ] Snapshot management UI (delete, pin, browse history)
- [ ] User notes integration (prompt before snapshot, view in card)
- [ ] Settings for capture sensitivity, retention, AI toggle
- [ ] Performance optimization (lazy loading, background capture)

---

## Appendix: Alternate Taglines

The main tagline is: **"Pick up exactly where you left off—even when you've forgotten where that was."**

Other candidates considered:

- "Your brain forgets. Right Now doesn't." — Punchy but maybe too negative.
- "Context Resurrection doesn't just save your work. It saves your *working memory*." — Good for body copy, too long for a tagline.
- "30 seconds to flow, not 30 minutes." — Specific but lacks poetry.
- "Time Machine for your working memory." — Apple IP concerns; too derivative.
- "The palest ink is better than the best memory." — Beautiful proverb, but not actionable.

---

*Last updated: February 2026*
*Author: Right Now Team*
