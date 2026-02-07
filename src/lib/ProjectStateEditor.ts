import matter from "gray-matter";

const TASK_RE = /^(\s*[-\*]?\s*)\[([xX\s])\]\s+(.*)$/;

/**
 * Regex for extracting task ID token from task name.
 * Matches: [abc.derived-label] (3-4 letter prefix + dot + kebab-case label)
 * Must come before session badge if present
 */
const TASK_ID_RE = /\s+\[([a-z]{3,4})\.([a-z0-9-]+)\](?=\s+\[(?:Running|Stopped|Waiting)\]|$)/;

/**
 * Regex for extracting session badge from task name.
 * Matches: [Status](todos://session/<id>) at end of line
 * Status can be: Running, Stopped, Waiting
 */
const SESSION_BADGE_RE = /\s+\[(Running|Stopped|Waiting)\]\(todos:\/\/session\/(\d+)\)$/;

/**
 * Session status type matching the daemon protocol
 */
export type SessionStatus = "Running" | "Stopped" | "Waiting";

/**
 * Session info extracted from a task's session badge
 */
export interface TaskSessionStatus {
  status: SessionStatus;
  sessionId: number;
}

/**
 * Extract task ID token and session badge from task name
 */
function extractTaskMetadata(fullName: string): {
  name: string;
  taskId: string | null;
  sessionStatus: TaskSessionStatus | null;
} {
  let name = fullName;
  let taskId: string | null = null;
  let sessionStatus: TaskSessionStatus | null = null;

  // Extract session badge first (it's at the very end)
  const sessionMatch = name.match(SESSION_BADGE_RE);
  if (sessionMatch) {
    const status = sessionMatch[1] as SessionStatus;
    const sessionId = parseInt(sessionMatch[2], 10);
    sessionStatus = { status, sessionId };
    name = name.replace(SESSION_BADGE_RE, "");
  }

  // Extract task ID (comes before session badge, so we extract from the cleaned name)
  const taskIdMatch = name.match(TASK_ID_RE);
  if (taskIdMatch) {
    const prefix = taskIdMatch[1];
    const label = taskIdMatch[2];
    taskId = `${prefix}.${label}`;
    name = name.replace(TASK_ID_RE, "");
  }

  return { name, taskId, sessionStatus };
}

/**
 * Format a task ID token for insertion into a task line
 */
export function formatTaskId(taskId: string): string {
  return ` [${taskId}]`;
}

/**
 * Format a session badge for insertion into a task line
 */
export function formatSessionBadge(status: SessionStatus, sessionId: number): string {
  return ` [${status}](todos://session/${sessionId})`;
}

