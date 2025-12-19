# Shell Integration Testing Matrix & Future Work

## Current State (2024-12)

### What's Implemented
- `todo start` enters session immediately (with `--background` flag for old behavior)
- Environment variables set in PTY: `RIGHT_NOW_SESSION_ID`, `RIGHT_NOW_TASK_KEY`, `RIGHT_NOW_PROJECT`
- `todo shell-integration` command for installing prompt integration
- Shell snippets for zsh, bash, fish (prompt + terminal title via OSC)

### What's Tested

| Test | Type | What It Validates |
|------|------|-------------------|
| `test_pty_environment_variables` | Integration | Env vars appear in PTY child process output |
| `test_bash_integration_e2e` | E2E | Bash can source snippet, function outputs correct format |
| `test_bash_integration_no_env_vars` | E2E | Function outputs nothing when env vars unset |
| `test_install_*` / `test_uninstall` | Unit | File manipulation, idempotency |
| `test_shell_type_*` | Unit | Shell detection from $SHELL path |
| `test_todo_start_shows_prompt_in_pty` | Integration | Env vars passed through daemon → PTY |
| `test_todo_start_background_does_not_attach` | Integration | `--background` flag returns immediately |
| `test_shell_integration_prompt_e2e_bash` | **Full E2E** | daemon → PTY → bash sources snippet → prompt function output ✅ |
| `test_shell_integration_prompt_e2e_zsh` | **Full E2E** | daemon → PTY → zsh sources snippet → prompt function output ✅ |

---

## Testing Gaps

### Shell Matrix

| Shell | Snippet Syntax | Prompt Function | PS1 Integration | Terminal Title |
|-------|---------------|-----------------|-----------------|----------------|
| bash  | ✅ E2E test   | ✅ E2E test     | ❌ Not tested   | ❌ Not tested  |
| zsh   | ✅ E2E test   | ✅ E2E test     | ❌ Not tested   | ❌ Not tested  |
| fish  | ❌ Needs fish | ❌ Needs fish   | ❌ Not tested   | ❌ Not tested  |

### Integration Gaps

| Scenario | Status | Notes |
|----------|--------|-------|
| Spawn `todo start`, verify prompt in PTY | ✅ Done | `test_shell_integration_prompt_e2e` |
| OSC terminal title codes emitted | 🔲 TODO | Need to capture raw PTY output |
| User's existing prompt preserved | 🔲 TODO | Test with pre-existing PS1/PROMPT |
| Interactive shell session end-to-end | 🔲 TODO | Full flow: start → work → detach |
| Multiple shells on same machine | 🔲 TODO | User might have bash + zsh |

### Platform Matrix

| Platform | Status | Notes |
|----------|--------|-------|
| macOS (arm64) | ✅ Dev machine | Primary development |
| macOS (x86_64) | 🔲 CI only | |
| Linux (Ubuntu) | 🔲 CI only | Different shell defaults |
| Windows | ❌ Not supported | Shell integration not implemented |

---

## Future Work

### ~~Priority 1: Core Integration Test~~ ✅ DONE

Implemented in `src-tauri/tests/shell_prompt_integration.rs`:
- `test_shell_integration_prompt_e2e` - Full E2E test that installs shell integration, starts session, sources snippet in PTY, verifies prompt output

### Priority 2: Shell Matrix CI

Add CI jobs that test each shell:

```yaml
# .github/workflows/shell-integration.yml
jobs:
  test-shells:
    strategy:
      matrix:
        shell: [bash, zsh]
        os: [ubuntu-latest, macos-latest]
    steps:
      - run: cargo test --lib shell_integration
      - run: cargo test --test shell_e2e  # Future test crate
```

### Priority 3: Terminal Title Verification (OSC codes)

Test that OSC escape codes are emitted:

```rust
#[test]
fn test_terminal_title_osc_emitted() {
    // Capture raw PTY output (before terminal processing)
    // Look for: \x1b]0;#42: Task name\x07
}
```

### Priority 4: User Config Compatibility (PS1 preservation)

Test that our integration doesn't break existing configs:

```rust
#[test]
fn test_preserves_existing_prompt() {
    // 1. Write rc file with custom PS1="$ "
    // 2. Install integration
    // 3. Source file, check prompt is "[#42: task] $ " not just "[#42: task]"
}
```

---

## Test Infrastructure Needed

### For Shell Matrix
- CI runners with zsh, fish installed
- Test fixtures for each shell's rc file format
- Skip logic for shells not available on system

### For PTY Integration Tests
- Test harness that can spawn `todo` binary
- PTY output capture with timeout
- Ability to send input to PTY (for "exit" command)
- Clean process teardown

### For Terminal Title Tests
- Raw PTY output capture (no terminal emulation)
- Regex matching for OSC sequences
- Different terminal emulator behaviors (optional)

---

## Known Limitations

1. **Ctrl-\ detach** - Cannot be automated without signal injection; manual test only
2. **True interactive mode** - Tests use `-c` flag, not actual interactive shells
3. **User environment variance** - Can't test every possible user shell config
4. **Terminal emulator differences** - OSC codes may render differently

---

## References

- Shell integration implementation: `src-tauri/src/session/shell_integration.rs`
- PTY runtime: `src-tauri/src/session/runtime.rs`
- CLI: `src-tauri/src/bin/todo.rs`
- **Integration tests: `src-tauri/tests/shell_prompt_integration.rs`**
- Testing philosophy: `TESTING.md`
