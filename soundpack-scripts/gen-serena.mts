/// <reference types="bun-types" />
import { createFalClient } from "@fal-ai/client";
import { generateSounds } from "./generateSounds.mts";

const fal = createFalClient({
  credentials: () => process.env.FAL_KEY,
});

// Introducing the Character: Serena
// * Voice Style: Confident, warm, and direct.
// * Persona: She’s your capable, supportive partner-in-productivity—always encouraging, never patronizing.
// Generate the sounds
await generateSounds({
  trialName: "serena",
  generator: async (speechContent) => {
    // est fal-ai/playai/tts/v3 $0.85 for these
    const result = await fal.subscribe("fal-ai/playai/tts/v3", {
      input: { input: speechContent, voice: "Ava (English (AU)/Australian)" },
    });

    const response = await fetch(result.data.audio.url);
    const blob = await response.blob();
    // Idk the cost
    return { mp3Blob: blob, pennies: 1 };
  },
  variants: {
    session_start: [
      "Time to shine—show them how it's done.",
      "Let's focus up—you know what to do.",
      "New session, new wins. Let's go.",
    ],
    session_ended: ["Session's a wrap—great hustle.", "Done and dusted! Nice work.", "That's progress—proud of you."],
    todo_complete: [
      "Task done—keep that momentum going.",
      "You nailed it—another win on the board.",
      "Crushed it! On to the next.",
    ],
    break_approaching: [
      "Break's coming—wrap it up smoothly.",
      "Almost time to decompress—start winding down.",
      "Heads-up, time to ease off soon.",
    ],
    break_start: [
      "Break time—pause and recharge.",
      "Let it breathe—you've earned this.",
      "Time out—take a moment for yourself.",
    ],
    break_end_approaching: [
      "Break's almost up—get ready to refocus.",
      "Wind down the break—work's calling.",
      "Few more seconds and we're back on.",
    ],
    work_resumed: [
      "Alright, game on—let's crush this.",
      "Back at it—bring that energy.",
      "We're rolling again—keep the pace.",
    ],
    break_nudge: [
      "Hey, you've been grinding—time for a breather.",
      "Still at it? Let's not push too hard—take a pause.",
      "Time to step away—recharge for a moment.",
    ],
  },
});
