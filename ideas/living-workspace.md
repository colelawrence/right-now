# Living Workspace

**Your workspace breathes with you.**

---

## The Problem

Productivity software is hostile to the human nervous system.

We spend eight, ten, twelve hours a day staring at the same cold interface. The same sterile whites. The same aggressive notification red. The same silence broken only by the sounds of our own typing. Hour after hour, the pixels don't change. The emotional temperature never shifts. The app makes no acknowledgment that time is passing, that you are a living being with circadian rhythms and attention cycles, that outside your window the sun has crossed the sky and evening is falling.

This is not neutral. This is actively harmful.

Research on environmental psychology is unambiguous: our surroundings shape our cognition. Natural light improves mood and focus. Color temperature affects alertness and relaxation. Ambient sound reduces stress and enhances creative thinking. The gentle presence of nature—even simulated—restores depleted attention.

Yet the tools we use to manage our most demanding cognitive work are designed as if we were machines interfacing with machines. Flat. Timeless. Dead.

The productivity app has become a sensory deprivation chamber.

Consider the cumulative weight: a task manager that looks exactly the same at 7am fresh start as it does at 9pm exhausted finish. A timer that ticks down in the same mechanical font whether you're in flow state or fighting burnout. An interface that offers no ceremony when you complete something difficult, no acknowledgment of the small victories that sustain motivation through long projects.

You've been staring at the same blue-white Notion sidebar for six hours and didn't even notice the sun set. You completed eleven tasks and your brain registered none of them—just checkboxes flipping from empty to filled, the same microscopic animation each time, meaningless. Your eyes hurt and you can't tell if you've been productive or just busy. You feel *used* but not *accomplished*.

This is what happens when we optimize for information density and forget that the person receiving that information has a body, emotions, and a relationship with time.

---

## The Vision

Imagine a different kind of day.

You open Right Now at 6:47am. The interface greets you in soft peach tones, the color of first light. The ambient hum of morning—distant birds, the suggestion of dew—plays so quietly you're not sure you're hearing it at all. You feel the gentle invitation to begin.

You pick your first task and start working. The colors slowly warm as the morning progresses, matching the quality of light outside your window without you ever noticing the change. A quiet generative soundscape breathes underneath your focus—something like rain on glass, but softer, warmer, more like the *texture* of rain than its sound. Through good headphones it feels almost tactile: droplets landing on your peripheral attention like fingertips on fabric, neither demanding nor absent, just *there* in the way a warm room is there.

You complete your first task. A bell sounds—not the harsh ping of a notification, but something like a temple bell struck in cool morning air, the tone bright and slightly metallic, a color you might call "sunlit brass." The sound doesn't so much play as *bloom*, opening outward into a resonance that fades before you realize you were listening. A gentle ripple animation flows from the checkbox—warm peach diffusing to nothing. Your phone gives a subtle haptic pulse, the feeling of something clicking into place. The moment is marked. The accomplishment is *felt*.

The morning passes. You finish three more tasks. Each completion has its own quality—the bells are similar but not identical, evolving through the day like a melody you're composing one note at a time. This is the secret rhythm of Living Workspace: your day has a musical arc. The first completion bell at 7am is bright and clear—an invitation. The third is richer, acknowledging momentum. By the seventh, there's depth there, harmonic complexity that wasn't present in the first. At day's end, if you listen back to your completions played in sequence, you hear a composition you made without trying. Five notes. Eight notes. Twelve notes. A song that sounds like *your* kind of productive, no one else's.

You're not consciously tracking this, but somewhere in your body you know: you're building something.

Your break timer approaches. The color temperature shifts imperceptibly warmer. The soundscape's pace gently decelerates. Before the timer ends, you feel your body beginning to relax. The transition isn't a jarring alarm—it's a natural tide going out.

During break, the colors shift toward calm blues. The ambient audio becomes more spacious, more restorative. You look away from the screen and when you return, the interface still feels like rest.

The afternoon arrives and the palette brightens—crisp, clear, energized. You're in your productive peak and the environment supports it. You complete task after task. The bells accumulate into a rhythm, a record of your progress you can feel even when you're not looking.

As evening approaches, amber creeps into the interface. The blue light that would interfere with your circadian rhythm is being gently filtered away. The soundscape has shifted to something warmer—crackling fire, perhaps, or gentle synth pads that rise and fall like breathing, their timbre like dark honey or polished wood. You complete your final task of the day. The completion bell has a particular resonance at this hour—lower, richer, the tone of burnished bronze rather than morning's bright brass. It lingers longer, too, as if the day itself is exhaling. A day's-end quality. A period, not a comma.

