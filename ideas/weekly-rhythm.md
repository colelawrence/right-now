# Weekly Rhythm

**Ten minutes to know your week. One ritual to own the next.**

---

## The Problem

The weekly review is the most powerful habit you could have—and the one you're most likely to skip.

David Allen's GTD framework hinges on it. Cal Newport's time-blocking works only if you periodically zoom out. James Clear reminds us that you don't rise to the level of your goals—you fall to the level of your systems. The weekly review *is* the system check.

Yet Sunday evening finds most of us doing... nothing. Not because we don't care, but because:

1. **It feels like homework.** Open a dozen apps. Cross-reference calendars, task lists, notes. Reconstruct what you actually did from memory and scattered tools. By the time you've gathered the data, your energy is gone.

2. **There's no feedback loop.** You set priorities last week—but did they matter? Did you even work on them? Traditional task apps tell you what's done, not *how* you worked.

3. **Burnout creeps in silently.** You feel tired, but can't point to evidence. Was this week harder than usual? Were your breaks shorter? Were you context-switching more? Without data, your only signal is exhaustion—and by then it's too late.

4. **It requires discipline you've already spent.** After a week of decision-making, the last thing you want is another unbounded decision: "What should I reflect on?"

5. **The review itself has no ritual quality.** There's no warmth to it, no ceremony. Think about the rituals that stick: morning coffee, the evening walk, lighting a candle before you journal. They have sensory texture. They feel like *arriving somewhere*. The weekly review, by contrast, feels like homework you forgot to do. It should feel like making yourself a cup of tea and settling into your favorite chair. Instead it feels like opening a spreadsheet.

The gap isn't knowledge. Everyone knows weekly reviews matter. The gap is *friction*—between knowing you should reflect and actually doing it.

---

## The Vision

It's Sunday at 6 PM. A gentle notification arrives—not a ping, but a soft chime, almost like a meditation bell.

> *"Your week is ready for review. ~10 minutes."*

You open Right Now. The UI shifts. The usual task list fades; in its place, a warm amber glow. The screen feels quieter somehow—muted colors, generous whitespace, the visual equivalent of a deep breath. Instead of a blank page, you see *your week*, already mapped. The deep work you did on Tuesday afternoon. The Wednesday morning that was choppy with interruptions. The Thursday where you stayed in flow for three hours straight.

A quiet note appears:

> *"You worked 35% more than your 4-week average. Your breaks averaged 3 minutes instead of your usual 7."*

No judgment. Just a mirror.

You scroll through what got done—the satisfying list of completions. Then to what got stuck: the task that's been sitting for 11 days, the project that stalled when you hit a blocker.

You archive what no longer matters. You promote one "someday" idea to "this week."

Finally, you set three priorities. The app suggests Tuesday 2-5 PM for the deep work—that's when you've historically been most focused.

Ten minutes later, you close the app. You know where your week went. You know what's coming. The anxiety of "am I doing enough?" dissolves into clarity.

This is weekly rhythm.

---

## How It Works

Weekly Rhythm is a five-step guided wizard. Each step is timed, focused, and backed by real data. The entire flow targets 10 minutes.

### Step 1: Your Week at a Glance
**Duration: 2 minutes** | **Mode: Read-only insight**

This is the dashboard—your week visualized before you interpret it.

**What the UI shows:**
- **Energy heatmap**: A 7-day × 24-hour grid showing when you were in "working" state. Dense cells = deep work blocks. Scattered cells = fragmented time. Visually: deep amber cells for sustained focus, soft grey for idle time, and scattered dots of pale gold for fragmented work. Your Tuesday afternoon *glows*—that three-hour flow session shows up as an unbroken warm band. Wednesday morning is a scattering of small dots, like pebbles on sand—the meeting-fragmented hours laid bare. You see it instantly, before you read a single word.
- **Focus sessions**: Total Pomodoro sessions completed, with trend vs. prior week.
- **Break health**: Average break duration, break-to-work ratio, and whether breaks were actually taken or skipped.
- **Task completion velocity**: Tasks completed per day, with sparkline trend.
- **Context switches**: Number of times you switched between tasks within a work session (derived from timing data changes).

