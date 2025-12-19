# Plan: PTY Shell Integration Testing Hardening

> **Interesting artifacts and learnings must be written back to this document.**

## Executive Summary

Our current E2E tests verify the happy path: "if everything lines up perfectly, does the prompt function output the right format?" This is necessary but not sufficient.

The hardest bugs in PTY/shell integration come from:
- **Escape sequence interactions** - ANSI codes breaking pattern matching
- **Shell startup sequence differences** - login vs non-login, rc file sourcing order
- **Timing and chunking** - output split across reads
- **User environment variance** - existing PS1, shell plugins, version differences

This plan addresses these gaps incrementally, prioritizing issues most likely to cause production failures.

---

## Current State

### What We Test Today
| Test | What It Validates |
|------|-------------------|
| `test_shell_integration_prompt_e2e_bash` | daemon → PTY → bash sources snippet → prompt function output |
| `test_shell_integration_prompt_e2e_zsh` | daemon → PTY → zsh sources snippet → prompt function output |
| `test_todo_start_shows_prompt_in_pty` | Env vars passed through daemon → PTY |
| `test_todo_start_background_does_not_attach` | `--background` flag returns immediately |
| `test_bash_integration_e2e` | Bash can source snippet, function outputs correct format |
| Unit tests in `attention.rs` | Attention pattern detection on static strings |

### Critical Gaps Identified
1. Tests use `--cmd` one-shot mode, not interactive shell startup
2. No verification that PS1/PROMPT integration works automatically
3. No testing of OSC terminal title escape codes
4. Attention detection not tested with ANSI escape sequences
5. Fixed `sleep()` calls create flaky tests
6. No adversarial input testing (shell metacharacters)
7. Process cleanup can leak on assertion failures
8. No chunked output testing for attention detection

---

## Phase 1: Foundation Hardening

**Objective**: Fix fundamental reliability issues that cause flaky tests and leaked processes.

**Scope**: Test infrastructure improvements, no new test scenarios.

**Dependencies**: None

### Tasks

#### 1.1 Replace Fixed Sleeps with Polling
- [x] Create helper function `wait_for_file_content(path, predicate, timeout) -> Result<String>`
- [x] Implement exponential backoff with configurable max timeout
- [x] Replace all `thread::sleep()` calls in integration tests with polling
- [x] Add explicit timeout error messages that include what was being waited for

**Acceptance Criteria**:
- No `thread::sleep()` calls remain in test assertion paths
- Tests fail with descriptive timeout messages, not silent hangs
- Polling uses exponential backoff starting at 50ms, max 10 retries

#### 1.2 RAII Process Cleanup
- [x] Create `DaemonGuard` struct that kills process on Drop
- [x] Ensure daemon cleanup happens even when assertions panic
- [x] Add test that verifies no orphan processes after test suite completion
- [x] Log daemon PID in test output for debugging

**Acceptance Criteria**:
- `DaemonGuard` implements `Drop` trait with process termination
- All tests use `DaemonGuard` instead of manual cleanup
- Running tests 10x in succession leaves no orphan `right-now-daemon` processes

#### 1.3 Shell Version Logging
- [x] Log shell version at start of each shell-specific test
- [x] Add version to test output for CI debugging
- [ ] Document known version-specific behaviors

**Acceptance Criteria**:
- Test output includes shell version (e.g., `bash 3.2.57`, `zsh 5.9`)
- Version logged before test assertions, visible in CI logs

### Verification

**Test Scenarios**:
1. Run integration tests with simulated slow I/O (verify polling works)
2. Force assertion failure mid-test, verify daemon is killed
3. Run test suite 10x, verify no process leaks

**Coverage Requirements**:
- All existing integration tests must still pass
- New helper functions must have unit tests

**Pass/Fail Criteria**:
- Zero `thread::sleep()` in assertion paths
- Zero orphan processes after any test run (pass or fail)
- All tests pass on both fast and slow machines

---

## Phase 2: Escape Sequence Resilience

**Objective**: Ensure attention detection and pattern matching work with real terminal output containing ANSI codes.

**Scope**: Attention detection hardening, escape sequence stripping.

**Dependencies**: Phase 1 (for reliable test infrastructure)

### Tasks