You close the app. The last thing you see is a gentle visualization of your day's rhythm—the completed tasks distributed across the hours, a heartbeat of productivity you created. Not gamified points or achievement badges. Just the truth of what you did, presented beautifully.

Tomorrow morning, when you open Right Now into the soft peach light, you'll start again. And it will feel like coming home.

---

## How It Works

Living Workspace comprises three interwoven subsystems that, together, create the sensation of an environment that is alive and responsive to you.

### Daylight Canvas

The app's colors breathe with the time of day, creating an interface that feels attuned to natural light even when you're working in artificial environments.

**Solar position calculation:**

The system computes the sun's position based on the user's local time and, optionally, their geographic coordinates for accuracy. We track five key moments:
- **Dawn** (civil twilight begin → sunrise): Soft peach and gold tones, low contrast, inviting
- **Morning** (sunrise → solar noon - 2hr): Warming palette, increasing brightness and clarity  
- **Midday** (solar noon ± 2hr): Crisp, clear, maximum contrast and alertness
- **Afternoon** (solar noon + 2hr → sunset): Gradual amber shift, sustained energy
- **Evening/Night** (sunset → civil twilight end → night): Deep warm tones transitioning to calm dark mode

For simplicity in MVP, we use time-based heuristics (6am dawn, 12pm midday, 6pm dusk) that can later be enhanced with actual solar calculations via `suncalc` or similar libraries.

**Color palette interpolation:**

Rather than abrupt theme switches, we use continuous interpolation between palettes. The transition happens over 30-60 minute windows, slow enough to be imperceptible but felt. A user checking at 3pm and again at 5pm will notice the interface looks different but won't have seen it change.

The palette system defines:
- Background gradients (current: `from-white to-gray-50`, `from-amber-50 to-amber-100`)
- Text colors and contrast ratios
- Accent colors for interactive elements
- Border and shadow tones

**CSS custom properties approach:**

The poetry becomes engineering here, but the goal remains the same: imperceptible change, always felt. Integration with the existing Tailwind setup via CSS custom properties updated on a timer:

```css
:root {
  --canvas-bg-from: theme('colors.slate.50');
  --canvas-bg-to: theme('colors.slate.100');
  --canvas-text: theme('colors.slate.900');
  --canvas-accent: theme('colors.blue.600');
  --canvas-transition: 2000ms;
}

/* Updates every ~5 minutes, interpolating between states */
```

A `DaylightCanvas` service runs alongside the existing `Clock` (`src/lib/clock.ts`), emitting palette updates that propagate through React context. Components opt into the living palette via utility classes or direct CSS variable references.

### Resonance Moments

Task completion transforms from a checkbox click into a small ceremony—a moment of acknowledgment that rewards the nervous system and builds a sense of accumulated progress.

**Completion detection:**

The existing task completion flow (`src/App.tsx:104-118`) already emits a `task_completed` event through the EventBus:

```typescript
// From src/lib/timer-logic.ts:145-155
export function computeTaskCompletedEvents(taskName: string, now: number): AppEvent[] {
  return [
    { type: "task_completed", timestamp: now, taskName },
    { type: "sound", timestamp: now, sound: SoundEventName.TodoComplete, reason: `Task completed: ${taskName}` },
  ];
}
```

We extend this to include visual and haptic feedback:

```typescript
export interface TaskCompletedEvent extends BaseEvent {
  type: "task_completed";
  taskName: string;
  completionIndex: number;  // nth completion today
  timeOfDay: 'morning' | 'afternoon' | 'evening';  // affects bell pitch
}
```

**Bell synthesis via FM synthesis:**

The current sound system (`src/lib/sounds.ts`) plays pre-recorded samples via the Tauri backend. For Resonance Moments, we introduce Web Audio API synthesis for completion bells, allowing dynamic variation:

```typescript
interface BellParameters {
  fundamentalFreq: number;    // Base pitch, varies with time of day
  modulationIndex: number;    // Complexity, increases with completion count
  attackTime: number;         // Faster in morning, slower in evening
  decayTime: number;          // Longer decay for "milestone" completions
  harmonicRatios: number[];   // Spectral character
}
```