**What data feeds it:**
- `tenSecondIncrements` arrays from `ProjectStore.timing` (~L13–18 of `store.ts`) provide per-task, per-session granularity.
- `lastUpdateTime` timestamps enable reconstruction of when work happened.
- `StateChangeEvent` history (from `events.ts` L17–22) captures working/break/planning transitions.
- `TaskCompletedEvent` history (L28–32) provides completion timestamps.

**Energy Canary signals** (detailed in section below) appear here as gentle annotations:
- *"You had 40% more hours than your 4-week baseline."*
- *"Your Monday was fragmented—12 context switches in 3 hours."*

**Actions available:** None. This step is pure observation. Rushing to fix things defeats the purpose.

---

### Step 2: Celebrate & Clear
**Duration: 2 minutes** | **Mode: Acknowledge + inbox zero**

**What the UI shows:**
- **Completed tasks** from the week, grouped by day.
- For each task: time invested (from `tenSecondIncrements` sum), whether it was a single session or distributed.
- **Inbox items**: Any tasks without a section heading, or marked with a specific tag (e.g., `#inbox`).

**Why celebration matters:**
Productivity culture emphasizes the gap—what's left undone. But behavioral psychology shows that acknowledging progress sustains motivation. This step forces a pause before the harder work of grooming.

**Actions available:**
- Archive completed tasks (move to a `## Done` or `## Archive` section).
- Quick-add tasks from inbox to a project section.
- Star a task as "highlight of the week" (emoji badge in Markdown).

---

### Step 3: What Got Stuck?
**Duration: 2 minutes** | **Mode: Surface blockers**

**What the UI shows:**
- **Stale tasks**: Items that have been in the file for > 7 days with no time logged.
- **Partially worked tasks**: Started but not completed, with time invested.
- **Tasks with no recorded work**: Listed but never touched.

**Why this matters:**
Stuck tasks are invisible debt. They clutter your list and your mind. Surfacing them explicitly converts vague anxiety into actionable decisions.

**Actions available:**
- **Defer**: Move to a "## Someday" section with optional future date.
- **Delegate**: Mark with `@name` and move to waiting section.
- **Delete**: Archive or remove outright.
- **Decompose**: Split into smaller tasks (opens a quick-add modal pre-filled with the original task name).

---

### Step 4: Groom the Backlog
**Duration: 2 minutes** | **Mode: Curate**

**What the UI shows:**
- **Someday/Maybe section**: Items you've deferred in past reviews.
- **Recently added tasks** not yet worked on.
- **Suggested promotions**: Tasks that have been in Someday for 3+ weeks (either promote or archive—limbo is poison).

**Actions available:**
- **Promote to Active**: Move to main task list.
- **Archive**: Remove from active view (move to `## Archive`).
- **Edit**: Quick-edit task name or add details.
- **Reorder**: Drag to prioritize within section.

**Why structured grooming matters:**
Backlogs grow unconsciously. Without periodic pruning, they become a graveyard that makes every glance at your task list subtly demoralizing. This step keeps the list alive.

---

### Step 5: Set Your Three
**Duration: 2 minutes** | **Mode: Commit**

**What the UI shows:**
- A prompt: *"What are your three priorities for the week?"*
- Suggested tasks from your active list (most recently worked, highest estimated time).
- **Schedule recommendations**: Based on your historical energy patterns, when to block time for each priority.

**How scheduling suggestions work:**
The app analyzes your `tenSecondIncrements` timestamps across the past 4 weeks. It identifies:
- **Peak focus windows**: Times of day when you sustain long work sessions.
- **Protected days**: Days that historically had few interruptions.
- **Danger zones**: Times when your sessions are short and fragmented.

