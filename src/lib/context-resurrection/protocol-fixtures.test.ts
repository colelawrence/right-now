import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import type {
  CaptureReason,
  ContextSnapshotV1,
  CrAttentionType,
  CrDaemonRequest,
  CrDaemonResponse,
  CrSessionStatus,
  DaemonErrorCode,
} from "./types";

/**
 * Protocol fixture tests - validate JSON files match TypeScript types
 *
 * These fixtures are shared between Rust and TypeScript to ensure protocol compatibility.
 */

const FIXTURE_DIR = join(import.meta.dir, "../../../test/fixtures/protocol");

function readFixture(name: string): unknown {
  const path = join(FIXTURE_DIR, name);
  const content = readFileSync(path, "utf-8");
  return JSON.parse(content);
}

describe("Protocol fixtures - CR requests", () => {
  test("cr_latest.json deserializes correctly", () => {
    const req = readFixture("cr_latest.json") as CrDaemonRequest;
    expect(req.type).toBe("cr_latest");
    if (req.type === "cr_latest") {
      expect(req.project_path).toBe("/Users/test/projects/app/TODO.md");
      expect(req.task_id).toBe("abc.test-task");
    }
  });

  test("cr_list.json deserializes correctly", () => {
    const req = readFixture("cr_list.json") as CrDaemonRequest;
    expect(req.type).toBe("cr_list");
    if (req.type === "cr_list") {
      expect(req.project_path).toBe("/Users/test/projects/app/TODO.md");
      expect(req.task_id).toBe("abc.test-task");
      expect(req.limit).toBe(50);
    }
  });

  test("cr_get.json deserializes correctly", () => {
    const req = readFixture("cr_get.json") as CrDaemonRequest;
    expect(req.type).toBe("cr_get");
    if (req.type === "cr_get") {
      expect(req.project_path).toBe("/Users/test/projects/app/TODO.md");
      expect(req.task_id).toBe("abc.test-task");
      expect(req.snapshot_id).toBe("2026-02-07T10:30:00Z_abc.test-task");
    }
  });

  test("cr_capture_now.json deserializes correctly", () => {
    const req = readFixture("cr_capture_now.json") as CrDaemonRequest;
    expect(req.type).toBe("cr_capture_now");
    if (req.type === "cr_capture_now") {
      expect(req.project_path).toBe("/Users/test/projects/app/TODO.md");
      expect(req.task_id).toBe("abc.test-task");
      expect(req.user_note).toBe("Manual snapshot before refactoring");
    }
  });

  test("cr_delete_task.json deserializes correctly", () => {
    const req = readFixture("cr_delete_task.json") as CrDaemonRequest;
    expect(req.type).toBe("cr_delete_task");
    if (req.type === "cr_delete_task") {
      expect(req.project_path).toBe("/Users/test/projects/app/TODO.md");
      expect(req.task_id).toBe("abc.test-task");
    }
  });

  test("cr_delete_project.json deserializes correctly", () => {
    const req = readFixture("cr_delete_project.json") as CrDaemonRequest;
    expect(req.type).toBe("cr_delete_project");
    if (req.type === "cr_delete_project") {
      expect(req.project_path).toBe("/Users/test/projects/app/TODO.md");
    }
  });
});

describe("Protocol fixtures - CR responses", () => {
  test("cr_snapshot.json deserializes correctly", () => {
    const resp = readFixture("cr_snapshot.json") as CrDaemonResponse;
    expect(resp.type).toBe("cr_snapshot");
    if (resp.type === "cr_snapshot") {
      expect(resp.snapshot).not.toBeNull();
      const snapshot = resp.snapshot as ContextSnapshotV1;

      expect(snapshot.id).toBe("2026-02-07T10:30:00Z_abc.test-task");
      expect(snapshot.version).toBe(1);
      expect(snapshot.project_path).toBe("/Users/test/projects/app/TODO.md");
      expect(snapshot.task_id).toBe("abc.test-task");
      expect(snapshot.task_title_at_capture).toBe("Implement feature X");
      expect(snapshot.captured_at).toBe("2026-02-07T10:30:00Z");
      expect(snapshot.capture_reason).toBe("session_stopped");

      expect(snapshot.terminal).toBeDefined();
      if (snapshot.terminal) {
        expect(snapshot.terminal.session_id).toBe(42);
        expect(snapshot.terminal.status).toBe("Stopped");
        expect(snapshot.terminal.exit_code).toBe(0);
        expect(snapshot.terminal.tail_inline).toBe("Build completed successfully\n");
      }
    }
  });

  test("cr_snapshot_null.json deserializes correctly", () => {
    const resp = readFixture("cr_snapshot_null.json") as CrDaemonResponse;
    expect(resp.type).toBe("cr_snapshot");
    if (resp.type === "cr_snapshot") {
      expect(resp.snapshot).toBeNull();
    }
  });

  test("cr_snapshots.json deserializes correctly", () => {
    const resp = readFixture("cr_snapshots.json") as CrDaemonResponse;
    expect(resp.type).toBe("cr_snapshots");
    if (resp.type === "cr_snapshots") {
      expect(resp.snapshots).toHaveLength(2);

      // First snapshot (manual)
      const snap1 = resp.snapshots[0];
      expect(snap1.id).toBe("2026-02-07T11:00:00Z_abc.test-task");
      expect(snap1.capture_reason).toBe("manual");
      expect(snap1.user_note).toBe("Before refactoring");
      expect(snap1.terminal).toBeUndefined();

      // Second snapshot (stopped)
      const snap2 = resp.snapshots[1];
      expect(snap2.id).toBe("2026-02-07T10:30:00Z_abc.test-task");
      expect(snap2.capture_reason).toBe("session_stopped");
      expect(snap2.terminal).toBeDefined();
    }
  });

  test("cr_deleted.json deserializes correctly", () => {
    const resp = readFixture("cr_deleted.json") as CrDaemonResponse;
    expect(resp.type).toBe("cr_deleted");
    if (resp.type === "cr_deleted") {
      expect(resp.deleted_count).toBe(5);
    }
  });

  test("error.json deserializes correctly", () => {
    const resp = readFixture("error.json") as CrDaemonResponse;
    expect(resp.type).toBe("error");
    if (resp.type === "error") {
      expect(resp.code).toBe("not_found");
      expect(resp.message).toBe("Snapshot not found");
    }
  });
});