The bell evolves through the day:
- **Morning bells**: Higher pitched, bright, energizing (fundamental ~800-1000 Hz)
- **Midday bells**: Clear, centered (fundamental ~600-700 Hz)
- **Evening bells**: Lower, more resonant, conclusive (fundamental ~400-500 Hz)

Each subsequent completion in a day subtly shifts the parameters—a fifth completion sounds richer than a first, acknowledging accumulation. But the changes are small enough to avoid gamification; this is texture, not scoring.

**Ripple animation:**

When a task completes, a radial ripple animation emanates from the checkbox. This is achieved via CSS animation triggered by completion state:

```css
@keyframes completion-ripple {
  0% {
    transform: scale(0.8);
    opacity: 0.5;
  }
  100% {
    transform: scale(2.5);
    opacity: 0;
  }
}

.task-checkbox[data-completing="true"]::after {
  animation: completion-ripple 600ms ease-out;
}
```

The ripple color follows the Daylight Canvas palette—peach in morning, crisp blue at midday, amber in evening.

**Haptic feedback:**

For users on devices with haptic capability (primarily mobile/tablet, but increasingly laptops), completion triggers a subtle haptic pattern via the Vibration API:

```typescript
if ('vibrate' in navigator) {
  navigator.vibrate([10, 30, 10]); // Short, pause, short—a "click into place" feeling
}
```

**Daily rhythm visualization:**

A subtle, optional visualization shows the day's completions distributed across time—a horizontal timeline with marks at each completion, resembling a heartbeat or rhythm strip. This appears on hover in the footer or as an end-of-day summary. It's not a chart to optimize; it's a poem about how you spent your attention.

### Stillwater Soundscapes

Ambient generative audio that breathes with your work state—present enough to support focus, subtle enough to never intrude.

**State-aware generative audio:**

The existing work states (`planning`, `working`, `break` per `src/lib/project.ts`) drive soundscape selection:

- **Planning state**: Silence or optional ambient (user preference)
- **Working state**: Focus soundscape—rain, white noise, lo-fi generative tones
- **Break state**: Restorative soundscape—more spacious, nature-forward

The soundscape doesn't just switch; it crossfades over 30-60 seconds, matching the natural transition of attention states.

**Web Audio API architecture:**

Soundscapes are generated in real-time via layered oscillators, noise generators, and convolution reverbs:

```typescript
class StillwaterEngine {
  private audioContext: AudioContext;
  private masterGain: GainNode;
  private layers: SoundLayer[];
  
  // Primary generators
  private rainGenerator: RainSynthesizer;
  private toneGenerator: AmbientToneGenerator;
  private fireGenerator: FireCrackleGenerator;
  
  // State management
  private currentState: WorkState;
  private transitionProgress: number;
}
```

Key generators:
- **Rain**: Filtered noise with randomized droplet events, gentle LFO on filter cutoff
- **Ambient tones**: Very slow FM synthesis with randomized frequency ratios, creating gentle harmonic drift
- **Fire**: Layered crackle events with warm filtering, convolution reverb for "room" feel
- **Nature**: Bird calls (sampled), wind (filtered noise), water (layered noise with resonant filters)

**Breath-like modulation:**

All soundscapes include slow amplitude modulation (0.05-0.1 Hz) that creates a subtle "breathing" quality. The modulation depth increases during breaks (more obvious relaxation cue) and decreases during focus (more constant support).

**Crossfading and transitions:**

State changes trigger 30-second crossfades:

```typescript
async transitionToState(newState: WorkState) {
  const oldScape = this.activeScape;
  const newScape = this.getScapeForState(newState);
  
  // Linear crossfade over 30 seconds
  await Promise.all([
    this.fadeOut(oldScape, 30000),
    this.fadeIn(newScape, 30000)
  ]);
}
```

**Sound design philosophy:**

The sounds must be almost-but-not-quite-nameable—familiar enough to be comforting, abstract enough to avoid cognitive capture. When someone says "is that rain?" the correct answer is "sort of."

1. **Never identifiable**: The sounds should be abstract enough that you can't name them easily. "Rain" is an approximation; the actual sound is rain-like but more abstract. If you can picture it, it's too literal.
2. **Never repetitive**: Generative parameters ensure no two minutes are identical. The ear detects loops within seconds; boredom follows within minutes.
3. **Never demanding**: Maximum volume is well below speaking level. The sounds are peripheral. If you can't hold a conversation over them, they're too loud.
4. **Always optional**: A clear, accessible toggle. Some users need silence; that's respected. Ambient sound is an offering, not an imposition.

