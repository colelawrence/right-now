import { IconRefresh, IconX } from "@tabler/icons-react";
import { useEffect, useState } from "react";
import { type Session, SessionClient, SessionStatus } from "../lib/SessionClient";

interface SessionsPanelProps {
  onClose: () => void;
}

export function SessionsPanel({ onClose }: SessionsPanelProps) {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [actingSessionId, setActingSessionId] = useState<number | null>(null);

  const sessionClient = new SessionClient();

  const loadSessions = async () => {
    setLoading(true);
    setError(null);
    setActionError(null);
    try {
      const result = await sessionClient.listSessions();
      // Sort: Running/Waiting first, then Stopped
      const sorted = [...result].sort((a, b) => {
        const statusOrder = { Running: 0, Waiting: 1, Stopped: 2 };
        const aOrder = statusOrder[a.status] ?? 3;
        const bOrder = statusOrder[b.status] ?? 3;
        if (aOrder !== bOrder) return aOrder - bOrder;
        // Secondary sort by ID (most recent first)
        return b.id - a.id;
      });
      setSessions(sorted);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleStopSession = async (sessionId: number) => {
    setActionError(null);
    setActingSessionId(sessionId);
    try {
      await sessionClient.stopSession(sessionId);
      await loadSessions();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : String(err));
    } finally {
      setActingSessionId(null);
    }
  };

  const handleContinueSession = async (sessionId: number) => {
    setActionError(null);
    setActingSessionId(sessionId);
    try {
      const result = await sessionClient.continueSession(sessionId, 512);
      const tail = SessionClient.tailBytesToString(result.tail);
      if (tail) {
        console.log(`Session ${sessionId} tail:\n${tail}`);
        alert(`Session output (last 512 bytes):\n\n${tail}`);
      } else {
        alert(`Session ${sessionId} continued (no recent output)`);
      }
    } catch (err) {
      setActionError(err instanceof Error ? err.message : String(err));
    } finally {
      setActingSessionId(null);
    }
  };

  useEffect(() => {
    loadSessions();
  }, []);

  const getStatusBadge = (status: SessionStatus) => {
    const baseClasses = "text-xs px-2 py-0.5 rounded-full font-medium";
    switch (status) {
      case SessionStatus.Running:
        return `${baseClasses} bg-green-100 text-green-700`;
      case SessionStatus.Waiting:
        return `${baseClasses} bg-yellow-100 text-yellow-700`;
      case SessionStatus.Stopped:
        return `${baseClasses} bg-gray-100 text-gray-700`;
      default:
        return `${baseClasses} bg-gray-100 text-gray-700`;
    }
  };

  const truncatePath = (path: string, maxLen = 50) => {
    if (path.length <= maxLen) return path;
    const parts = path.split("/");
    if (parts.length <= 2) return `...${path.slice(-maxLen)}`;
    return `.../${parts.slice(-2).join("/")}`;
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-30">
      <div className="bg-white rounded-lg shadow-xl w-full max-w-3xl max-h-[80vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200">
          <h2 className="text-xl font-semibold">Sessions</h2>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={loadSessions}
              disabled={loading}
              className="p-2 rounded hover:bg-gray-100 text-gray-600 disabled:opacity-50"
              title="Refresh"
            >
              <IconRefresh size={18} className={loading ? "animate-spin" : ""} />
            </button>
            <button
              type="button"
              onClick={onClose}
              className="p-2 rounded hover:bg-gray-100 text-gray-600"
              title="Close"
            >
              <IconX size={18} />
            </button>
          </div>
        </div>

        {/* Error alerts */}
        {error && (
          <div className="mx-6 mt-4 p-3 bg-red-50 border border-red-200 text-red-700 rounded text-sm">
            <strong>Error loading sessions:</strong> {error}
          </div>
        )}
        {actionError && (
          <div className="mx-6 mt-4 p-3 bg-red-50 border border-red-200 text-red-700 rounded text-sm">
            <strong>Action failed:</strong> {actionError}
          </div>
        )}

        {/* Sessions list */}
        <div className="flex-1 overflow-y-auto px-6 py-4">
          {loading && sessions.length === 0 ? (
            <div className="text-center text-gray-500 py-8">Loading sessions...</div>
          ) : sessions.length === 0 ? (
            <div className="text-center text-gray-500 py-8">No sessions found</div>
          ) : (
            <div className="space-y-3">
              {sessions.map((session) => (
                <div
                  key={session.id}
                  className="border border-gray-200 rounded-lg p-4 hover:bg-gray-50 transition-colors"
                >
                  <div className="flex items-start justify-between gap-4">
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-2">
                        <span className="font-mono text-sm text-gray-500">#{session.id}</span>
                        <span className="font-semibold truncate">{session.task_key}</span>
                        <span className={getStatusBadge(session.status)}>{session.status}</span>
                      </div>
                      <div className="text-sm text-gray-600 space-y-1">
                        <div className="flex items-center gap-2">
                          <span className="text-gray-500">Project:</span>
                          <span className="font-mono text-xs truncate" title={session.project_path}>
                            {truncatePath(session.project_path)}
                          </span>
                        </div>
                        <div className="flex items-center gap-2">
                          <span className="text-gray-500">Created:</span>
                          <span className="text-xs">{new Date(session.created_at).toLocaleString()}</span>
                        </div>
                        {session.exit_code !== undefined && (
                          <div className="flex items-center gap-2">
                            <span className="text-gray-500">Exit code:</span>
                            <span className="text-xs font-mono">{session.exit_code}</span>
                          </div>
                        )}
                        {session.last_attention && (
                          <div className="text-xs text-orange-600 mt-1">⚠️ {session.last_attention.preview}</div>
                        )}
                      </div>
                    </div>

                    {/* Actions */}
                    <div className="flex gap-2 flex-shrink-0">
                      <button
                        type="button"
                        onClick={() => handleContinueSession(session.id)}
                        disabled={actingSessionId === session.id}
                        className="px-3 py-1.5 bg-blue-500 text-white text-sm rounded hover:bg-blue-600 disabled:opacity-50 transition-colors"
                      >
                        Continue
                      </button>
                      {session.status !== SessionStatus.Stopped && (
                        <button
                          type="button"
                          onClick={() => handleStopSession(session.id)}
                          disabled={actingSessionId === session.id}
                          className="px-3 py-1.5 bg-red-500 text-white text-sm rounded hover:bg-red-600 disabled:opacity-50 transition-colors"
                        >
                          Stop
                        </button>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="px-6 py-3 border-t border-gray-200 bg-gray-50 rounded-b-lg">
          <div className="text-sm text-gray-600">
            {sessions.length === 0 ? "No sessions" : `${sessions.length} session${sessions.length === 1 ? "" : "s"}`}
          </div>
        </div>
      </div>
    </div>
  );
}
