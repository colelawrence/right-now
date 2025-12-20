import { useEffect, useRef, useState } from "react";
import type { Clock } from "../lib/clock";
import { realClock } from "../lib/clock";
import { cn } from "./utils/cn";

interface TimerProps {
  /** For showing "time worked" */
  startTime: number;
  /** Showing "time left" */
  endTime: number;
  className?: string;
  /** Directly edited the timer and this is the difference between the new time and the original time */
  onAdjustTime?: (ms: number) => void;
  /** Clock instance for time operations (defaults to realClock) */
  clock?: Clock;
}

const TIME_PATTERNS = [
  // 1:30, 1:00, etc
  {
    regex: /^(\d+):(\d{1,2})$/,
    parse: (matches: RegExpMatchArray) => {
      const minutes = parseInt(matches[1], 10);
      const seconds = parseInt(matches[2], 10);
      return { ms: (minutes * 60 + seconds) * 1000 };
    },
  },
  // 90s, 90sec, 90seconds
  {
    regex: /^(\d+)\s*(s|sec|seconds?)$/i,
    parse: (matches: RegExpMatchArray) => ({ ms: parseInt(matches[1], 10) * 1000 }),
  },
  // 5m, 5min, 5minutes
  {
    regex: /^(\d+)\s*(m|min|minutes?)$/i,
    parse: (matches: RegExpMatchArray) => ({ ms: parseInt(matches[1], 10) * 60 * 1000 }),
  },
  // 1h, 1hr, 1hour, 1hours
  {
    regex: /^(\d+)\s*(h|hr|hours?)$/i,
    parse: (matches: RegExpMatchArray) => ({ ms: parseInt(matches[1], 10) * 60 * 60 * 1000 }),
  },
  // Plain numbers are interpreted as minutes
  {
    regex: /^(\d+)$/,
    parse: (matches: RegExpMatchArray) => ({ ms: parseInt(matches[1], 10) * 60 * 1000 }),
  },
];

function parseTimeInput(input: string): { ms: number } | null {
  input = input.trim();

  for (const pattern of TIME_PATTERNS) {
    const matches = input.match(pattern.regex);
    if (matches) {
      try {
        return pattern.parse(matches);
      } catch (e) {
        console.warn("Failed to parse time input:", e);
        return null;
      }
    }
  }

  return null;
}

export function Timer({ startTime, endTime, className, onAdjustTime, clock = realClock }: TimerProps) {
  const [time, setTime] = useState(clock.now());
  const [isEditing, setIsEditing] = useState(false);
  const [inputError, setInputError] = useState(false);
  const [isCountingDown, setIsCountingDown] = useState(true);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const intervalId = clock.setInterval(() => {
      if (!isEditing) {
        setTime(clock.now());
      }
    }, 500);
    return () => clock.clearInterval(intervalId);
  }, [isEditing, clock]);

  // Calculate delta based on mode.
  let delta: number;
  let overtime = false;
  if (isCountingDown) {
    delta = endTime - time;
    if (delta < 0) {
      overtime = true;
      delta = Math.abs(delta);
    }
  } else {
    // Count up mode shows elapsed work time.
    delta = time - startTime;
    if (time > endTime) {
      overtime = true;
    }
  }

  // Format minutes and seconds using the absolute delta.
  const displayMinutes = Math.floor(delta / 60000);
  const displaySeconds = Math.floor((delta % 60000) / 1000);
  const formattedTime = `${displayMinutes}:${displaySeconds.toString().padStart(2, "0")}`;

  // Prepare an optional overtime indicator.
  const overtimeIndicator = overtime ? (isCountingDown ? "-" : "+") : "";

  const handleClick = () => {
    setIsCountingDown(!isCountingDown);
  };

  const handleDoubleClick = () => {
    if (!onAdjustTime) return;
    setIsEditing(true);
    setTimeout(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    }, 0);
  };

  const handleInputBlur = () => {
    setIsEditing(false);
    setInputError(false);
  };

  const handleInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      const input = e.currentTarget.value;
      const match = parseTimeInput(input);
      if (!match) {
        setInputError(true);
        return;
      }

      onAdjustTime?.(match.ms);
      setIsEditing(false);
      setInputError(false);
    } else if (e.key === "Escape") {
      setIsEditing(false);
      setInputError(false);
    }
  };

  if (isEditing) {
    return (
      <input
        ref={inputRef}
        type="text"
        className={cn(
          "max-w-16 text-start rounded bg-transparent border-none field-sizing-content p-1",
          inputError ? "border-red-500" : "border-gray-300",
          className,
        )}
        defaultValue={`${formattedTime}`}
        placeholder="1:30, 5m..."
        onBlur={handleInputBlur}
        onKeyDown={handleInputKeyDown}
        onChange={() => inputError && setInputError(false)}
      />
    );
  }

  return (
    <span
      className={cn(
        "p-2 group cursor-pointer hover:text-blue-600 flex items-center gap-1",
        overtimeIndicator ? "text-red-700 animate-pulse" : "",
        className,
      )}
      onClick={handleClick}
      onDoubleClick={handleDoubleClick}
      title={
        `${startTime}->${endTime}|` +
        (onAdjustTime
          ? "Double-click to edit (examples: 5m, 1:30, 90s). Click to toggle count mode."
          : "Click to toggle between time elapsed and time remaining")
      }
    >
      {overtimeIndicator}
      {formattedTime}
    </span>
  );
}