---

## User Scenarios

### A Full Day Arc

**Maya, 6:15 AM**

Maya opens Right Now before the house wakes up. The interface glows in soft peach and rose gold—dawn colors. The ambient sound is off (her preference), but the visual warmth feels welcoming. She reviews her tasks, noting that the colors feel different from last night's session. Less intense. More inviting.

She starts her first deep work block. The timer begins and she enables Stillwater—a gentle, abstract rain sound fills her headphones, so quiet she sometimes forgets it's there. Two hours pass. She completes three difficult tasks. Each completion brings a clear, bright bell and a subtle ripple animation. She notices she's smiling after the third one.

**Maya, 10:30 AM**

The interface has shifted without her noticing—crisp whites and light blues now, energetic and clear. She's in her most productive hours and the environment matches. The ambient sound has a slightly different character now, though she couldn't say how. She completes two more tasks. The bells sound different too—maybe slightly lower? She doesn't analyze it; she just keeps working.

**Maya, 2:15 PM**

Post-lunch, she notices the first hints of amber creeping into the interface. It's subtle—the accent colors have warmed slightly, the shadows are softer. Her break timer is approaching; she notices her body already preparing to step away. When the break begins, the ambient sound opens up, becomes more spacious. She looks out the window and back at her screen; somehow, they match.

**Maya, 6:45 PM**

The interface is fully amber now, deep and warm. She's completed eight tasks today—a personal best. The final completion bell is lower, more resonant than the morning ones. It feels conclusive. She hovers over the footer and sees a subtle visualization of her day—eight marks distributed across the timeline, her productivity made visible as rhythm rather than score.

She closes the app. Tomorrow morning, she'll return to the peach glow of dawn and begin again.

### The Flow of Five Completions

**Marcus, mid-morning**

Marcus is in flow state. He's been working for ninety minutes and has knocked out four tasks in rapid succession. Each completion brought its bell, its ripple, its tiny moment of ceremony. He wasn't consciously counting, but something in him feels the accumulation—a momentum building.

He finishes the fifth task. The bell sounds slightly different—not dramatically, not gamified, but there's a richness to it. Maybe it's that fifth-note feeling, like a resolved chord. The ripple animation seems to linger a moment longer, though perhaps he's imagining it.

He glances at the rhythm visualization out of curiosity. Five marks clustered in the last hour and a half. He feels something he rarely feels with productivity tools: satisfaction that isn't guilt. He hasn't been grinding. He's been flowing.

He stands up, makes coffee, and returns to find the interface subtly warmer than when he left. Midday approaching. He's ready for the next batch.

### A Break That Actually Feels Restorative

**Aisha, 3:47 PM**

The afternoon slump hit hard. Aisha has been staring at the same complex task for twenty minutes, making little progress. Her timer shows 13 minutes left in this work block, but she can feel her attention fragmenting.

She manually triggers a break early. The sound of the break-start chime is gentle, not jarring—permission granted. The interface's color temperature shifts: the bright whites of her work session soften into calm blues and grays. The Stillwater soundscape opens up, the tight focus-sounds spreading into something more spacious. Ambient nature sounds, impossibly soft.

She stands, stretches, looks out the window. When she glances back at her screen, the interface still feels like rest. Not demanding her return. Not counting down aggressively. Just holding space.

She makes tea. She sits without working. The soundscape breathes. The interface waits, blue and patient.

Ten minutes later, she feels it: a subtle shift in her attention. Not a notification demanding return. Not a timer guilt-tripping her into productivity. Something else—a gradual coalescence, the way your eyes naturally refocus after staring at clouds. Her mind starts reaching toward the complex task on its own, the way a plant turns toward light. *This* is the magic moment: the environment did the work that willpower usually has to do. It created conditions for focus to return naturally, rather than forcing a context switch through alarm and obligation.

She ends the break. The interface warms back toward productivity—but gradually, gently, matching the pace of her returning attention. The soundscape tightens into its focus character. She returns to the complex task.

It's easier now. Not because she rested, though she did. Because she was *restored*—the break wasn't an interruption of work, it was part of the work's rhythm.

### The Absence Effect

There's a test for whether an environmental feature has truly integrated into someone's experience: what happens when it's gone?

