/**
 * Event-driven architecture for observable application events.
 *
 * This module provides an EventBus for decoupling event producers from
 * consumers. It enables:
 * - Observable application events for testing
 * - Decoupled sound playback (sound player subscribes to events)
 * - Event history for test assertions
 *
 * @example
 * const eventBus = new AppEventBus();
 * eventBus.subscribe((event) => console.log(event));
 * eventBus.emit({ type: 'sound', timestamp: Date.now(), sound: 'todo_complete', reason: 'task done' });
 */

import type { WorkState } from "./project";
import type { SoundEventName } from "./sounds";

/** Base fields present on all events */
interface BaseEvent {
  timestamp: number;
}

/** Sound playback request */
export interface SoundEvent extends BaseEvent {
  type: "sound";
  sound: SoundEventName;
  reason: string;
}

/** Work state transition */
export interface StateChangeEvent extends BaseEvent {
  type: "state_change";
  from: WorkState;
  to: WorkState;
}

/** Timer approaching threshold warning */
export interface WarningEvent extends BaseEvent {
  type: "warning";
  state: WorkState;
  timeLeft: number;
}

/** Task marked complete */
export interface TaskCompletedEvent extends BaseEvent {
  type: "task_completed";
  taskName: string;
}

/** Timer tick for UI updates */
export interface TimerTickEvent extends BaseEvent {
  type: "timer_tick";
  timeLeft: number;
  overtime: boolean;
}

/** Union type of all application events */
export type AppEvent = SoundEvent | StateChangeEvent | WarningEvent | TaskCompletedEvent | TimerTickEvent;

/** Extract event type string literals for type discrimination */
export type AppEventType = AppEvent["type"];

/** Event handler function type */
export type EventHandler<T extends AppEvent = AppEvent> = (event: T) => void;

/**
 * Interface for the application event bus.
 * Allows dependency injection and testing.
 */
export interface EventBus {
  /**
   * Emits an event to all subscribers.
   * @param event The event to emit
   */
  emit(event: AppEvent): void;

  /**
   * Subscribes to all events.
   * @param handler Function called for each event
   * @returns Unsubscribe function
   */
  subscribe(handler: EventHandler): () => void;

  /**
   * Subscribes to events of a specific type.
   * @param type The event type to filter for
   * @param handler Function called for matching events
   * @returns Unsubscribe function
   */
  subscribeByType<T extends AppEventType>(type: T, handler: EventHandler<Extract<AppEvent, { type: T }>>): () => void;

  /**
   * Returns a copy of all emitted events (for testing).
   */
  getHistory(): AppEvent[];

  /**
   * Clears event history (for test isolation).
   */
  clearHistory(): void;
}

/**
 * Implementation of the application event bus.
 *
 * Features:
 * - Synchronous event delivery
 * - Reentrant emission (events emitted by handlers are delivered to all subscribers)
 * - Error isolation (subscriber errors don't prevent delivery to other subscribers)
 * - Event history for testing
 */
export class AppEventBus implements EventBus {
  private subscribers = new Set<EventHandler>();
  private history: AppEvent[] = [];

  emit(event: AppEvent): void {
    // Add to history before delivery
    this.history.push(event);

    // Deliver to all current subscribers
    // Take a snapshot of subscribers to handle reentrant subscriptions
    const currentSubscribers = Array.from(this.subscribers);

    for (const handler of currentSubscribers) {
      try {
        handler(event);
      } catch (error) {
        // Error isolation: log but don't prevent other subscribers from receiving
        console.error("EventBus subscriber error:", error);
      }
    }
  }

  subscribe(handler: EventHandler): () => void {
    this.subscribers.add(handler);
    return () => {
      this.subscribers.delete(handler);
    };
  }

  subscribeByType<T extends AppEventType>(type: T, handler: EventHandler<Extract<AppEvent, { type: T }>>): () => void {
    const wrappedHandler: EventHandler = (event) => {
      if (event.type === type) {
        handler(event as Extract<AppEvent, { type: T }>);
      }
    };
    return this.subscribe(wrappedHandler);
  }

  getHistory(): AppEvent[] {
    // Return a copy to prevent external mutation
    return [...this.history];
  }

  clearHistory(): void {
    this.history = [];
  }

  /** Returns the number of active subscribers */
  getSubscriberCount(): number {
    return this.subscribers.size;
  }
}

/**
 * Creates a new AppEventBus instance.
 * Convenience factory function.
 */
export function createEventBus(): EventBus {
  return new AppEventBus();
}
