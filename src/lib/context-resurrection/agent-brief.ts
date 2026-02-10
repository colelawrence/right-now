import { tailToExcerpt } from "./selectors";
import type { ContextSnapshotV1 } from "./types";

export type AgentBriefOptions = {
  /** Max lines of terminal tail to include (when tail_inline exists). */
  maxTailLines?: number;
};

function formatHeaderLine(label: string, value: string | number | null | undefined): string {
  if (value == null) return `${label}: (none)`;
  const str = typeof value === "number" ? String(value) : value.trim();
  return `${label}: ${str.length === 0 ? "(none)" : str}`;
}

function fencedTextBlock(text: string): string {
  const trimmed = text.trim();
  if (trimmed.length === 0) return "";
  return `\n\n\`\`\`text\n${trimmed}\n\`\`\``;
}

/**
 * Turn a CR snapshot into an agent-friendly prompt you can paste into Pi / an LLM.
 *
 * Pure + deterministic (no Date.now usage).
 */
export function formatAgentBrief(snapshot: ContextSnapshotV1, options: AgentBriefOptions = {}): string {
  const maxTailLines = typeof options.maxTailLines === "number" && options.maxTailLines > 0 ? options.maxTailLines : 40;

  const lines: string[] = [];

  lines.push("You are an AI coding assistant. Help me resume the task below.");
  lines.push("");

  lines.push(formatHeaderLine("Project (TODO.md)", snapshot.project_path));
  lines.push(formatHeaderLine("Task", snapshot.task_id));
  lines.push(formatHeaderLine("Title at capture", snapshot.task_title_at_capture));
  lines.push("");

  lines.push("Snapshot:");
  lines.push(`- captured_at: ${snapshot.captured_at}`);
  lines.push(`- reason: ${snapshot.capture_reason}`);

  if (snapshot.user_note && snapshot.user_note.trim().length > 0) {
    lines.push("");
    lines.push("User note:");
    lines.push(fencedTextBlock(snapshot.user_note).trimStart());
  }

  const t = snapshot.terminal;
  if (t) {
    lines.push("");
    lines.push("Terminal:");
    lines.push(`- session_id: ${t.session_id}`);
    lines.push(`- status: ${t.status}`);
    if (t.exit_code !== undefined) lines.push(`- exit_code: ${t.exit_code}`);

    if (t.last_attention) {
      lines.push("- last_attention:");
      lines.push(`  - attention_type: ${t.last_attention.attention_type}`);
      lines.push(`  - triggered_at: ${t.last_attention.triggered_at}`);
      if (t.last_attention.preview.trim().length > 0) {
        lines.push("  - preview:");
        lines.push(fencedTextBlock(t.last_attention.preview).replace(/^/gm, "    ").trimEnd());
      }
    }

    if (t.tail_inline && t.tail_inline.trim().length > 0) {
      const excerpt = tailToExcerpt(t.tail_inline, maxTailLines);
      lines.push("");
      lines.push(`Terminal tail (last ${maxTailLines} lines):`);
      lines.push(fencedTextBlock(excerpt).trimStart());
    } else if (t.tail_path && t.tail_path.trim().length > 0) {
      lines.push("");
      lines.push(formatHeaderLine("Terminal tail path", t.tail_path));
    } else {
      lines.push("");
      lines.push("Terminal tail: (none)");
    }
  } else {
    lines.push("");
    lines.push("Terminal: (none captured)");
  }

  lines.push("");
  lines.push("Please respond with:");
  lines.push("1) A brief summary of what happened / current state");
  lines.push("2) The most likely next steps (commands + files to inspect)");
  lines.push("3) Any questions you need answered to proceed");

  return (
    lines
      .join("\n")
      .replace(/\n{3,}/g, "\n\n")
      .trim() + "\n"
  );
}
