/**
 * Sound player that subscribes to EventBus and plays sounds.
 *
 * This module decouples sound playback from event emission. The SoundPlayer
 * acts as an "edge" subscriber that converts SoundEvents into actual audio
 * playback via ISoundManager.
 *
 * In production, create and wire the SoundPlayer at app initialization.
 * In tests, you can verify sound events are emitted without this subscriber.
 *
 * @example
 * const eventBus = new AppEventBus();
 * const soundPlayer = createSoundPlayer(eventBus, soundManager);
 * // Sound events are now automatically played
 *
 * // Cleanup when done
 * soundPlayer.dispose();
 */

import type { EventBus, SoundEvent } from "./events";
import type { ISoundManager } from "./sounds";

/**
 * Sound player that subscribes to EventBus and plays sounds via ISoundManager.
 *
 * Listens for SoundEvents and triggers the appropriate sound playback.
 * Must be disposed when no longer needed to clean up the subscription.
 */
export class SoundPlayer {
  private unsubscribe: () => void;

  /**
   * Creates a SoundPlayer and subscribes to the EventBus.
   *
   * @param eventBus The event bus to subscribe to
   * @param soundManager The sound manager to use for playback
   */
  constructor(
    private eventBus: EventBus,
    private soundManager: ISoundManager,
  ) {
    this.unsubscribe = eventBus.subscribeByType("sound", this.handleSoundEvent);
  }

  /**
   * Handles incoming sound events by playing the appropriate sound.
   */
  private handleSoundEvent = (event: SoundEvent): void => {
    this.soundManager.playSound(event.sound);
  };

  /**
   * Disposes the sound player and unsubscribes from the event bus.
   * Call this when the sound player is no longer needed.
   */
  dispose(): void {
    this.unsubscribe();
  }
}

/**
 * Creates and returns a new SoundPlayer instance.
 * Convenience factory function.
 *
 * @param eventBus The event bus to subscribe to
 * @param soundManager The sound manager to use for playback
 * @returns A new SoundPlayer instance
 */
export function createSoundPlayer(eventBus: EventBus, soundManager: ISoundManager): SoundPlayer {
  return new SoundPlayer(eventBus, soundManager);
}