function parseTaskLine(line: string): {
  prefix: string;
  complete: string | false;
  name: string;
  taskId: string | null;
  sessionStatus: TaskSessionStatus | null;
} | null {
  const match = line.match(TASK_RE);
  if (!match) return null;

  const fullName = match[3];
  const { name, taskId, sessionStatus } = extractTaskMetadata(fullName);

  return {
    prefix: match[1],
    complete: match[2].trim() || false,
    name,
    taskId,
    sessionStatus,
  };
}
function parseHeadingLine(line: string): { level: number; text: string } | null {
  const match = line.match(/^(#{1,6})\s+(.*)$/);
  if (!match) return null;
  return { level: match[1].length, text: match[2] };
}

/**
 * Task block with optional task ID and session status
 */
export interface TaskBlock {
  type: "task";
  name: string;
  details: string | null;
  complete: string | false;
  prefix: string;
  taskId: string | null;
  sessionStatus: TaskSessionStatus | null;
}

/**
 * The ProjectMarkdown type represents parsed markdown blocks
 */
export type ProjectMarkdown =
  | TaskBlock
  | { type: "heading"; level: number; text: string }
  | { type: "unrecognized"; markdown: string };

export interface ProjectFile {
  pomodoroSettings: {
    workDuration: number;
    breakDuration: number;
  };
  workState?: "planning" | "working" | "break";
  stateTransitions?: {
    startedAt: number;
    endsAt?: number;
  };
  markdown: ProjectMarkdown[];
}

/**
 * Generate a stable task ID from task name with collision avoidance
 */
export function generateTaskId(taskName: string, existingIds: Set<string>): string {
  // Derive label from task name: lowercase, keep alphanumeric and hyphens
  const rawLabel = taskName
    .toLowerCase()
    .replace(/[^a-z0-9\s-]/g, "")
    .trim()
    .replace(/\s+/g, "-")
    .substring(0, 40); // Keep it reasonably short

  // Ensure label is non-empty and doesn't start/end with hyphens
  const label = rawLabel.replace(/-+/g, "-").replace(/^-+|-+$/g, "") || "task";

  // Generate 3-letter prefix
  const chars = "abcdefghijklmnopqrstuvwxyz";

  // Try 3-letter prefix first
  for (let attempt = 0; attempt < 100; attempt++) {
    const prefix = Array.from({ length: 3 }, () => chars[Math.floor(Math.random() * chars.length)]).join("");

    const candidate = `${prefix}.${label}`;
    if (!existingIds.has(candidate)) {
      return candidate;
    }
  }

  // Fallback to 4-letter prefix if 3-letter has too many collisions
  for (let attempt = 0; attempt < 100; attempt++) {
    const prefix = Array.from({ length: 4 }, () => chars[Math.floor(Math.random() * chars.length)]).join("");

    const candidate = `${prefix}.${label}`;
    if (!existingIds.has(candidate)) {
      return candidate;
    }
  }

  // Last resort: append random suffix to a 4-letter prefix
  const prefix = Array.from({ length: 4 }, () => chars[Math.floor(Math.random() * chars.length)]).join("");

  return `${prefix}.${label}-${Date.now()}`;
}

/**
 * Ensure a task has a stable ID, generating one if missing
 */
export function ensureTaskId(task: TaskBlock, existingIds: Set<string>): void {
  if (!task.taskId) {
    task.taskId = generateTaskId(task.name, existingIds);
    existingIds.add(task.taskId);
  }
}

export namespace ProjectStateEditor {
  /**
   * Move an entire heading section (heading + all content until next heading) up or down.
   *
   * @param content - The full markdown content (with frontmatter)
   * @param headingIndex - The index of the heading in the blocks array (not line number)
   * @param direction - "up" or "down"
   * @returns Updated markdown content, or null if the move is invalid/no-op
   */
  export function moveHeadingSection(content: string, headingIndex: number, direction: "up" | "down"): string | null {
    const state = parse(content);
    const blocks = state.markdown;

    // Find all heading indices
    const headingIndices: number[] = [];
    blocks.forEach((block, idx) => {
      if (block.type === "heading") {
        headingIndices.push(idx);
      }
    });

    // Validate that headingIndex points to a heading
    if (!headingIndices.includes(headingIndex)) {
      return null;
    }

    // Find which heading number this is (0-based among headings)
    const headingPosition = headingIndices.indexOf(headingIndex);

    // Determine section boundaries
    const sectionStart = headingIndex;
    const nextHeadingIdx = headingIndices[headingPosition + 1];
    const sectionEnd = nextHeadingIdx !== undefined ? nextHeadingIdx : blocks.length;

    if (direction === "up") {
      // Can't move the first section up
      if (headingPosition === 0) {
        return null;
      }

      // Find previous section boundaries
      const prevSectionStart = headingIndices[headingPosition - 1];
      const prevSectionEnd = sectionStart;

      // Extract sections
      const prevSection = blocks.slice(prevSectionStart, prevSectionEnd);
      const currentSection = blocks.slice(sectionStart, sectionEnd);

      // Rebuild blocks with swapped sections
      const newBlocks = [
        ...blocks.slice(0, prevSectionStart),
        ...currentSection,
        ...prevSection,
        ...blocks.slice(sectionEnd),
      ];

      state.markdown = newBlocks;
      return update(content, state);
    } else {
      // direction === "down"
      // Can't move the last section down
      if (headingPosition === headingIndices.length - 1) {
        return null;
      }

      // Find next section boundaries
      const nextSectionStart = nextHeadingIdx;
      const nextNextHeadingIdx = headingIndices[headingPosition + 2];
      const nextSectionEnd = nextNextHeadingIdx !== undefined ? nextNextHeadingIdx : blocks.length;

      // Extract sections
      const currentSection = blocks.slice(sectionStart, sectionEnd);
      const nextSection = blocks.slice(nextSectionStart, nextSectionEnd);

      // Rebuild blocks with swapped sections
      const newBlocks = [
        ...blocks.slice(0, sectionStart),
        ...nextSection,
        ...currentSection,
        ...blocks.slice(nextSectionEnd),
      ];

      state.markdown = newBlocks;
      return update(content, state);
    }
  }

  /**
   * Parse the entire Markdown content into:
   * 1. Frontmatter -> ProjectState fields
   * 2. Body -> Array of ProjectMarkdown (tasks, headings, unrecognized)
   */
  export function parse(content: string): ProjectFile {
    // 1. Extract frontmatter + body
    const file = matter(content);
    const front = file.data || {};

    // 2. Build the ProjectState from frontmatter
    //    Use defaults where fields might not exist
    const projectState: ProjectFile = {
      pomodoroSettings: {
        workDuration: front.pomodoro_settings?.work_duration ?? 25,
        breakDuration: front.pomodoro_settings?.break_duration ?? 5,
      },
      // Parse timer state from namespaced frontmatter (right_now)
      workState: front.right_now?.work_state,
      stateTransitions: front.right_now?.state_transitions
        ? {
            startedAt:
              typeof front.right_now.state_transitions.started_at === "number"
                ? front.right_now.state_transitions.started_at
                : undefined,
            endsAt:
              typeof front.right_now.state_transitions.ends_at === "number"
                ? front.right_now.state_transitions.ends_at
                : undefined,
          }
        : undefined,
      // 3. Parse the body into structured blocks
      markdown: parseBody(file.content),
    };

    return projectState;
  }

  /**
   * Update the entire Markdown file from the given ProjectState.
   * Steps:
   * 1. Re-inject updated frontmatter
   * 2. Re-stringify the parsed body from `state.markdown`
   */
  export function update(originalContent: string, state: ProjectFile): string {
    // 1. Parse existing frontmatter/body so we can rewrite the frontmatter
    const file = matter(originalContent);
    const front = file.data || {};

    // Update pomodoro settings
    front.pomodoro_settings = front.pomodoro_settings || {};
    front.pomodoro_settings.work_duration = state.pomodoroSettings.workDuration;
    front.pomodoro_settings.break_duration = state.pomodoroSettings.breakDuration;

    // Update timer state in namespaced frontmatter (right_now)
    if (state.workState !== undefined || state.stateTransitions !== undefined) {
      front.right_now = front.right_now || {};

      if (state.workState !== undefined) {
        front.right_now.work_state = state.workState;
      }

      if (state.stateTransitions !== undefined) {
        front.right_now.state_transitions = {
          started_at: state.stateTransitions.startedAt,
          ...(state.stateTransitions.endsAt !== undefined && { ends_at: state.stateTransitions.endsAt }),
        };
      }
    }

    // 4. Rebuild the body from the current markdown blocks
    const updatedBody = stringifyProjectMarkdown(state.markdown);

    // 5. Combine frontmatter + body using gray-matter
    //    `matter.stringify` automatically wraps the YAML with --- delimiters
    return matter.stringify(updatedBody, front);
  }
}

/* -------------------------------------------------------------------
 *                BODY PARSING & STRINGIFY HELPERS
 * ------------------------------------------------------------------- */

/**
 * Parse the body (Markdown after frontmatter) into an array of ProjectMarkdown blocks:
 * - `heading`: lines matching /^#{1,6}\s+/
 * - `task`: lines matching {@link parseTaskLine}
 *           plus subsequent lines as "details" until the next heading/task or EOF
 * - `unrecognized`: everything else, aggregated to preserve original format
 */
function parseBody(body: string): ProjectMarkdown[] {
  const lines = body.split("\n");
  const blocks: ProjectMarkdown[] = [];

  let i = 0;
  let unrecognizedBuffer: string[] = [];

  const flushUnrecognized = () => {
    if (unrecognizedBuffer.length > 0) {
      blocks.push({
        type: "unrecognized",
        markdown: unrecognizedBuffer.join("\n"),
      });
      unrecognizedBuffer = [];
    }
  };

  while (i < lines.length) {
    const line = lines[i];

    // 1. Check for a heading: e.g., "## My heading"
    const headingMatch = parseHeadingLine(line);
    if (headingMatch) {
      // Flush any unrecognized content before this heading
      flushUnrecognized();
      blocks.push({ type: "heading", ...headingMatch });
      i += 1;
      continue;
    }

    // 2. Check for a task: e.g., "- [ ] My task"
    const taskMatch = parseTaskLine(line);
    if (taskMatch) {
      // Flush any unrecognized content before this task
      flushUnrecognized();
      // Gather subsequent lines until the next heading/task (or EOF)
      let detailsLines: string[] = [];
      let j = i + 1;
      while (j < lines.length) {
        const nextLine = lines[j];
        // If next line is a heading, a new task, or a blank line, stop
        if (isHeading(nextLine) || isTask(nextLine) || nextLine.trim() === "") {
          break;
        }
        detailsLines.push(nextLine);
        j++;
      }

      // Add the "task" block
      blocks.push({
        type: "task",
        ...taskMatch,
        details: detailsLines.length > 0 ? detailsLines.join("\n") : null,
      });

      // Advance i to skip over details
      i = j;
      continue;
    }

    // 3. If neither heading nor task, collect as unrecognized
    unrecognizedBuffer.push(line);
    i += 1;
  }

  // Flush any remaining unrecognized lines
  flushUnrecognized();

  return blocks;
}

/**
 * Helper function to detect if a line is a heading
 */
function isHeading(line: string) {
  return parseHeadingLine(line) != null;
}

/** Helper function to detect if a line is a task */
function isTask(line: string) {
  return parseTaskLine(line) != null;
}

/** Stringify an array of ProjectMarkdown blocks back into a single Markdown string. */
function stringifyProjectMarkdown(blocks: ProjectMarkdown[]): string {
  return blocks
    .map((block) => {
      switch (block.type) {
        case "heading": {
          const hashes = "#".repeat(block.level);
          return `${hashes} ${block.text}`;
        }
        case "task": {
          // Build the task line with optional task ID and session badge
          // Order: task name → task ID → session badge
          // Preserve the original prefix (indentation + bullet style) if available
          const prefix = block.prefix || "- ";
          const taskIdToken = block.taskId ? formatTaskId(block.taskId) : "";
          const badge = block.sessionStatus
            ? formatSessionBadge(block.sessionStatus.status, block.sessionStatus.sessionId)
            : "";
          const lines = [`${prefix}[${block.complete || " "}] ${block.name}${taskIdToken}${badge}`];
          if (block.details) {
            lines.push(block.details);
          }
          return lines.join("\n");
        }
        case "unrecognized":
          return block.markdown;
      }
    })
    .join("\n");
}