Users who disable Living Workspace for a day—maybe to test battery impact, maybe out of curiosity—often re-enable it within hours. When asked why, they struggle to articulate it. "The app felt cold." "It was too quiet." "I don't know, I just missed *something*."

This is the absence effect: the strongest evidence of emotional integration. The features have dropped below conscious awareness into the peripheral layer where *ambiance* lives. You don't notice good air conditioning until it breaks. You don't notice natural light until you work a day in a windowless room. You don't notice Living Workspace until it's gone and the app feels like a spreadsheet again.

The absence effect is what separates genuine environmental design from feature theater. If users notice Living Workspace while it's on, we've failed. If they notice when it's *off*, we've succeeded.

---

## Design Philosophy

### Subtlety Over Spectacle

Every element of Living Workspace operates at the edge of perception. Color shifts happen over thirty minutes, not three seconds. Bells are rich but quiet. Soundscapes breathe rather than pulse. The goal is always influence, never intrusion.

If a user consciously notices a transition, we've been too aggressive. The changes should be felt before they're seen—a general sense that the environment is supporting you, without being able to point to how.

**Rule of thumb:** If a user notices a transition happening in real-time, it's too fast. If they can name the exact moment a color changed, we've failed. Aim for changes detectable only by comparison—screenshot now vs. screenshot thirty minutes ago.

### Peripheral Over Demanding

Traditional productivity software demands focal attention: notifications that interrupt, dashboards that require analysis, metrics that invite comparison. Living Workspace works in the peripheral channel—the same channel through which you register ambient light, room temperature, the quality of the air.

Peripheral awareness doesn't compete with focus; it supports it. A brain already processing complex tasks doesn't need more demands on its central attention. It needs an environment that its peripheral systems can relax into.

**Rule of thumb:** If you can focus on the feature, it shouldn't exist. Living Workspace elements should be impossible to stare at—like trying to look directly at your peripheral vision. Test by asking: can a user demonstrate this feature to a friend? If yes, it's too prominent.

### Natural Over Mechanical

Every design decision asks: what would this feel like in nature?

In nature, light doesn't flip from day to night; it transitions through infinite gradations. Rain doesn't start and stop; it builds and fades. Temperature shifts gradually. Sound layers and evolves. Seasons turn.

Living Workspace mimics these patterns. We borrow the timing, textures, and transitions of natural systems—what designers call biomimicry. Not because nature is inherently better, but because human nervous systems spent millions of years calibrating to natural patterns. We recognize them in our bones.

A mechanical timer says "25:00... 24:59... 24:58..." and we feel the countdown pressure.

A natural environment says "the light is changing, the sounds are shifting, something is coming" and we feel it approaching without anxiety.

**Rule of thumb:** Before implementing any transition, ask: "Does this happen instantly anywhere in nature?" If not, it shouldn't happen instantly here. The minimum transition duration is 10 seconds; preferred is 30-60. Nature doesn't have cut edits.

### Presence Over Notification

The conventional approach to productivity feedback is notification: something happens, interrupt the user, demand acknowledgment. Living Workspace takes the opposite approach: presence.

The environment is always subtly communicating, but it never shouts. The colors tell you the time of day. The soundscape tells you your work state. The bells tell you about accumulated progress. But none of these demand response. They're just there, like the walls of a room you're working in.

You can ignore them entirely and the app functions perfectly well. But if you're receptive, the presence communicates.

**Rule of thumb:** No element of Living Workspace should ever require dismissal. If there's an X button, we've built a notification. If there's an "OK" button, we've built a dialog. Presence means the information is available; taking it in is always optional.

---

## Why This Is Right Now's Emotional Moat

Features can be copied in a quarter. Feelings take years to replicate.

Any competitor can implement a Pomodoro timer, a task list, Markdown storage, terminal integration. These are checkboxes in a feature comparison matrix. Living Workspace can't be captured in a matrix.

When someone asks "what do you like about Right Now?" and the answer is "I don't know, I just... like being in it," we've built something defensible. The preference is felt, not analyzed. It lives in the nervous system, not the rational mind.

This is the emotional moat: the quality of experience that makes people choose your product not because it has more features or costs less, but because it feels right.

The productivity tool market is crowded with competent software that nobody loves. Todoist has better collaboration. Things 3 has better design. OmniFocus has better power features. They're all fine. They all feel the same.

Living Workspace makes Right Now feel *different*. Not better on any measurable axis—different in kind. A productivity tool that wants you to thrive, not just perform. That treats you as a living being in an environment, not a cursor clicking checkboxes.

