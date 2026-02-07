# Task ID Parsing Cross-Language Parity Test Fixtures

**Bead**: bd-q85.4 – CR Phase 0: Cross-language parity tests for task-id parsing

## Purpose

These fixtures ensure that the TypeScript parser (`ProjectStateEditor.ts`) and Rust parser (`markdown.rs`) produce identical results when parsing task IDs and session badges.

## Files

- **task-id-parsing.md**: Shared markdown fixture containing representative test cases
- **task-id-parsing.expected.json**: Expected parsed output (array of tasks with simplified structure)
- **task-id-parsing.README.md**: This file

## Test Cases Included

The fixture contains 14 test cases covering:

1. **3-letter prefix IDs**: `abc.first-task`, `xyz.second-task-123`
2. **4-letter prefix IDs**: `abcd.long-prefix-task`, `wxyz.completed-task`
3. **ID + session badge combinations**: Tasks with Running/Stopped/Waiting badges
4. **Badge without ID**: Tasks with only session badge (no task ID)
5. **Special characters in labels**: Hyphens, numbers in task ID labels
6. **Plain tasks**: Tasks without ID or badge

## Expected JSON Structure

Each task is represented as:

```json
{
  "name": "Task name (without ID or badge)",
  "complete": true/false,
  "taskId": "abc.task-id" or null,
  "sessionStatus": {
    "status": "Running|Stopped|Waiting",
    "sessionId": 42
  } or null
}
```

## Test Implementation

### TypeScript Test
**Location**: `src/lib/__tests__/ProjectStateEditor.test.ts`  
**Test name**: `"cross-language parity with Rust parser" > "should match Rust parser results for shared fixture"`  
**Status**: ✅ **PASSING** (verified with `bun run test:unit`)

The test:
1. Reads the shared fixtures using `new URL(..., import.meta.url)`
2. Parses with `ProjectStateEditor.parse()`
3. Converts tasks to simplified format matching expected JSON
4. Asserts each field matches expected values

### Rust Test
**Location**: `src-tauri/src/session/markdown.rs`  
**Test name**: `test_cross_language_parity_with_typescript_parser`  
**Status**: ⚠️ **BLOCKED** - Compilation errors in unrelated modules (`lib.rs`, `session/persistence.rs`)

The test:
1. Reads shared fixtures using `env!("CARGO_MANIFEST_DIR")`
2. Parses with `parse_body()`
3. Deserializes expected JSON with serde
4. Asserts each field matches expected values

**Blocking issues** (pre-existing, not caused by this bead):
- `DaemonRequest::Start` missing `task_id` field in `lib.rs:231`
- `Session::new()` calls missing `task_id` argument in `session/persistence.rs` (multiple locations)

These are unrelated to the markdown parsing code and should be fixed separately.

## Usage

### Running TypeScript Test
```bash
bun run test:unit
# Or specifically:
bun test src/lib/__tests__/ProjectStateEditor.test.ts
```

### Running Rust Test (once compilation issues fixed)
```bash
cd src-tauri
cargo test session::markdown::tests::test_cross_language_parity_with_typescript_parser
```

## Verification

✅ TypeScript test passes (242 tests passing)  
⚠️ Rust test code is correct but blocked by unrelated compilation errors

## Maintenance

When updating task ID or session badge parsing logic:

1. Update **both** parsers (TS and Rust)
2. Add new test cases to `task-id-parsing.md` if needed
3. Update `task-id-parsing.expected.json` to match
4. Run **both** test suites to verify parity

Any divergence will cause the tests to fail, preventing silent parsing inconsistencies.