#### 2.1 ANSI Escape Sequence Stripping
- [x] Add function `strip_ansi_codes(text: &str) -> String` to attention module
- [x] Apply stripping before attention pattern matching
- [x] Handle common sequences: colors, cursor movement, clearing
- [x] Preserve semantic content (the actual text)

**Acceptance Criteria**:
- `\x1b[32m✔\x1b[0m Submit` becomes `✔ Submit` after stripping
- Cursor movement codes (`\x1b[H`, `\x1b[2J`) are removed
- Function handles malformed/partial escape sequences gracefully

#### 2.2 Attention Detection with ANSI Codes
- [x] Add tests with colored output from common tools (cargo, npm, pytest)
- [x] Test with real Claude Code output samples (contains escape codes)
- [x] Verify triggers still match when wrapped in color codes

**Acceptance Criteria**:
- `✔ Submit` trigger matches regardless of surrounding color codes
- `Build ready` matches with bold/color formatting
- No false positives from escape sequence fragments

#### 2.3 OSC Terminal Title Verification
- [x] Capture raw PTY output before terminal processing
- [x] Add regex matching for `\x1b]0;#<id>: <task>\x07` pattern
- [x] Test with various task name lengths and characters
- [x] Verify title codes are emitted on precmd/prompt

**Acceptance Criteria**:
- Raw PTY output contains OSC 0 sequence with correct format
- Task names with spaces, unicode work correctly
- Title updates verified for both bash and zsh

### Verification

**Test Scenarios**:
1. Attention detection with cargo test output (contains colors)
2. Attention detection with `\x1b[32m✔\x1b[0m Submit` (exact sequence)
3. OSC title code presence in PTY output stream
4. Task name with unicode emoji in terminal title

**Coverage Requirements**:
- New `strip_ansi_codes` function: 100% branch coverage
- Attention detection tests with escape codes: minimum 5 patterns

**Pass/Fail Criteria**:
- Colored output triggers match same as plain text
- OSC codes present and correctly formatted in raw PTY output
- No regressions in existing attention detection

---

## Phase 3: Interactive Shell Startup

**Objective**: Verify shell integration works in actual interactive shell startup, not just explicit sourcing.

**Scope**: Testing real shell initialization paths.

**Dependencies**: Phase 1

### Tasks

#### 3.1 Login vs Non-Login Shell Testing
- [ ] Test interactive login shell startup (`bash -l`, `zsh -l`)
- [ ] Test interactive non-login shell startup (`bash -i`, `zsh -i`)
- [ ] Verify rc file sourcing in each mode
- [ ] Document which mode the PTY actually uses

**Acceptance Criteria**:
- Understand and document: Does PTY spawn login or non-login shell?
- Integration snippet is sourced in the actual PTY spawn mode
- Tests verify the real startup path, not just explicit sourcing

#### 3.2 Automatic Prompt Function Invocation
- [ ] Test that prompt function is called on each command line (not just once)
- [ ] Issue multiple commands in session, verify prompt appears each time
- [ ] Test that function is in PS1/PROMPT, not just defined

**Acceptance Criteria**:
- Multiple command prompts in session all show `[#id: task]` prefix
- Prompt updates if env vars change mid-session
- Works without explicit `_right_now_prompt` call

#### 3.3 Existing Prompt Preservation
- [ ] Install integration over existing custom PS1/PROMPT
- [ ] Verify original prompt content is preserved
- [ ] Test with complex existing prompts (git status, virtualenv, etc.)

**Acceptance Criteria**:
- Pre-existing `PS1="$ "` becomes `[#0: task] $ ` after integration
- Complex prompts with command substitution still work
- Order is: our prefix + original prompt

### Verification

**Test Scenarios**:
1. PTY spawn with default configuration, verify snippet is auto-sourced
2. Send 3 commands to interactive session, verify 3 prompts with prefix
3. Pre-set `PS1="custom> "`, install integration, verify `[#0: task] custom> `
4. Pre-set complex `PS1` with `$(git branch)`, verify both parts work

**Coverage Requirements**:
- Both bash and zsh interactive modes tested
- Minimum 3 sequential prompts verified in multi-command test

**Pass/Fail Criteria**:
- Integration works without explicit sourcing in PTY
- Multi-command sessions show consistent prompt prefix
- No user prompt content is lost

---

## Phase 4: Chunked Output Handling

**Objective**: Ensure attention detection works when triggers are split across PTY read boundaries.

