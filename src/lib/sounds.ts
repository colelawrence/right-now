import { invoke } from "@tauri-apps/api/core";
import { resolveResource } from "@tauri-apps/api/path";
import type { WritableAtom } from "jotai";
import { MapOrSetDefault } from "./MapOrSetDefault";
import { debounce } from "./debounce";
import type { JotaiStore } from "./jotai-types";

export enum SoundEventName {
  SessionStart = "session_start",
  SessionEnd = "session_end",
  TodoComplete = "todo_complete",
  BreakApproaching = "break_approaching",
  BreakStart = "break_start",
  BreakEndApproaching = "break_end_approaching",
  WorkResumed = "work_resumed",
  BreakNudge = "break_nudge",
}

// How many seconds before state change to play warning sound
export const WARNING_THRESHOLD_MS = 60 * 1000; // 1 minute

export type SoundPack = {
  name: string;
  isSelectedAtom: WritableAtom<boolean, [true], void>;
};

export interface ISoundManager {
  playSound(event: SoundEventName): void;
  listSoundPacks(): Promise<string[]>;
  listSoundVariations(event: SoundEventName): Promise<string[]>;
}

export class ISoundManager implements ISoundManager {
  private soundPackPath: string;
  // Start at a random sound for this session
  private invocationCounter = new MapOrSetDefault((name: string) => Math.floor(Math.random() * 100));
  // Debounce the sound play to avoid spamming the system for example when the user is returning from sleep
  private playSoundDebounced = debounce(1000, async (event: SoundEventName) => {
    try {
      await invoke("play_sound", {
        soundPackPath: this.soundPackPath,
        name: event,
        // Increment counter to cycle through variations
        invocation: this.invocationCounter.update(event, (invocation) => invocation + 1),
      });
    } catch (error) {
      console.error(`Failed to play sound ${event}:`, error);
      // Don't throw - we want to fail silently if sound doesn't work
    }
  });

  constructor(
    private store: JotaiStore,
    soundPackPath: string,
  ) {
    this.soundPackPath = soundPackPath;
  }

  static async initialize(store: JotaiStore): Promise<ISoundManager> {
    // For now, use the hardcoded path from main.tsx
    // TODO: Use appDataDir() to get the proper location once we implement sound pack installation
    const soundPackPath = await resolveResource("resources/Serena.v0.zip");
    return new ISoundManager(store, soundPackPath);
  }

  playSound(event: SoundEventName): void {
    this.playSoundDebounced(event);
  }

  async listSoundPacks(): Promise<string[]> {
    // TODO: Implement once we add sound pack installation support
    return [this.soundPackPath];
  }

  async listSoundVariations(event: SoundEventName): Promise<string[]> {
    try {
      return await invoke("list_sound_variations", {
        soundPackPath: this.soundPackPath,
        name: event,
      });
    } catch (error) {
      console.error(`Failed to list sound variations for ${event}:`, error);
      return [];
    }
  }
}