This feeling becomes identity. Users start saying "I'm a Right Now person" the way people say "I'm an Apple person"—not because of specs, but because of resonance.

---

## Technical Architecture

### MVP vs Full Vision

Not everything needs to ship at once. The smallest version of Living Workspace that still feels magical:

**MVP (ships first):**
- **Daylight Canvas**: Time-based palette shifts (heuristic, not solar-calculated). Even without soundscapes, an interface that warms through the day feels fundamentally different from a static one.
- **Completion bells**: Web Audio-synthesized bells with time-of-day variation. No soundscapes yet, just the moments of ceremony.

This MVP delivers the core emotional insight—your workspace breathes with you—in roughly 2-3 weeks of work. Users will feel the difference immediately. The palette alone is a revelation for anyone who's stared at the same white screen for eight hours.

**Full Vision (follows):**
- Stillwater soundscapes with state-aware transitions
- Bell evolution based on completion count (not just time)
- Geographic solar calculation
- Rhythm visualization
- Haptic feedback

The MVP is enough to validate the hypothesis that environmental design matters. The Full Vision is what makes it inimitable.

### System Integration

Living Workspace integrates with the existing Right Now architecture, extending rather than replacing current systems.

**Event Bus extension** (`src/lib/events.ts`):

New event types for Living Workspace:

```typescript
export interface DaylightUpdateEvent extends BaseEvent {
  type: "daylight_update";
  phase: 'dawn' | 'morning' | 'midday' | 'afternoon' | 'evening' | 'night';
  palette: DaylightPalette;
  transitionProgress: number;  // 0-1, for interpolation
}

export interface SoundscapeStateEvent extends BaseEvent {
  type: "soundscape_state";
  active: boolean;
  workState: WorkState;
  volume: number;
}

export interface ResonanceEvent extends BaseEvent {
  type: "resonance";
  completionIndex: number;
  bellParameters: BellParameters;
}
```

**Sound system extension** (`src/lib/sounds.ts`):

The existing `ISoundManager` (lines 24-70) handles file-based sound playback via Tauri. We add a parallel `WebAudioManager` for synthesized sounds:

```typescript
export class WebAudioManager {
  private context: AudioContext;
  private bellSynth: BellSynthesizer;
  private stillwater: StillwaterEngine;
  
  async playBell(params: BellParameters): Promise<void>;
  async setStillwaterState(state: WorkState): Promise<void>;
  async setStillwaterVolume(volume: number): Promise<void>;
}
```

The two managers coexist: `ISoundManager` for UI sounds (state changes, warnings), `WebAudioManager` for Living Workspace synthesis.

**Timer integration** (`src/lib/timer-logic.ts`):

The existing `computeStateChangeEvents` (lines 128-143) emits state transition events. Living Workspace subscribes to these for soundscape transitions:

```typescript
eventBus.subscribeByType("state_change", (event) => {
  stillwaterEngine.transitionToState(event.to);
});
```

### Performance Considerations

Living Workspace must be invisible not just perceptually but computationally. An environment that drains your battery is not an environment that supports your work.

**Web Audio API efficiency:**

- Oscillators and noise generators are lightweight; the synthesis is simple
- Use a single AudioContext, shared across all Living Workspace audio
- Suspend the AudioContext when the app is backgrounded
- All audio processing happens on the browser's audio thread, not blocking main thread

**CSS custom property updates:**

- Palette updates happen every 5 minutes maximum (debounced)
- CSS transitions handle interpolation, not JavaScript animation loops
- Only changed properties are updated
- Repaints are contained to color/background, not layout

**Memory management:**

- Soundscape generators allocate buffers once, reuse them
- Bell synthesis creates short-lived nodes (cleaned up after decay)
- No memory leaks from subscription chains (proper cleanup on unmount)

**Battery impact:**

- All audio can be disabled globally (no Web Audio API running)
- Daylight Canvas is CSS-only; minimal battery impact
- Generative audio uses simple synthesis, not heavy convolution

### Browser API Requirements

- **Web Audio API**: All modern browsers (Safari 14.1+, Chrome 35+, Firefox 25+)
- **CSS Custom Properties**: All modern browsers
- **Vibration API**: Mobile browsers only; gracefully degrades on desktop
- **localStorage**: For user preferences (already in use)

---

## What Success Looks Like