describe("Enum value validation", () => {
  test("all error codes are valid", () => {
    const validCodes: DaemonErrorCode[] = [
      "not_found",
      "skipped",
      "invalid_request",
      "store_unavailable",
      "internal",
      "daemon_unavailable",
      "timeout",
      "version_mismatch",
    ];

    // Just verify the type constraint accepts all values
    validCodes.forEach((code) => {
      expect(typeof code).toBe("string");
    });
  });

  test("error_version_mismatch.json deserializes correctly", () => {
    const resp = readFixture("error_version_mismatch.json") as CrDaemonResponse;
    expect(resp.type).toBe("error");
    if (resp.type === "error") {
      expect(resp.code).toBe("version_mismatch");
      expect(resp.message).toBe("Daemon is newer than app—please update the app.");
    }
  });

  test("all capture reasons are valid", () => {
    const validReasons: CaptureReason[] = [
      "session_stopped",
      "session_waiting",
      "session_running",
      "idle_timeout",
      "manual",
    ];

    validReasons.forEach((reason) => {
      expect(typeof reason).toBe("string");
    });
  });

  test("all session statuses are valid", () => {
    const validStatuses: CrSessionStatus[] = ["Running", "Waiting", "Stopped"];

    validStatuses.forEach((status) => {
      expect(typeof status).toBe("string");
    });
  });

  test("all attention types are valid", () => {
    const validTypes: CrAttentionType[] = ["input_required", "decision_point", "completed", "error"];

    validTypes.forEach((type) => {
      expect(typeof type).toBe("string");
    });
  });
});

describe("Shape validation", () => {
  test("ContextSnapshotV1 has required fields", () => {
    const resp = readFixture("cr_snapshot.json") as CrDaemonResponse;
    if (resp.type === "cr_snapshot" && resp.snapshot) {
      const snapshot = resp.snapshot;

      // Required fields
      expect(snapshot).toHaveProperty("id");
      expect(snapshot).toHaveProperty("version");
      expect(snapshot).toHaveProperty("project_path");
      expect(snapshot).toHaveProperty("task_id");
      expect(snapshot).toHaveProperty("task_title_at_capture");
      expect(snapshot).toHaveProperty("captured_at");
      expect(snapshot).toHaveProperty("capture_reason");

      // Optional fields may or may not be present
      // (terminal, user_note, editor)
    }
  });

  test("TerminalContext has correct shape", () => {
    const resp = readFixture("cr_snapshot.json") as CrDaemonResponse;
    if (resp.type === "cr_snapshot" && resp.snapshot?.terminal) {
      const terminal = resp.snapshot.terminal;

      expect(terminal).toHaveProperty("session_id");
      expect(terminal).toHaveProperty("status");
      expect(typeof terminal.session_id).toBe("number");
      expect(typeof terminal.status).toBe("string");
    }
  });

  test("Request tags match variant names", () => {
    const requests = [
      { file: "cr_latest.json", expectedType: "cr_latest" },
      { file: "cr_list.json", expectedType: "cr_list" },
      { file: "cr_get.json", expectedType: "cr_get" },
      { file: "cr_capture_now.json", expectedType: "cr_capture_now" },
      { file: "cr_delete_task.json", expectedType: "cr_delete_task" },
      { file: "cr_delete_project.json", expectedType: "cr_delete_project" },
    ];

    requests.forEach(({ file, expectedType }) => {
      const req = readFixture(file) as CrDaemonRequest;
      expect(req.type).toBe(expectedType);
    });
  });

  test("Response tags match variant names", () => {
    const responses = [
      { file: "cr_snapshot.json", expectedType: "cr_snapshot" },
      { file: "cr_snapshot_null.json", expectedType: "cr_snapshot" },
      { file: "cr_snapshots.json", expectedType: "cr_snapshots" },
      { file: "cr_deleted.json", expectedType: "cr_deleted" },
      { file: "error.json", expectedType: "error" },
    ];

    responses.forEach(({ file, expectedType }) => {
      const resp = readFixture(file) as CrDaemonResponse;
      expect(resp.type).toBe(expectedType);
    });
  });
});
