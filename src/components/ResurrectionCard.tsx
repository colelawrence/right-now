import { IconClock, IconTerminal, IconX } from "@tabler/icons-react";
import { useEffect, useMemo, useState } from "react";
import { selectCardData } from "../lib/context-resurrection/selectors";
import type { ContextSnapshotV1 } from "../lib/context-resurrection/types";
import { cn } from "./utils/cn";

export interface ResurrectionCardProps {
  snapshot: ContextSnapshotV1;
  onDismiss: () => void;
  onResume?: () => void | Promise<void>;
  onSaveNote?: (note: string) => void | Promise<void>;
  onForgetTaskContext?: () => void | Promise<void>;
  onForgetProjectContext?: () => void | Promise<void>;
  className?: string;
}

function formatLocalTime(iso: string): string {
  const ms = Date.parse(iso);
  if (!Number.isFinite(ms)) return iso;
  return new Date(ms).toLocaleString();
}

function statusBadge(status: string | undefined) {
  const base = "text-xs px-2 py-0.5 rounded-full font-medium";
  if (status === "Running") return <span className={cn(base, "bg-green-100 text-green-700")}>Running</span>;
  if (status === "Waiting") return <span className={cn(base, "bg-yellow-100 text-yellow-700")}>Waiting</span>;
  if (status === "Stopped") return <span className={cn(base, "bg-gray-100 text-gray-700")}>Stopped</span>;
  return null;
}