**Scope**: Streaming/chunked input handling for pattern matching.

**Dependencies**: Phase 2 (escape sequence handling)

### Tasks

#### 4.1 Chunked Attention Detection Analysis
- [x] Document current behavior: does detection work across chunks?
- [x] Analyze ring buffer implementation for cross-chunk matching
- [x] Determine if triggers must be in single chunk (document limitation)

**Acceptance Criteria**:
- Clear documentation of chunking behavior
- Decision: fix cross-chunk matching OR document single-chunk requirement

#### 4.2 Cross-Chunk Pattern Matching (if fixing)
- [x] Implement sliding window or lookback for pattern matching
- [x] Handle trigger split at arbitrary byte boundaries
- [x] Ensure no duplicate detections at chunk boundaries

**Acceptance Criteria**:
- `✔ Sub` + `mit` across two chunks still triggers
- No duplicate triggers when pattern spans boundary
- Performance acceptable (no O(n²) scanning)

#### 4.3 Chunked Output Test Infrastructure
- [x] Create helper to simulate chunked PTY output
- [x] Test triggers split at every possible byte position
- [x] Test with realistic chunk sizes (4KB, 16KB)

**Acceptance Criteria**:
- Test infrastructure can inject arbitrary chunk boundaries
- At least one test per trigger pattern with mid-trigger split

### Verification

**Test Scenarios**:
1. Trigger "Build ready" split as "Build rea" + "dy\n"
2. Trigger "✔ Submit" split in middle of UTF-8 sequence (edge case)
3. Trigger at exact chunk boundary (last byte of chunk 1, first of chunk 2)
4. Rapid successive triggers in single chunk

**Coverage Requirements**:
- All attention triggers tested with at least one split scenario
- UTF-8 boundary handling tested

**Pass/Fail Criteria**:
- Cross-chunk triggers detected (or limitation clearly documented)
- No panics on malformed UTF-8 at chunk boundaries
- No duplicate detections

---

## Phase 5: Adversarial Input Hardening

**Objective**: Ensure shell integration is secure against injection and handles edge-case inputs.

**Scope**: Security and robustness of task name handling.

**Dependencies**: Phase 1

### Tasks

#### 5.1 Shell Metacharacter Testing
- [x] Test task names with: `$(whoami)`, `` `id` ``, `'; rm -rf /; echo '`
- [x] Verify command substitution is NOT executed in prompt
- [x] Verify quoting prevents injection

**Acceptance Criteria**:
- Task name `$(whoami)` displays literally, not as command output
- No shell errors from metacharacters in task names
- Prompt output is exactly `[#0: $(whoami)]` for that task name

#### 5.2 Special Character Handling
- [x] Test task names with: newlines, tabs, quotes (single/double), backslashes
- [x] Test unicode: emoji, RTL text, zero-width characters
- [x] Verify terminal title handles special characters

**Acceptance Criteria**:
- Newlines in task name don't break prompt (escaped or stripped)
- Quotes don't terminate the prompt string early
- Unicode displays correctly (or is safely escaped)

#### 5.3 Length Edge Cases
- [x] Test empty task name
- [x] Test very long task name (1000+ characters)
- [x] Verify prompt doesn't wrap or break terminal

**Acceptance Criteria**:
- Empty task name produces valid (possibly empty-looking) prompt
- Long task names truncated or handled gracefully
- No terminal corruption from length edge cases

> Update: `RIGHT_NOW_TASK_DISPLAY` now carries a sanitized/truncated version of the task key for shell integrations, ensuring adversarial names never break prompts or terminal titles while preserving the raw key for automation (`RIGHT_NOW_TASK_KEY`).

### Verification

**Test Scenarios**:
1. Task name: `$(echo INJECTED)` - verify "INJECTED" NOT in prompt
2. Task name: `'; cat /etc/passwd; echo '` - verify no file disclosure
3. Task name: `Hello\nWorld` - verify single-line prompt
4. Task name: 500 character string - verify no crash/hang

**Coverage Requirements**:
- Minimum 10 adversarial task names tested
- Both prompt function and terminal title tested for each

**Pass/Fail Criteria**:
- Zero command execution from task names
- No shell syntax errors
- All adversarial inputs produce valid (if ugly) output

---

## Phase 6: Expect-Style Interactive Testing