A suggestion might read:
> *"Your deepest work happens Tuesday 2–5 PM. Consider protecting that slot for Priority #1."*

**Actions available:**
- **Set priority order**: Drag to rank 1, 2, 3.
- **Add time estimate** (optional): Links to Pomodoro planning.
- **Accept schedule suggestion**: Creates a `#focus-block` annotation for the task.
- **Dismiss**: If you prefer to allocate time intuitively.

---

### The Closing Moment

After completing all five steps, a summary appears:

> **Your week in review:**
> - 23 tasks completed
> - 14 hours of focused work
> - 3 priorities set for next week
>
> *You're 12 weeks into your streak. Nice.*

The "streak" is simply the count of consecutive weeks with a completed review. This is the **Seinfeld Strategy** in action: Jerry Seinfeld wrote jokes every day and marked each day with a red X on a wall calendar. His only rule: "Don't break the chain." The chain itself becomes the motivation.

But we're not cruel about it. If you miss a week—vacation, illness, life—your streak doesn't reset to zero. Instead, you see a visual gap: *"Weeks 1-8 • [gap] • Week 10."* The chain shows the gap honestly, but invites you back. One missed week doesn't erase twelve good ones. The message shifts: *"You missed last week. Welcome back. Let's pick up where you left off."*

Gentle gamification—enough to notice, not enough to become anxiety.

---

### What Gets Saved

The priorities you set aren't ephemeral. They persist and reappear:

- **In the main task view**: Your three priorities show with a subtle badge—a small `①②③` indicator that keeps them visible without shouting.
- **In next week's review**: Step 1 opens with "Last week, you set these priorities:" followed by a completion check. Did you work on them? Finish them? Abandon them? This closes the loop—intentions meet reality.
- **In your history**: Each weekly snapshot stores what you committed to. Over time, you can see: do your priorities match your actual work? Are you good at predicting what matters? This is meta-awareness—knowing not just what you do, but how well you know yourself.

---

## The Energy Canary

A canary in a coal mine doesn't tell miners what to do. It simply stops singing.

The Energy Canary watches for burnout signals and reflects them back—gently, descriptively, never prescriptively. You decide what to do with the information.

### How It Works

**Rolling baseline calculation:**
For each metric, the app maintains a 4-week rolling average. This becomes "normal" for *you*, not some abstract standard.

**Signals it watches:**

| Signal | Data Source | Warning Threshold |
|--------|-------------|-------------------|
| Total work hours | Sum of `tenSecondIncrements` per week | >25% above baseline |
| Break duration | Time between `working → break` and `break → working` events | <50% of baseline average |
| Break skipping | Ratio of work sessions without subsequent break | >30% of sessions |
| Context switches | Distinct task IDs with overlapping timestamps | >40% above baseline |
| Late-night work | Sessions starting after 9 PM local time | >3 per week when baseline is 0 |
| Weekend encroachment | Any work sessions on Saturday/Sunday | Present when baseline is 0 |
| Completion rate drop | Tasks completed / tasks attempted | <70% of baseline |

**What it never does:**
- Prescribe action ("You should take a break")
- Moralize ("You're working too hard")
- Alarm or interrupt during work

**What it does:**
- Provide factual comparison ("Your hours were 35% above your 4-week average")
- Surface patterns you might not notice ("This is your third week with shortened breaks")
- Celebrate recovery ("Your break patterns returned to baseline—nice reset")

### Example Messages

**Concerning week:**
> *"You logged 42 hours this week. Your 4-week average is 31. Breaks averaged 3.2 minutes (your usual: 6.8)."*

**Recovery week:**
> *"Your hours dropped back to 29. Break duration is up to 7 minutes. Whatever you're doing differently—it's working."*

**Pattern detection:**
> *"Wednesday afternoons have been fragmented for 3 weeks running. Might be worth investigating—or protecting that time differently."*

