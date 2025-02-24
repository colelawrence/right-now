/// <reference types="bun-types" />
import { createFalClient } from "@fal-ai/client";
import { generateSounds } from "./generateSounds.mts";

const fal = createFalClient({
  credentials: () => process.env.FAL_KEY,
});

// Generate the sounds
await generateSounds({
  trialName: "jennifer-chaplin",
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
      "Ladies and gentlemen, the stage is set—let the grand performance of your day commence!",
      "Now, dear friends, let us step into the light of a new adventure in productivity.",
      "The curtain rises on a day of splendid endeavors; it's time to dazzle the world!",
    ],
    session_ended: [
      "A magnificent performance! Take a bow, you've earned this moment of triumph.",
      "As the curtain falls on today's show, remember every scene was worth the effort.",
      "Another spectacular show comes to a close—until we meet again, dear friend!",
    ],
    todo_complete: [
      "Bravo! Another masterful scene perfectly executed!",
      "With the grace of a true artist, you've completed another wonderful act.",
      "A delightful achievement! Your performance continues to sparkle.",
    ],
    break_approaching: [
      "A gentle intermission draws near—finish your act with a flourish!",
      "The hour for a brief pause is upon us; let your final lines sparkle with charm.",
      "Remember, even the most delightful show must pause for a moment of reflection.",
    ],
    break_start: [
      "Ah, a pause in the performance—take a moment to savor the quiet between acts.",
      "Time for an intermission, my friend—let us relish a brief escape from the hustle.",
      "Step off the stage for a moment; even the brightest star needs a little rest.",
    ],
    break_end_approaching: [
      "The intermission draws to a close—prepare to return to the spotlight with renewed grace.",
      "Our pause is nearly over; gather your spirit as the next act awaits.",
      "Just a little longer, and we shall resume our performance—ready your heart for the next scene.",
    ],
    work_resumed: [
      "The show resumes! Step back into the limelight and let your brilliance unfold.",
      "Now, let us return to the stage where creativity and effort meet in splendid harmony.",
      "Lift the curtain once more—your next act of ingenuity awaits!",
    ],
    break_nudge: [
      "Even the most tireless performer savors a pause—consider a brief interlude for yourself.",
      "Remember, a moment of respite can add charm to your act; perhaps it's time for a gentle pause?",
      "In the midst of your brilliant routine, do take a breath—every great show enjoys an intermission.",
    ],
  },
});