export function ResurrectionCard({
  snapshot,
  onDismiss,
  onResume,
  onSaveNote,
  onForgetTaskContext,
  onForgetProjectContext,
  className,
}: ResurrectionCardProps) {
  const data = useMemo(() => selectCardData(snapshot), [snapshot]);

  const [draftNote, setDraftNote] = useState(snapshot.user_note ?? "");
  const [isSavingNote, setIsSavingNote] = useState(false);
  const [noteError, setNoteError] = useState<string | null>(null);

  useEffect(() => {
    setDraftNote(snapshot.user_note ?? "");
    setIsSavingNote(false);
    setNoteError(null);
  }, [snapshot.id, snapshot.user_note]);

  const handleSaveNote = async () => {
    if (!onSaveNote) return;

    setNoteError(null);
    setIsSavingNote(true);
    try {
      await onSaveNote(draftNote);
    } catch (error) {
      setNoteError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSavingNote(false);
    }
  };

  return (
    <div
      className={cn(
        "fixed right-4 bottom-4 z-40 w-[460px] max-w-[calc(100vw-2rem)]",
        "bg-white border border-gray-200 rounded-xl shadow-xl",
        "max-h-[80vh] overflow-hidden",
        className,
      )}
      role="dialog"
      aria-label="Resurrection card"
    >
      {/* Header */}
      <div className="flex items-start justify-between gap-3 px-4 py-3 border-b border-gray-200">
        <div className="min-w-0">
          <div className="text-xs text-gray-500 flex items-center gap-1">
            <IconClock size={14} />
            <span>Last active: {formatLocalTime(data.capturedAt)}</span>
          </div>
          <div className="mt-1 font-semibold text-gray-900 truncate">{data.taskTitle}</div>
          <div className="mt-1 text-xs text-gray-500 truncate">{data.taskId}</div>
        </div>

        <button
          type="button"
          onClick={onDismiss}
          className="p-2 rounded hover:bg-gray-100 text-gray-600"
          aria-label="Dismiss resurrection card"
          title="Dismiss"
        >
          <IconX size={16} />
        </button>
      </div>

      {/* Body */}
      <div className="px-4 py-3 overflow-y-auto max-h-[calc(80vh-56px)]">
        {/* Terminal */}
        {data.terminal && (
          <div className="mb-3">
            <div className="flex items-center gap-2 mb-2">
              <IconTerminal size={16} className="text-gray-500" />
              <div className="text-sm font-medium text-gray-900">Terminal</div>
              <div className="flex-1" />
              {statusBadge(data.terminal.status)}
            </div>

            <div className="text-xs text-gray-600 space-y-1">
              <div>
                <span className="text-gray-500">Session:</span>{" "}
                <span className="font-mono">#{data.terminal.sessionId}</span>
                {data.terminal.exitCode !== undefined && (
                  <>
                    {" "}
                    <span className="text-gray-500">Exit:</span>{" "}
                    <span className="font-mono">{data.terminal.exitCode}</span>
                  </>
                )}
              </div>

              {data.terminal.lastAttention && (
                <div className="p-2 bg-orange-50 border border-orange-200 text-orange-800 rounded">
                  <div className="font-medium">Attention</div>
                  <div className="font-mono whitespace-pre-wrap">{data.terminal.lastAttention.preview}</div>
                </div>
              )}

              {data.terminal.tailExcerpt ? (
                <div className="mt-2">
                  <div className="text-gray-500 mb-1">Tail (last lines)</div>
                  <pre className="bg-gray-50 border border-gray-200 rounded p-2 font-mono text-xs whitespace-pre-wrap">
                    {data.terminal.tailExcerpt}
                  </pre>
                </div>
              ) : data.terminal.tailPath ? (
                <div className="mt-2 text-xs text-gray-500">
                  Tail stored at: <span className="font-mono">{data.terminal.tailPath}</span>
                </div>
              ) : (
                <div className="mt-2 text-xs text-gray-500">No terminal output captured.</div>
              )}
            </div>
          </div>
        )}

        {/* Note (display) */}
        {data.userNote && (
          <div className="mb-3">
            <div className="text-sm font-medium text-gray-900 mb-1">Note to future self</div>
            <div className="bg-blue-50 border border-blue-200 text-blue-900 rounded p-2 text-sm whitespace-pre-wrap">
              {data.userNote}
            </div>
          </div>
        )}

        {/* Note editor (manual capture) */}
        {onSaveNote && (
          <div className="mb-3">
            <div className="text-sm font-medium text-gray-900 mb-1">{data.userNote ? "Update note" : "Add note"}</div>
            <textarea
              value={draftNote}
              onChange={(e) => setDraftNote(e.currentTarget.value)}
              rows={3}
              className="w-full text-sm border border-gray-300 rounded p-2 focus:outline-none focus:ring-2 focus:ring-blue-200"
              placeholder="Write a note to future you..."
            />
            {noteError && <div className="mt-1 text-xs text-red-600">{noteError}</div>}
            <div className="mt-2 flex justify-end">
              <button
                type="button"
                onClick={handleSaveNote}
                disabled={isSavingNote || draftNote.trim().length === 0}
                className="px-3 py-1.5 bg-blue-500 text-white text-sm rounded hover:bg-blue-600 disabled:opacity-50 transition-colors"
              >
                {isSavingNote ? "Saving..." : "Save note"}
              </button>
            </div>
          </div>
        )}

        {!data.terminal && !data.userNote && !onSaveNote && (
          <div className="text-sm text-gray-600">No additional context captured for this snapshot.</div>
        )}

        {onResume && (
          <div className="mt-3 flex justify-end">
            <button
              type="button"
              onClick={onResume}
              className="px-3 py-1.5 bg-blue-500 text-white text-sm rounded hover:bg-blue-600 transition-colors"
            >
              Resume work
            </button>
          </div>
        )}

        {(onForgetTaskContext || onForgetProjectContext) && (
          <div className="mt-3 pt-3 border-t border-gray-200 flex items-center justify-between gap-3">
            <div className="text-xs text-gray-500">Forget context</div>
            <div className="flex items-center gap-2">
              {onForgetTaskContext && (
                <button type="button" onClick={onForgetTaskContext} className="text-xs text-red-700 hover:underline">
                  Forget task
                </button>
              )}
              {onForgetProjectContext && (
                <button type="button" onClick={onForgetProjectContext} className="text-xs text-red-700 hover:underline">
                  Forget project
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