**Celebration (positive trend):**
> *"You had three days this week with 90+ minute uninterrupted focus blocks. That's up from one last month. Your deep work capacity is growing."*
>
> *"Zero weekend work for the fourth week running. Your boundaries are holding."*

Positive reinforcement matters as much as warning signals. The Canary isn't just a smoke detector—it's also the friend who notices when you're doing well.

**Neutral observation:**
> *"Pretty typical week. Hours and breaks aligned with your averages."*

### Privacy and Data Handling

All Energy Canary data stays local. The `ProjectStore` already persists timing data in `ProjectStore.json` (per the Tauri store plugin). Weekly snapshots are stored in the same local-first model—no server, no sync, no upload.

Users can:
- Clear historical data at any time.
- Disable the Energy Canary entirely.
- View the raw data (transparency builds trust).

---

## User Scenarios

### Scenario 1: A Sustainable Week

**The user:** Maya, a freelance developer using Right Now to manage client projects.

**What happened:**
Maya logged 28 hours of focused work across 5 projects. Her breaks averaged 6 minutes. She completed 15 tasks and had only 2 stuck items—both waiting on client feedback.

**The review experience:**

Step 1 shows a healthy heatmap—dense blocks on Tuesday and Thursday, lighter days Monday and Friday. The Energy Canary is quiet: *"Pretty typical week. Hours and breaks aligned with your averages."*

Step 2 lets Maya acknowledge her completed work. She stars the "Launch client dashboard" task as her highlight.

Step 3 surfaces the 2 waiting-on-client tasks. She moves them to a "## Waiting" section with `@client-name` annotations.

Step 4 is quick—backlog is already clean from last week's grooming.

Step 5, she sets three priorities. The app suggests her Thursday morning slot for the deepest work—that's when her historical focus is strongest.

**Total time:** 8 minutes. **Feeling:** Clear and energized for the week.

---

### Scenario 2: A Warning Week

**The user:** Jordan, a startup founder juggling product, fundraising, and ops.

**What happened:**
Jordan pushed hard to hit a demo deadline. They logged 51 hours—vs. a 34-hour baseline. Breaks were skipped or shortened. Context switches spiked on Wednesday (investor call prep overlapping with product work).

**The review experience:**

Step 1 hits differently. The heatmap is dense—almost solid blocks from 8 AM to 10 PM on Wednesday and Thursday. The Energy Canary speaks up:

> *"You logged 51 hours this week. Your 4-week average is 34. That's 50% above baseline."*
>
> *"Break duration dropped to 2.1 minutes (your usual: 5.4). You skipped breaks entirely in 8 of 14 work sessions."*
>
> *"Wednesday had 23 context switches—your highest single day."*

No alarm. Just facts. Jordan pauses, recognizing the pattern.

Step 3 shows several stuck tasks—things that got pushed for the deadline. Jordan defers two, deletes one that no longer matters.

Step 5, Jordan intentionally sets only two priorities instead of three—leaving margin.

**Total time:** 11 minutes. **Feeling:** Honest reckoning. A conscious choice to recover.

---

### Scenario 3: A Recovery Week

**The user:** Jordan, one week later.

**What happened:**
After the warning, Jordan intentionally slowed down. Logged 30 hours. Took real breaks. Protected Thursday completely.

**The review experience:**

Step 1 shows a calmer heatmap. Gaps where rest happened. The amber blocks are shorter, with breathing room between them. The Energy Canary:

> *"Your hours dropped to 30—back in your baseline range. Break duration averaged 6.2 minutes."*
>
> *"You had zero context switches on Thursday. First single-focus day in 6 weeks."*
>
> *"Whatever you're doing differently—it's working."*

Jordan pauses at that last line. It's not much—seven words—but something loosens in their chest. *It's working.* Not "you should keep doing this" or "good job protecting your time." Just an observation, but one that lands like validation. They'd been half-worried the slower week was a cop-out, that they were falling behind. The data says otherwise. The data says: you chose sustainability and it was the right call.