### Qualitative Signals

- Users describe Right Now as "cozy," "calm," "alive," or use similar emotional language
- Users report feeling less fatigued after long sessions compared to other tools
- Unprompted testimonials mention the environmental quality ("I love the colors," "the sounds help me focus")
- Users leave Living Workspace enabled by default after trying it
- Support tickets mention missing the feature when it breaks

### Quantitative Metrics

- **Feature adoption**: >60% of active users enable at least one Living Workspace subsystem
- **Retention delta**: Users with Living Workspace enabled show higher 30-day retention
- **Session duration**: Average session length increases (without increased frustration signals)
- **Break compliance**: Users more consistently take breaks rather than skipping them
- **Completion satisfaction**: Optional post-completion micro-survey shows higher satisfaction scores

### Anti-Metrics (Signals of Failure)

- Users frequently toggling features on/off (indicates inconsistency or annoyance)
- Bug reports about visual flickering or audio glitches
- Performance complaints (battery drain, app slowness)
- Accessibility complaints from colorblind users or users needing silence

---

## Open Questions

### Sound Design

- What specific soundscapes resonate most broadly? Need user research.
- Should users be able to choose from soundscape presets (rain, nature, synth) or is a single "Stillwater" enough?
- How do we handle headphones vs speakers vs system audio context?
- What's the right default volume? (Probably lower than you think.)

### Accessibility

**Visual:**
- Colorblind users: Do the daylight transitions work with protanopia/deuteranopia/tritanopia?
- Can users opt out of color changes entirely while keeping other features?
- How do we ensure sufficient contrast ratios across all palette states?

**Audio:**
- Users who need silence for accessibility reasons must have a frictionless disable
- Screen reader compatibility: Do soundscapes interfere?
- Hearing-impaired users: Visual alternatives for bell feedback?

**Motion:**
- Some users have vestibular disorders; ripple animations should respect `prefers-reduced-motion`

### Platform

- Does WebAudio synthesis work well in Tauri's webview on all platforms?
- Should bells use native synthesis (Rust) for lower latency?
- How do we handle system audio permission on macOS Sequoia+?

### Energy and Performance

- What's the actual battery impact of continuous low-level audio synthesis?
- On underpowered hardware, do we gracefully degrade to file-based sounds only?
- Memory footprint target for the synthesizer?

### Personalization

- Should users be able to customize palettes while maintaining the living quality?
- Time zone handling: What if someone travels? Auto-adapt or manual setting?
- Geographic accuracy: Is it worth computing actual sunrise/sunset vs time-based heuristics?

---

## Inspiration

**Things 3** — The gold standard for delightful productivity software. Proves that polish and feel matter. Their Today view's simplicity, the satisfying checkbox animation, the subtle sounds. They showed that a task manager can feel premium. Living Workspace takes this further into environmental immersion.

**Forest** — Pioneered the gamification of focus through growth metaphors. We take the opposite approach—no points, no streaks, no competition—but share the insight that productivity tools should engage more than the analytical mind.

**Endel** — Demonstrated that AI-generated ambient soundscapes can genuinely support focus. Their science-backed approach to generative audio informs Stillwater's design, though we aim for even more subtlety.

**f.lux / Night Shift** — Proved that users accept and appreciate automatic color temperature shifts for wellness. The core insight—that display color affects physiology—validates Daylight Canvas. We extend it from blue-light filtering to full palette breathing.

**Natural environments** — The real inspiration. How does it feel to work in a cabin with a fire crackling, rain on the windows, light shifting as clouds pass? How does it feel to complete something and hear a bell echoing across a valley? Living Workspace is a digital approximation of working in a place you love.

**Japanese tea ceremony** — The principle of *ichigo ichie* (one time, one meeting): every moment is unique and should be appreciated. Every task completion is a small ceremony, unique in its position in the day's rhythm, worthy of acknowledgment.

**Circadian lighting design** — Modern architecture increasingly uses dynamic lighting that shifts through the day. WeWork, Apple, and Google offices use this. Living Workspace brings the same principle to the digital workspace.

---

---

*Living Workspace transforms Right Now from a tool you use into a place you inhabit. You will open it in the morning and feel welcomed. You will complete a task and feel acknowledged. You will take a break and feel restored. You will close it at night and feel finished.*

*This is productivity software that understands you are not a machine. You are a living being that breathes, changes, and responds to your environment. And Right Now—finally—responds back.*
