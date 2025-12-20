import matter from "gray-matter";

const TASK_RE = /^(\s*[-\*]?\s*)\[([xX\s])\]\s+(.*)$/;

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
 * Extract session badge from task name, returning the clean name and session status
 */
function extractSessionBadge(fullName: string): {
  name: string;
  sessionStatus: TaskSessionStatus | null;
} {
  const match = fullName.match(SESSION_BADGE_RE);
  if (!match) {
    return { name: fullName, sessionStatus: null };
  }

  const status = match[1] as SessionStatus;
  const sessionId = parseInt(match[2], 10);

  // Remove badge from name
  const name = fullName.replace(SESSION_BADGE_RE, "");

  return {
    name,
    sessionStatus: { status, sessionId },
  };
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
  sessionStatus: TaskSessionStatus | null;
} | null {
  const match = line.match(TASK_RE);
  if (!match) return null;

  const fullName = match[3];
  const { name, sessionStatus } = extractSessionBadge(fullName);

  return {
    prefix: match[1],
    complete: match[2].trim() || false,
    name,
    sessionStatus,
  };
}
function parseHeadingLine(line: string): { level: number; text: string } | null {
  const match = line.match(/^(#{1,6})\s+(.*)$/);
  if (!match) return null;
  return { level: match[1].length, text: match[2] };
}

/**
 * Task block with optional session status
 */
export interface TaskBlock {
  type: "task";
  name: string;
  details: string | null;
  complete: string | false;
  prefix: string;
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
  markdown: ProjectMarkdown[];
}

export namespace ProjectStateEditor {
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
          // Build the task line with optional session badge
          // Preserve the original prefix (indentation + bullet style) if available
          const prefix = block.prefix || "- ";
          const badge = block.sessionStatus
            ? formatSessionBadge(block.sessionStatus.status, block.sessionStatus.sessionId)
            : "";
          const lines = [`${prefix}[${block.complete || " "}] ${block.name}${badge}`];
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