**Objective**: Add proper interactive session testing using expect-style patterns.

**Scope**: New test infrastructure for interactive PTY sessions.

**Dependencies**: Phase 3 (interactive shell understanding)

### Tasks

#### 6.1 Expect-Style Test Library Integration
- [ ] Evaluate Rust options: `rexpect`, `expectrl`, custom
- [ ] Add chosen library to dev-dependencies
- [ ] Create wrapper for common patterns (send command, expect output)

**Acceptance Criteria**:
- Library supports: send input, expect pattern with timeout, handle Ctrl-sequences
- Wrapper provides ergonomic API for tests
- Works with our PTY spawn mechanism

#### 6.2 Interactive Session Tests
- [ ] Test: spawn session, wait for prompt, send command, verify output
- [ ] Test: send Ctrl-C, verify interrupt handling
- [ ] Test: send Ctrl-\ (detach), verify clean detachment
- [ ] Test: multiple commands in sequence

**Acceptance Criteria**:
- Interactive tests complete in <5 seconds each
- Ctrl-C test shows interrupt behavior
- Ctrl-\ test shows detach (if implemented) or expected behavior

#### 6.3 Prompt Appearance Timing
- [ ] Measure time from session start to first prompt
- [ ] Verify prompt appears before user could start typing
- [ ] Test with slow shell startup (many plugins)

**Acceptance Criteria**:
- First prompt appears within 2 seconds of session start
- Prompt is visible before any user input is possible

### Verification

**Test Scenarios**:
1. Spawn session, expect `[#0: task]` in prompt within 2s
2. Send `echo hello`, expect `hello` in output
3. Send Ctrl-C during `sleep 100`, expect prompt returns
4. Send 5 commands in sequence, verify all complete

**Coverage Requirements**:
- Minimum 5 interactive session tests
- At least one Ctrl-sequence test

**Pass/Fail Criteria**:
- Interactive tests are not flaky (pass 10/10 runs)
- Session responds to input correctly
- Timing requirements met

---

## Phase 7: Property-Based and Fuzz Testing

**Objective**: Find edge cases through generated inputs.

**Scope**: Property-based testing for escape codes and task names.

**Dependencies**: Phase 2, Phase 5

### Tasks

#### 7.1 Property-Based Testing Setup
- [ ] Add `proptest` or `quickcheck` to dev-dependencies
- [ ] Create generators for: task names, escape sequences, output streams

**Acceptance Criteria**:
- Property tests run as part of normal test suite
- Generators produce diverse, realistic inputs

#### 7.2 Escape Sequence Properties
- [ ] Property: stripping then matching equals matching stripped input
- [ ] Property: any input to strip_ansi produces valid UTF-8
- [ ] Property: attention detection never panics on arbitrary input

**Acceptance Criteria**:
- Properties hold for 1000+ generated inputs
- Failing cases are shrunk to minimal reproduction

#### 7.3 Task Name Properties
- [ ] Property: any task name produces valid shell output (no syntax error)
- [ ] Property: prompt output contains task name (possibly escaped)
- [ ] Property: terminal title is valid OSC sequence

**Acceptance Criteria**:
- Properties tested with unicode, control chars, shell metacharacters
- No panics or shell errors from any generated input

### Verification

**Test Scenarios**:
1. 1000 random task names through prompt function
2. 1000 random byte sequences through ANSI stripper
3. 1000 random escape-laden strings through attention detection

**Coverage Requirements**:
- Property tests achieve same line coverage as manual tests
- Generators documented with examples

**Pass/Fail Criteria**:
- Zero panics from generated inputs
- All properties hold after 1000 iterations
- Any found bugs are added as regression tests

---

## Phase 8: CI/CD Integration

**Objective**: Run comprehensive shell tests in CI across platforms.

**Scope**: CI workflow configuration.

**Dependencies**: All previous phases

### Tasks

#### 8.1 Shell Matrix CI Workflow
- [ ] Create `.github/workflows/shell-integration.yml`
- [ ] Matrix: bash × zsh × (macOS, Ubuntu)
- [ ] Install fish on CI runners for fish tests
- [ ] Run both unit and integration tests

**Acceptance Criteria**:
- CI runs shell tests on every PR
- Matrix covers: bash+macOS, bash+Ubuntu, zsh+macOS, zsh+Ubuntu
- Fish tests run where fish is available