Step 2 shows 8 tasks completed—fewer than the crunch week, but sustainable. Jordan notices they don't feel guilty about the smaller number. Eight good things, finished with focus.

Step 5, Jordan feels comfortable setting three priorities again. The anxiety from last week has dissolved into something quieter: confidence.

**Total time:** 9 minutes. **Feeling:** Recovery validated. Momentum without burnout. Trust in oneself—restored.

---

## Why Right Now Is Uniquely Positioned

Most task apps could theoretically add a weekly review. But Right Now has something they don't: **real timing data at the task level**.

**What exists today:**
- `tenSecondIncrements: number[]` (store.ts, L5–9): Per-task timing with 10-second granularity.
- `lastUpdateTime: number`: Enables timestamp reconstruction.
- `workState` and `stateTransitions`: Pomodoro state machine with start/end times (project.ts, L8–13).
- `EventBus` with `StateChangeEvent`, `TaskCompletedEvent`: Observable history of work patterns (events.ts, L17–32).

**What this enables that competitors can't do:**
- **When** you worked, not just **what** you worked on.
- **How long** each task actually took, vs. estimates.
- **Break patterns** as first-class data, not inferred from absence.
- **Context switches** detectable from overlapping task timing.

Todoist knows what you checked off. Notion knows what pages you edited. Neither knows you did your best work on Tuesday afternoon, or that your Wednesdays are fragmented.

The Pomodoro + sessions + task state combination creates a *reflection surface* that doesn't exist elsewhere.

---

## Technical Architecture

### Data Aggregation Layer

A new `WeeklyAggregator` service collects timing data into weekly buckets:

```typescript
interface WeeklySnapshot {
  weekStart: number;               // Monday 00:00:00 timestamp
  weekEnd: number;                 // Sunday 23:59:59 timestamp
  
  // From tenSecondIncrements aggregation
  totalWorkSeconds: number;
  workSecondsByDay: number[];      // [Mon, Tue, Wed, Thu, Fri, Sat, Sun]
  workSecondsByHour: number[];     // [0..23] totaled across week
  
  // From state transitions
  pomodoroSessions: number;
  breaksTaken: number;
  breaksSkipped: number;
  avgBreakDurationSeconds: number;
  
  // From task timing analysis
  tasksCompleted: string[];        // Task names
  tasksWorkedOn: string[];         // Including incomplete
  contextSwitches: number;         // Derived from overlapping task times
  
  // Energy Canary data
  baselineComparison: {
    hoursVsBaseline: number;       // Percentage difference
    breakDurationVsBaseline: number;
    switchesVsBaseline: number;
  };
  
  // Review outcomes (filled after wizard completion)
  reviewCompletedAt?: number;
  prioritiesSet: string[];
  staleDismissed: number;
  someedayPromoted: number;
}
```

### Storage Strategy

Weekly snapshots are stored in `ProjectStore.json` under a new `weeklySnapshots` key:

```typescript
interface StoreSchema {
  // ... existing fields
  weeklySnapshots: WeeklySnapshot[];
  weeklyReviewStreak: number;
  lastReviewTimestamp?: number;
}
```

The 4-week rolling baseline is computed on-demand from the most recent 4 entries—no separate storage needed.

### New React Components

| Component | Purpose |
|-----------|---------|
| `WeeklyRhythmWizard` | Shell component managing 5-step flow |
| `WeekAtAGlance` | Heatmap + Energy Canary display |
| `CelebrateAndClear` | Completed task list + archive actions |
| `StuckSurfacer` | Stale/blocked task identification |
| `BacklogGroomer` | Someday section + promote/archive |
| `SetYourThree` | Priority picker + schedule suggestions |
| `EnergyCanary` | Shared component for rendering canary messages |

### Event Extensions

The `events.ts` event bus is extended with review-specific events:

```typescript
interface WeeklyReviewStartedEvent extends BaseEvent {
  type: "weekly_review_started";
  weekStart: number;
}

interface WeeklyReviewCompletedEvent extends BaseEvent {
  type: "weekly_review_completed";
  weekStart: number;
  durationSeconds: number;
  prioritiesSet: string[];
}
```

### Reminder System

A new `ReminderService` handles weekly review prompts:
- Configurable day/time (default: Sunday 6 PM).
- Uses Tauri's notification API.
- Respects system Do Not Disturb.
- Never interrupts active Pomodoro sessions.

---

## What Success Looks Like

**Adoption:**
- 60%+ of active users complete at least one weekly review within first 30 days.
- 40%+ maintain a 4-week streak.

**Behavior change:**
- Users who complete reviews show more stable work hours over time (less variance week-to-week).
- Break-skip rates decrease after the first Energy Canary warning.

**Qualitative:**
- Users describe feeling "less anxious about productivity."
- The review becomes a ritual—something looked forward to, not dreaded.

**Engagement:**
- Average review completion time stays under 12 minutes.
- Wizard step abandonment rate stays under 20%.

---

## Open Questions

### Timing of reminder
Sunday 6 PM is one choice, but:
- Some users prefer Sunday morning (fresh mind, plan the week ahead).
- Some prefer Friday afternoon (close out the work week).
- Some prefer Monday morning (set the week's tone).

**Proposed solution:** Configurable, with smart defaults based on when the user historically does their first Pomodoro of the week.

### What if users skip weeks?
The 4-week rolling baseline assumes continuity. If a user misses two weeks:
- Option A: Baseline freezes (may feel dated).
- Option B: Baseline recalculates from available data (may under-represent).
- Option C: The canary notes the gap explicitly: *"No data for weeks 2–3. Baseline reflects weeks 1 and 4."*

**Proposed solution:** Option C—transparency over hidden logic.

### Privacy of burnout data
Some users may share their screens or Right Now window. Sensitive health-adjacent data (burnout signals) should be opt-in for display.

**Proposed solution:** Energy Canary is collapsed by default in the review. User must expand to see. Export/share excludes this section.

### Integration with calendar blocking
Schedule suggestions are most useful if they can actually create calendar blocks.

**Proposed solution:** V1 provides copy-friendly format ("Tuesday 2–5 PM: Deep work on Priority #1"). V2 explores CalDAV integration or clipboard-to-calendar workflows.

### What about team/shared projects?
Weekly Rhythm is currently individual. Shared team TODOs raise questions about aggregated patterns vs. individual.

**Proposed solution:** Defer to future. V1 is personal. Team features are a different product surface.

---

## Inspiration

### GTD Weekly Review
Allen's original 5-step review: get clear, get current, get creative. Weekly Rhythm adapts this for the digital-native, data-rich era.

### Exist.io
Auto-correlates life data (sleep, exercise, mood) into patterns. Weekly Rhythm brings this sensibility to work patterns specifically.

### Apple Health Rings
Simple, gamified, daily visual. The streak counter and energy heatmap borrow this "glanceable progress" language.

### Spotify Wrapped
Annual reflection done right: personal, surprising, shareable. Weekly Rhythm is "Wrapped for your work week"—smaller scale, higher frequency.

### RescueTime / Timing.app
Automatic time tracking that surfaces patterns. But neither ties to task completion—Weekly Rhythm's unique angle.

### Cal Newport's Shutdown Ritual
The idea of a formal end-of-work ceremony. Weekly Rhythm extends this to a weekly scope.

---

## Closing Thought

Productivity is not about doing more. It's about knowing what you're doing—and whether that's what you intended.

Most tools help you *do*. Weekly Rhythm helps you *see*.

But seeing is only half the gift. The other half is *kindness*. Kindness toward the week you actually lived, not the one you imagined. Kindness toward the breaks you took and the ones you skipped. Kindness toward the gap in your streak—because you came back.

Ten minutes a week. A mirror for your work. A rhythm you can trust.

*You showed up this week. Let's see what you made of it.*
