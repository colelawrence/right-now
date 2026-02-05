// SessionsDebugPanel - Dev-only debug UI for testing SessionClient
// Provides a simple interface to list, start, and stop sessions

import { useEffect, useState } from "react";
import { type Session, SessionClient, SessionStatus, sessionClient } from "../lib/SessionClient";

export function SessionsDebugPanel() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Form state for starting a new session
  const [taskKey, setTaskKey] = useState("");
  const [projectPath, setProjectPath] = useState("");

  const loadSessions = async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await sessionClient.listSessions();
      setSessions(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleStartSession = async () => {
    if (!taskKey.trim() || !projectPath.trim()) {
      setError("Task key and project path are required");
      return;
    }

    setLoading(true);
    setError(null);
    try {
      await sessionClient.startSession(taskKey.trim(), projectPath.trim());
      setTaskKey("");
      await loadSessions();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleStopSession = async (sessionId: number) => {
    setLoading(true);
    setError(null);
    try {
      await sessionClient.stopSession(sessionId);
      await loadSessions();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  const handleContinueSession = async (sessionId: number) => {
    setLoading(true);
    setError(null);
    try {
      const result = await sessionClient.continueSession(sessionId, 512);
      const tail = result.tail ? SessionClient.tailBytesToString(result.tail) : "(no output)";
      console.log(`Session ${sessionId} tail:`, tail);
      alert(`Session ${sessionId} output (last 512 bytes):\n\n${tail}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadSessions();
  }, []);

  const getStatusColor = (status: SessionStatus) => {
    switch (status) {
      case SessionStatus.Running:
        return "text-green-600";
      case SessionStatus.Waiting:
        return "text-yellow-600";
      case SessionStatus.Stopped:
        return "text-gray-600";
      default:
        return "text-gray-600";
    }
  };

  return (
    <div className="p-4 border border-gray-300 rounded bg-white shadow-lg max-w-4xl mx-auto my-4">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-bold">Sessions Debug Panel</h2>
        <button
          type="button"
          onClick={loadSessions}
          disabled={loading}
          className="px-3 py-1 bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50"
        >
          {loading ? "Loading..." : "Refresh"}
        </button>
      </div>

      {error && (
        <div className="mb-4 p-3 bg-red-100 border border-red-400 text-red-700 rounded">
          <strong>Error:</strong> {error}
        </div>
      )}

      {/* Start session form */}
      <div className="mb-4 p-3 bg-gray-100 rounded">
        <h3 className="font-semibold mb-2">Start New Session</h3>
        <div className="flex gap-2 mb-2">
          <input
            type="text"
            placeholder="Task key (e.g., 'Implement')"
            value={taskKey}
            onChange={(e) => setTaskKey(e.target.value)}
            className="flex-1 px-2 py-1 border border-gray-300 rounded"
          />
          <input
            type="text"
            placeholder="Project path (e.g., /path/to/TODO.md)"
            value={projectPath}
            onChange={(e) => setProjectPath(e.target.value)}
            className="flex-1 px-2 py-1 border border-gray-300 rounded"
          />
          <button
            type="button"
            onClick={handleStartSession}
            disabled={loading}
            className="px-3 py-1 bg-green-500 text-white rounded hover:bg-green-600 disabled:opacity-50"
          >
            Start
          </button>
        </div>
        <p className="text-xs text-gray-600">
          Note: Task key must match a task in the TODO file (fuzzy matched). Project path must exist.
        </p>
      </div>

      {/* Sessions list */}
      <div>
        <h3 className="font-semibold mb-2">Active Sessions ({sessions.length})</h3>
        {sessions.length === 0 ? (
          <p className="text-gray-600 italic">No sessions found</p>
        ) : (
          <div className="space-y-2">
            {sessions.map((session) => (
              <div
                key={session.id}
                className="p-3 border border-gray-200 rounded bg-gray-50 flex items-start justify-between"
              >
                <div className="flex-1">
                  <div className="flex items-center gap-2 mb-1">
                    <span className="font-mono text-sm text-gray-500">#{session.id}</span>
                    <span className="font-semibold">{session.task_key}</span>
                    <span className={`text-sm font-medium ${getStatusColor(session.status)}`}>[{session.status}]</span>
                  </div>
                  <p className="text-sm text-gray-600 mb-1">
                    Project: <span className="font-mono text-xs">{session.project_path}</span>
                  </p>
                  <p className="text-xs text-gray-500">Created: {new Date(session.created_at).toLocaleString()}</p>
                  {session.exit_code !== undefined && (
                    <p className="text-xs text-gray-500">Exit code: {session.exit_code}</p>
                  )}
                  {session.last_attention && (
                    <p className="text-xs text-orange-600 mt-1">⚠️ Attention: {session.last_attention.preview}</p>
                  )}
                </div>
                <div className="flex gap-2">
                  {session.status !== SessionStatus.Stopped && (
                    <button
                      type="button"
                      onClick={() => handleStopSession(session.id)}
                      disabled={loading}
                      className="px-2 py-1 bg-red-500 text-white text-sm rounded hover:bg-red-600 disabled:opacity-50"
                    >
                      Stop
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={() => handleContinueSession(session.id)}
                    disabled={loading}
                    className="px-2 py-1 bg-blue-500 text-white text-sm rounded hover:bg-blue-600 disabled:opacity-50"
                  >
                    View Output
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