#### 8.2 Flakiness Detection
- [ ] Run integration tests 5x in CI to detect flakiness
- [ ] Track test timing for performance regressions
- [ ] Alert on tests that pass <100% of runs

**Acceptance Criteria**:
- Flaky tests are identified and fixed before merge
- Test timing tracked over time
- No test takes >30 seconds individually

#### 8.3 Shell Version Matrix
- [ ] Test on bash 3.2 (macOS default) and bash 5.x
- [ ] Test on zsh 5.8 and 5.9
- [ ] Document any version-specific behaviors discovered

**Acceptance Criteria**:
- Tests pass on oldest supported shell versions
- Version-specific behaviors documented in this plan

### Verification

**Test Scenarios**:
1. CI run with bash 3.2 on macOS
2. CI run with bash 5.x on Ubuntu
3. 5x test repetition on each platform

**Coverage Requirements**:
- All integration tests run in CI
- Minimum 2 platforms tested

**Pass/Fail Criteria**:
- 100% pass rate on all platforms
- No flaky tests (5/5 passes required)
- CI completes in <10 minutes

---

## Appendix A: Test File Organization

```
src-tauri/
├── src/
│   └── session/
│       ├── attention.rs          # Unit tests for attention detection
│       ├── shell_integration.rs  # Unit tests for snippet generation
│       └── runtime.rs            # Unit tests for PTY runtime
└── tests/
    ├── shell_prompt_integration.rs    # Current E2E tests
    ├── shell_interactive.rs           # Phase 6: expect-style tests
    ├── shell_adversarial.rs           # Phase 5: injection tests
    ├── attention_escape_codes.rs      # Phase 2: ANSI handling
    └── helpers/
        ├── mod.rs
        ├── polling.rs                 # Phase 1: wait_for_file_content
        ├── daemon_guard.rs            # Phase 1: RAII cleanup
        └── expect.rs                  # Phase 6: expect wrapper
```

## Appendix B: Naming Conventions

- **Unit tests**: `test_<function>_<scenario>`
- **Integration tests**: `test_<feature>_<shell>_<scenario>`
- **Property tests**: `prop_<invariant>`
- **Adversarial tests**: `test_adversarial_<attack_type>`

Examples:
- `test_strip_ansi_removes_color_codes`
- `test_prompt_zsh_preserves_existing_ps1`
- `prop_any_task_name_produces_valid_shell`
- `test_adversarial_command_substitution`

## Appendix C: Learnings Log

> Record interesting discoveries, bugs found, and design decisions here.

### Template Entry
```
### [Date] - [Brief Title]
**Context**: What were you working on?
**Discovery**: What did you find?
**Impact**: How does this affect the codebase?
**Action**: What was done about it?
```

---

### 2025-12-19 - Phase 1 & 2 Hardening Landed
**Context**: Implemented polling + DaemonGuard helpers for the PTY suite and added ANSI stripping/OSC verification in the attention detector and integration tests.
**Discovery**: The Codex macOS sandbox refuses to bind Unix sockets (EPERM) even under `src-tauri/target/test-artifacts`, so `cargo test --test shell_prompt_integration` cannot succeed in this environment.
**Impact**: PTY integration tests must run on developer machines or CI runners with socket permissions; only the new unit tests execute within Codex.
**Action**: Added `helpers::{polling, daemon_guard}`, replaced direct sleeps with `wait_for_file_content`, logged shell versions + OSC regex assertions, and documented the sandbox limitation here for future runs.

### 2025-12-19 - Chunked Attention & Adversarial Prompt Hardening
**Context**: Delivered Phase 4 (chunked attention detection) and Phase 5 (adversarial task names) improvements.
**Discovery**: Regex matching operated per PTY chunk, so triggers split across reads never fired; raw task names also allowed control characters/newlines into prompts and OSC titles.
**Impact**: Without a sliding window plus sanitization, Claude-style prompts could be missed and malicious task names could corrupt shells.
**Action**: Added `AttentionAccumulator` streaming detection with multi-match tests (including UTF-8 splits), introduced `RIGHT_NOW_TASK_DISPLAY` with sanitized/truncated content, updated all shell snippets to prefer it, and added unit tests covering metacharacters, whitespace collapse, and length limits.

*Document created: 2024-12*
*Last updated: 2025-12-19*
