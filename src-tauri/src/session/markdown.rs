// Markdown parser for TODO files with session badge support
//
// This module mirrors the TypeScript implementation in src/lib/ProjectStateEditor.ts
// to ensure both Rust (daemon) and TypeScript (UI) agree on the badge format.
//
// Session badge format: [Status](todos://session/<id>)
// Example: - [ ] Implement reports [Running](todos://session/42)

use crate::session::protocol::{SessionId, SessionStatus};
use regex::Regex;
use std::sync::LazyLock;

/// Regex for parsing task lines (mirrors TASK_RE in ProjectStateEditor.ts)
/// Captures: (prefix)(checkbox)(name with optional session badge)
static TASK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*[-*]?\s*)\[([xX\s])\]\s+(.*)$").unwrap());

/// Regex for extracting session badge from task name
/// Matches: [Status](todos://session/<id>) at end of line
static SESSION_BADGE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s+\[(Running|Stopped|Waiting)\]\(todos://session/(\d+)\)$").unwrap()
});

/// Regex for extracting task ID token
/// Matches: [abc.derived-label] (3-4 letter prefix + derived label)
static TASK_ID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\s+\[([a-z]{3,4}\.[a-z0-9\-]+)\](?:\s+\[(Running|Stopped|Waiting)\]\(todos://session/\d+\))?$").unwrap()
});

/// Regex for a bare task id key (no brackets), e.g. "abc.derived-label"
static TASK_ID_KEY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z]{3,4}\.[a-z0-9\-]+$").unwrap());

/// Session status parsed from a task line
#[derive(Debug, Clone, PartialEq)]
pub struct TaskSessionStatus {
    pub status: SessionStatus,
    pub session_id: SessionId,
}

/// A parsed task from a markdown line
#[derive(Debug, Clone)]
pub struct ParsedTask {
    /// The prefix (whitespace and bullet)
    pub prefix: String,
    /// The checkbox state: "x", "X", or " " (or empty for unchecked)
    pub complete: Option<char>,
    /// The task name without the session badge or task ID
    pub name: String,
    /// Task ID token if present (e.g., "abc.derived-label")
    pub task_id: Option<String>,
    /// Session status if present
    pub session_status: Option<TaskSessionStatus>,
    /// The original full line (for round-tripping)
    pub original_line: String,
}

/// A parsed heading from a markdown line
#[derive(Debug, Clone)]
pub struct ParsedHeading {
    pub level: usize,
    pub text: String,
}

/// Parsed markdown block types
#[derive(Debug, Clone)]
pub enum MarkdownBlock {
    Task(ParsedTask),
    Heading(ParsedHeading),
    Unrecognized(String),
}

/// Parse a single line as a task
pub fn parse_task_line(line: &str) -> Option<ParsedTask> {
    let caps = TASK_RE.captures(line)?;

    let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
    let checkbox = caps.get(2).map(|m| m.as_str()).unwrap_or(" ");
    let complete = checkbox.chars().next().filter(|c| *c != ' ');
    let full_name = caps.get(3).map(|m| m.as_str()).unwrap_or("");

    // Extract session badge if present (must be at the very end)
    let (name_with_id, session_status) =
        if let Some(badge_caps) = SESSION_BADGE_RE.captures(full_name) {
            let status_str = badge_caps.get(1).map(|m| m.as_str()).unwrap_or("Running");
            let session_id: SessionId = badge_caps
                .get(2)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);

            let status = match status_str {
                "Running" => SessionStatus::Running,
                "Waiting" => SessionStatus::Waiting,
                "Stopped" => SessionStatus::Stopped,
                _ => SessionStatus::Running,
            };

            // Remove badge from name
            let name = SESSION_BADGE_RE.replace(full_name, "").to_string();

            (name, Some(TaskSessionStatus { status, session_id }))
        } else {
            (full_name.to_string(), None)
        };

    // Extract task ID token if present (appears after name, before badge)
    let (name, task_id) = if let Some(id_caps) = TASK_ID_RE.captures(&name_with_id) {
        let task_id_str = id_caps.get(1).map(|m| m.as_str().to_string());
        // Remove task ID from name
        let name = TASK_ID_RE.replace(&name_with_id, "").to_string();
        (name, task_id_str)
    } else {
        (name_with_id, None)
    };

    Some(ParsedTask {
        prefix,
        complete,
        name,
        task_id,
        session_status,
        original_line: line.to_string(),
    })
}

/// Parse a single line as a heading
pub fn parse_heading_line(line: &str) -> Option<ParsedHeading> {
    static HEADING_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.*)$").unwrap());

    let caps = HEADING_RE.captures(line)?;
    let level = caps.get(1).map(|m| m.as_str().len()).unwrap_or(1);
    let text = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string();

    Some(ParsedHeading { level, text })
}

/// Check if a line is a task
pub fn is_task(line: &str) -> bool {
    TASK_RE.is_match(line)
}

/// Check if a line is a heading
pub fn is_heading(line: &str) -> bool {
    line.starts_with('#')
}

/// Format a session badge for insertion into a task line
pub fn format_session_badge(status: SessionStatus, session_id: SessionId) -> String {
    format!(" [{}](todos://session/{})", status, session_id)
}

/// Update a task line with a new session status, or remove the badge if None
/// Preserves task ID tokens in the correct order: name → task_id → badge
pub fn update_task_session(line: &str, session_status: Option<&TaskSessionStatus>) -> String {
    let Some(task) = parse_task_line(line) else {
        return line.to_string();
    };

    // Build the new line
    let checkbox = match task.complete {
        Some(c) => c.to_string(),
        None => " ".to_string(),
    };

    let task_id_token = match &task.task_id {
        Some(id) => format!(" [{}]", id),
        None => String::new(),
    };

    let badge = match session_status {
        Some(ss) => format_session_badge(ss.status, ss.session_id),
        None => String::new(),
    };

    format!(
        "{}[{}] {}{}{}",
        task.prefix, checkbox, task.name, task_id_token, badge
    )
}

/// Parse a complete markdown body into blocks
pub fn parse_body(content: &str) -> Vec<MarkdownBlock> {
    let lines: Vec<&str> = content.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0;
    let mut unrecognized_buffer = Vec::new();

    let flush_unrecognized = |blocks: &mut Vec<MarkdownBlock>, buffer: &mut Vec<String>| {
        if !buffer.is_empty() {
            blocks.push(MarkdownBlock::Unrecognized(buffer.join("\n")));
            buffer.clear();
        }
    };

    while i < lines.len() {
        let line = lines[i];

        // Check for heading
        if let Some(heading) = parse_heading_line(line) {
            flush_unrecognized(&mut blocks, &mut unrecognized_buffer);
            blocks.push(MarkdownBlock::Heading(heading));
            i += 1;
            continue;
        }

        // Check for task
        if let Some(task) = parse_task_line(line) {
            flush_unrecognized(&mut blocks, &mut unrecognized_buffer);
            blocks.push(MarkdownBlock::Task(task));
            i += 1;
            continue;
        }

        // Unrecognized line
        unrecognized_buffer.push(line.to_string());
        i += 1;
    }

    flush_unrecognized(&mut blocks, &mut unrecognized_buffer);
    blocks
}

/// Find a task in the markdown content by task key.
///
/// Matching strategy:
/// 1. If `task_key` looks like a task id (e.g. "abc.derived-label"), match by `task_id`
/// 2. Otherwise prefer exact name match (case-insensitive)
/// 3. Fallback to starts-with match (case-insensitive) for CLI convenience
pub fn find_task_by_key<'a>(blocks: &'a [MarkdownBlock], task_key: &str) -> Option<&'a ParsedTask> {
    if TASK_ID_KEY_RE.is_match(task_key) {
        return find_task_by_id(blocks, task_key);
    }

    let key_lower = task_key.to_lowercase();

    // Prefer exact match on task name
    for block in blocks {
        if let MarkdownBlock::Task(task) = block {
            if task.name.to_lowercase() == key_lower {
                return Some(task);
            }
        }
    }

    // Fallback: starts-with match on task name (legacy behavior)
    for block in blocks {
        if let MarkdownBlock::Task(task) = block {
            if task.name.to_lowercase().starts_with(&key_lower) {
                return Some(task);
            }
        }
    }

    None
}

/// Find a task in the markdown content by task ID
pub fn find_task_by_id<'a>(blocks: &'a [MarkdownBlock], task_id: &str) -> Option<&'a ParsedTask> {
    for block in blocks {
        if let MarkdownBlock::Task(task) = block {
            if let Some(ref id) = task.task_id {
                if id == task_id {
                    return Some(task);
                }
            }
        }
    }

    None
}

/// Result of updating a task's session badge
#[derive(Debug)]
pub struct UpdateResult {
    /// The updated content
    pub content: String,
    /// Whether a task line was actually modified
    pub task_found: bool,
}

/// Update a specific task's session badge in the markdown content
/// Returns the updated content and whether the task was found
///
/// Matching strategy:
/// 1. If task_name contains a dot and matches task ID pattern, prefer task_id match
/// 2. Otherwise, use exact match on task name (case-insensitive)
pub fn update_task_session_in_content(
    content: &str,
    task_name: &str,
    session_status: Option<&TaskSessionStatus>,
) -> UpdateResult {
    let is_task_id = TASK_ID_KEY_RE.is_match(task_name);

    let name_lower = task_name.to_lowercase();
    let lines: Vec<&str> = content.lines().collect();
    let mut task_found = false;

    let updated_lines: Vec<String> = lines
        .into_iter()
        .map(|line| {
            if let Some(task) = parse_task_line(line) {
                // Prefer task_id match if the input looks like a task ID
                if is_task_id {
                    if let Some(ref task_id) = task.task_id {
                        if task_id == task_name {
                            task_found = true;
                            return update_task_session(line, session_status);
                        }
                    }
                } else {
                    // Fall back to exact name match (case-insensitive)
                    if task.name.to_lowercase() == name_lower {
                        task_found = true;
                        return update_task_session(line, session_status);
                    }
                }
            }
            line.to_string()
        })
        .collect();

    UpdateResult {
        content: updated_lines.join("\n"),
        task_found,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_task_without_badge() {
        let line = "- [ ] Implement reports";
        let task = parse_task_line(line).unwrap();

        assert_eq!(task.prefix, "- ");
        assert_eq!(task.complete, None);
        assert_eq!(task.name, "Implement reports");
        assert!(task.session_status.is_none());
    }

    #[test]
    fn test_parse_task_with_badge() {
        let line = "- [ ] Implement reports [Running](todos://session/42)";
        let task = parse_task_line(line).unwrap();

        assert_eq!(task.prefix, "- ");
        assert_eq!(task.complete, None);
        assert_eq!(task.name, "Implement reports");
        assert!(task.session_status.is_some());

        let ss = task.session_status.unwrap();
        assert_eq!(ss.status, SessionStatus::Running);
        assert_eq!(ss.session_id, 42);
    }

    #[test]
    fn test_parse_task_with_stopped_badge() {
        let line = "- [x] Done task [Stopped](todos://session/123)";
        let task = parse_task_line(line).unwrap();

        assert_eq!(task.complete, Some('x'));
        assert_eq!(task.name, "Done task");

        let ss = task.session_status.unwrap();
        assert_eq!(ss.status, SessionStatus::Stopped);
        assert_eq!(ss.session_id, 123);
    }

    #[test]
    fn test_parse_heading() {
        let line = "## My Heading";
        let heading = parse_heading_line(line).unwrap();

        assert_eq!(heading.level, 2);
        assert_eq!(heading.text, "My Heading");
    }

    #[test]
    fn test_format_session_badge() {
        let badge = format_session_badge(SessionStatus::Running, 42);
        assert_eq!(badge, " [Running](todos://session/42)");
    }

    #[test]
    fn test_update_task_session_add_badge() {
        let line = "- [ ] Implement reports";
        let ss = TaskSessionStatus {
            status: SessionStatus::Running,
            session_id: 42,
        };

        let updated = update_task_session(line, Some(&ss));
        assert_eq!(
            updated,
            "- [ ] Implement reports [Running](todos://session/42)"
        );
    }

    #[test]
    fn test_update_task_session_change_status() {
        let line = "- [ ] Implement reports [Running](todos://session/42)";
        let ss = TaskSessionStatus {
            status: SessionStatus::Stopped,
            session_id: 42,
        };

        let updated = update_task_session(line, Some(&ss));
        assert_eq!(
            updated,
            "- [ ] Implement reports [Stopped](todos://session/42)"
        );
    }

    #[test]
    fn test_update_task_session_remove_badge() {
        let line = "- [ ] Implement reports [Running](todos://session/42)";
        let updated = update_task_session(line, None);
        assert_eq!(updated, "- [ ] Implement reports");
    }

    #[test]
    fn test_parse_body() {
        let content = r#"# Main Heading

- [ ] First Task
- [ ] Second Task [Running](todos://session/1)

## Sub Heading

Some unrecognized text"#;

        let blocks = parse_body(content);

        // 7 blocks: heading, blank line, task, task, blank line, heading, unrecognized text
        assert_eq!(blocks.len(), 7);

        // Check heading
        if let MarkdownBlock::Heading(h) = &blocks[0] {
            assert_eq!(h.level, 1);
            assert_eq!(h.text, "Main Heading");
        } else {
            panic!("Expected heading");
        }

        // Check first task
        if let MarkdownBlock::Task(t) = &blocks[2] {
            assert_eq!(t.name, "First Task");
            assert!(t.session_status.is_none());
        } else {
            panic!("Expected task");
        }

        // Check second task with badge
        if let MarkdownBlock::Task(t) = &blocks[3] {
            assert_eq!(t.name, "Second Task");
            assert!(t.session_status.is_some());
        } else {
            panic!("Expected task");
        }
    }

    #[test]
    fn test_find_task_by_key() {
        let content = r#"# Tasks
- [ ] Implement reports
- [ ] Build pipeline
"#;
        let blocks = parse_body(content);

        let task = find_task_by_key(&blocks, "impl").unwrap();
        assert_eq!(task.name, "Implement reports");

        let task = find_task_by_key(&blocks, "BUILD").unwrap();
        assert_eq!(task.name, "Build pipeline");

        assert!(find_task_by_key(&blocks, "nonexistent").is_none());
    }

    #[test]
    fn test_find_task_by_key_matches_task_id() {
        let content = r#"# Tasks
- [ ] Implement reports [abc.implement-reports]
- [ ] Build pipeline [xyz.build-pipeline]
"#;
        let blocks = parse_body(content);

        let task = find_task_by_key(&blocks, "xyz.build-pipeline").unwrap();
        assert_eq!(task.name, "Build pipeline");
        assert_eq!(task.task_id.as_deref(), Some("xyz.build-pipeline"));
    }

    #[test]
    fn test_update_task_session_in_content() {
        let content = r#"# Tasks
- [ ] Implement reports
- [ ] Build pipeline
"#;

        let ss = TaskSessionStatus {
            status: SessionStatus::Running,
            session_id: 42,
        };

        // Now requires exact match (case-insensitive)
        let result = update_task_session_in_content(content, "Implement reports", Some(&ss));

        assert!(result.task_found, "Task should be found");
        assert!(result
            .content
            .contains("Implement reports [Running](todos://session/42)"));
        assert!(result.content.contains("- [ ] Build pipeline"));
    }

    #[test]
    fn test_update_task_session_exact_match_only() {
        let content = r#"# Tasks
- [ ] Build feature
- [ ] Build feature - backend
- [ ] Build pipeline
"#;

        let ss = TaskSessionStatus {
            status: SessionStatus::Running,
            session_id: 1,
        };

        // Should only update exact match, not prefix matches
        let result = update_task_session_in_content(content, "Build feature", Some(&ss));

        assert!(result.task_found, "Task should be found");
        assert!(
            result
                .content
                .contains("- [ ] Build feature [Running](todos://session/1)"),
            "Should update exact match. Got: {}",
            result.content
        );
        assert!(
            result.content.contains("- [ ] Build feature - backend\n"),
            "Should NOT update prefix match. Got: {}",
            result.content
        );
        assert!(
            result.content.contains("- [ ] Build pipeline"),
            "Should NOT update unrelated task. Got: {}",
            result.content
        );
    }

    #[test]
    fn test_update_task_session_task_not_found() {
        let content = r#"# Tasks
- [ ] Build feature
"#;

        let ss = TaskSessionStatus {
            status: SessionStatus::Running,
            session_id: 1,
        };

        // Task name doesn't exist
        let result = update_task_session_in_content(content, "Nonexistent task", Some(&ss));

        assert!(!result.task_found, "Task should NOT be found");
        // Content should be unchanged
        assert_eq!(result.content.trim(), content.trim());
    }

    // ============================================================================
    // Robustness tests for various task line formats
    // ============================================================================

    #[test]
    fn test_asterisk_bullet() {
        let line = "* [ ] Task with asterisk";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.name, "Task with asterisk");
        assert_eq!(task.prefix, "* ");
    }

    #[test]
    fn test_indented_tasks() {
        let line = "  - [ ] Indented task";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.name, "Indented task");
        assert_eq!(task.prefix, "  - ");
    }

    #[test]
    fn test_tab_indented_tasks() {
        let line = "\t- [ ] Tab indented task";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.name, "Tab indented task");
        assert_eq!(task.prefix, "\t- ");
    }

    #[test]
    fn test_bare_checkbox() {
        let line = "[ ] Bare checkbox task";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.name, "Bare checkbox task");
        assert_eq!(task.prefix, "");
    }

    #[test]
    fn test_uppercase_x_completed() {
        let line = "- [X] Completed with uppercase X";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.complete, Some('X'));
    }

    #[test]
    fn test_preserve_prefix_on_update() {
        let line = "  * [ ] Indented asterisk task";
        let ss = TaskSessionStatus {
            status: SessionStatus::Running,
            session_id: 1,
        };
        let updated = update_task_session(line, Some(&ss));
        assert_eq!(
            updated,
            "  * [ ] Indented asterisk task [Running](todos://session/1)"
        );
    }

    // ============================================================================
    // Special characters tests
    // ============================================================================

    #[test]
    fn test_emoji_in_task_name() {
        let line = "- [ ] Fix bug 🐛 in parser";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.name, "Fix bug 🐛 in parser");
    }

    #[test]
    fn test_emoji_with_session_badge() {
        let line = "- [ ] Deploy 🚀 [Running](todos://session/42)";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.name, "Deploy 🚀");
        assert_eq!(task.session_status.as_ref().unwrap().session_id, 42);
    }

    #[test]
    fn test_unicode_characters() {
        let line = "- [ ] Café résumé naïve";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.name, "Café résumé naïve");
    }

    #[test]
    fn test_cjk_characters() {
        let line = "- [ ] 日本語タスク";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.name, "日本語タスク");
    }

    #[test]
    fn test_inline_code_in_task() {
        let line = "- [ ] Fix `console.log` statement";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.name, "Fix `console.log` statement");
    }

    #[test]
    fn test_brackets_not_confused_with_badge() {
        let line = "- [ ] Fix array[0] access";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.name, "Fix array[0] access");
        assert!(task.session_status.is_none());
    }

    #[test]
    fn test_parentheses_not_confused_with_link() {
        let line = "- [ ] Implement function(arg1, arg2)";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.name, "Implement function(arg1, arg2)");
        assert!(task.session_status.is_none());
    }

    // ============================================================================
    // Multiple links tests
    // ============================================================================

    #[test]
    fn test_link_before_session_badge() {
        let line =
            "- [ ] See [docs](https://example.com) for details [Running](todos://session/42)";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.name, "See [docs](https://example.com) for details");
        assert_eq!(task.session_status.as_ref().unwrap().session_id, 42);
    }

    #[test]
    fn test_multiple_links_before_badge() {
        let line = "- [ ] Check [link1](http://a.com) and [link2](http://b.com) [Running](todos://session/1)";
        let task = parse_task_line(line).unwrap();
        assert_eq!(
            task.name,
            "Check [link1](http://a.com) and [link2](http://b.com)"
        );
        assert_eq!(task.session_status.as_ref().unwrap().session_id, 1);
    }

    // ============================================================================
    // Session badge edge cases
    // ============================================================================

    #[test]
    fn test_badge_in_middle_not_matched() {
        let line = "- [ ] Status is [Running](not-a-link) and continue";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.name, "Status is [Running](not-a-link) and continue");
        assert!(task.session_status.is_none());
    }

    #[test]
    fn test_no_space_before_badge_not_matched() {
        let line = "- [ ] Task name[Running](todos://session/42)";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.name, "Task name[Running](todos://session/42)");
        assert!(task.session_status.is_none());
    }

    #[test]
    fn test_wrong_protocol_not_matched() {
        let line = "- [ ] Task [Running](http://session/42)";
        let task = parse_task_line(line).unwrap();
        assert!(task.session_status.is_none());
    }

    #[test]
    fn test_wrong_path_not_matched() {
        let line = "- [ ] Task [Running](todos://sessions/42)";
        let task = parse_task_line(line).unwrap();
        assert!(task.session_status.is_none());
    }

    #[test]
    fn test_non_numeric_id_not_matched() {
        let line = "- [ ] Task [Running](todos://session/abc)";
        let task = parse_task_line(line).unwrap();
        assert!(task.session_status.is_none());
    }

    #[test]
    fn test_invalid_status_not_matched() {
        let line = "- [ ] Task [Paused](todos://session/42)";
        let task = parse_task_line(line).unwrap();
        assert!(task.session_status.is_none());
    }

    #[test]
    fn test_large_session_id() {
        let line = "- [ ] Task [Running](todos://session/9999999999999)";
        let task = parse_task_line(line).unwrap();
        assert_eq!(
            task.session_status.as_ref().unwrap().session_id,
            9999999999999
        );
    }

    #[test]
    fn test_session_id_zero() {
        let line = "- [ ] Task [Running](todos://session/0)";
        let task = parse_task_line(line).unwrap();
        assert_eq!(task.session_status.as_ref().unwrap().session_id, 0);
    }

    // ============================================================================
    // Task ID token tests
    // ============================================================================

    #[test]
    fn test_parse_task_with_task_id_only() {
        let line = "- [ ] Implement reports [abc.derived-label]";
        let task = parse_task_line(line).unwrap();

        assert_eq!(task.prefix, "- ");
        assert_eq!(task.complete, None);
        assert_eq!(task.name, "Implement reports");
        assert_eq!(task.task_id, Some("abc.derived-label".to_string()));
        assert!(task.session_status.is_none());
    }

    #[test]
    fn test_parse_task_with_task_id_and_badge() {
        let line = "- [ ] Implement reports [abc.derived-label] [Running](todos://session/42)";
        let task = parse_task_line(line).unwrap();

        assert_eq!(task.name, "Implement reports");
        assert_eq!(task.task_id, Some("abc.derived-label".to_string()));
        assert!(task.session_status.is_some());

        let ss = task.session_status.unwrap();
        assert_eq!(ss.status, SessionStatus::Running);
        assert_eq!(ss.session_id, 42);
    }

    #[test]
    fn test_parse_task_id_with_four_letter_prefix() {
        let line = "- [ ] Task [abcd.some-label]";
        let task = parse_task_line(line).unwrap();

        assert_eq!(task.name, "Task");
        assert_eq!(task.task_id, Some("abcd.some-label".to_string()));
    }

    #[test]
    fn test_parse_task_id_with_three_letter_prefix() {
        let line = "- [ ] Task [xyz.label]";
        let task = parse_task_line(line).unwrap();

        assert_eq!(task.name, "Task");
        assert_eq!(task.task_id, Some("xyz.label".to_string()));
    }

    #[test]
    fn test_parse_task_id_with_numbers_in_label() {
        let line = "- [ ] Task [abc.label-123]";
        let task = parse_task_line(line).unwrap();

        assert_eq!(task.name, "Task");
        assert_eq!(task.task_id, Some("abc.label-123".to_string()));
    }

    #[test]
    fn test_update_task_preserves_task_id() {
        let line = "- [ ] Implement reports [abc.derived-label]";
        let ss = TaskSessionStatus {
            status: SessionStatus::Running,
            session_id: 42,
        };

        let updated = update_task_session(line, Some(&ss));
        assert_eq!(
            updated,
            "- [ ] Implement reports [abc.derived-label] [Running](todos://session/42)"
        );
    }

    #[test]
    fn test_update_task_preserves_task_id_when_changing_badge() {
        let line = "- [ ] Task [abc.label] [Running](todos://session/1)";
        let ss = TaskSessionStatus {
            status: SessionStatus::Stopped,
            session_id: 1,
        };

        let updated = update_task_session(line, Some(&ss));
        assert_eq!(
            updated,
            "- [ ] Task [abc.label] [Stopped](todos://session/1)"
        );
    }

    #[test]
    fn test_update_task_preserves_task_id_when_removing_badge() {
        let line = "- [ ] Task [abc.label] [Running](todos://session/1)";
        let updated = update_task_session(line, None);
        assert_eq!(updated, "- [ ] Task [abc.label]");
    }

    #[test]
    fn test_update_task_adds_badge_preserves_task_id() {
        let line = "- [ ] Task [abc.label]";
        let ss = TaskSessionStatus {
            status: SessionStatus::Waiting,
            session_id: 99,
        };

        let updated = update_task_session(line, Some(&ss));
        assert_eq!(
            updated,
            "- [ ] Task [abc.label] [Waiting](todos://session/99)"
        );
    }

    #[test]
    fn test_find_task_by_id() {
        let content = r#"# Tasks
- [ ] First task [abc.first-label]
- [ ] Second task [xyz.second-label]
- [ ] Third task
"#;
        let blocks = parse_body(content);

        let task = find_task_by_id(&blocks, "abc.first-label").unwrap();
        assert_eq!(task.name, "First task");

        let task = find_task_by_id(&blocks, "xyz.second-label").unwrap();
        assert_eq!(task.name, "Second task");

        assert!(find_task_by_id(&blocks, "nonexistent.label").is_none());
    }

    #[test]
    fn test_update_task_by_id_in_content() {
        let content = r#"# Tasks
- [ ] First task [abc.first-label]
- [ ] Second task [xyz.second-label]
- [ ] Third task
"#;

        let ss = TaskSessionStatus {
            status: SessionStatus::Running,
            session_id: 42,
        };

        // Update using task ID
        let result = update_task_session_in_content(content, "abc.first-label", Some(&ss));

        assert!(result.task_found, "Task should be found by ID");
        assert!(
            result
                .content
                .contains("First task [abc.first-label] [Running](todos://session/42)"),
            "Should update task with ID. Got: {}",
            result.content
        );
        assert!(
            result
                .content
                .contains("- [ ] Second task [xyz.second-label]"),
            "Should NOT update other tasks. Got: {}",
            result.content
        );
        assert!(
            result.content.contains("- [ ] Third task"),
            "Should NOT update task without ID. Got: {}",
            result.content
        );
    }

    #[test]
    fn test_update_task_by_name_when_no_id() {
        let content = r#"# Tasks
- [ ] First task
- [ ] Second task
"#;

        let ss = TaskSessionStatus {
            status: SessionStatus::Running,
            session_id: 1,
        };

        // Update using exact task name
        let result = update_task_session_in_content(content, "First task", Some(&ss));

        assert!(result.task_found, "Task should be found by name");
        assert!(
            result
                .content
                .contains("First task [Running](todos://session/1)"),
            "Should update task by name. Got: {}",
            result.content
        );
        assert!(
            result.content.contains("- [ ] Second task"),
            "Should NOT update other task. Got: {}",
            result.content
        );
    }

    #[test]
    fn test_task_id_not_confused_with_regular_brackets() {
        let line = "- [ ] Fix array[index] access";
        let task = parse_task_line(line).unwrap();

        assert_eq!(task.name, "Fix array[index] access");
        assert!(task.task_id.is_none());
    }

    #[test]
    fn test_task_id_requires_dot_separator() {
        let line = "- [ ] Task [abcdefgh]";
        let task = parse_task_line(line).unwrap();

        // Without dot, should not be recognized as task ID
        assert_eq!(task.name, "Task [abcdefgh]");
        assert!(task.task_id.is_none());
    }

    #[test]
    fn test_task_id_prefix_length_constraint() {
        // Too short (2 letters)
        let line = "- [ ] Task [ab.label]";
        let task = parse_task_line(line).unwrap();
        assert!(task.task_id.is_none());

        // Too long (5 letters)
        let line = "- [ ] Task [abcde.label]";
        let task = parse_task_line(line).unwrap();
        assert!(task.task_id.is_none());
    }

    #[test]
    fn test_multiple_tasks_same_name_different_ids() {
        let content = r#"# Tasks
- [ ] Build feature [abc.variant-a]
- [ ] Build feature [xyz.variant-b]
"#;

        let ss = TaskSessionStatus {
            status: SessionStatus::Running,
            session_id: 1,
        };

        // Update by task ID should only affect the specific task
        let result = update_task_session_in_content(content, "abc.variant-a", Some(&ss));

        assert!(result.task_found);
        assert!(
            result
                .content
                .contains("Build feature [abc.variant-a] [Running](todos://session/1)"),
            "Should update first task. Got: {}",
            result.content
        );
        assert!(
            result
                .content
                .contains("- [ ] Build feature [xyz.variant-b]"),
            "Should NOT update second task with different ID. Got: {}",
            result.content
        );
    }

    #[test]
    fn test_task_id_with_completed_task() {
        let line = "- [x] Completed task [abc.done-label] [Stopped](todos://session/42)";
        let task = parse_task_line(line).unwrap();

        assert_eq!(task.complete, Some('x'));
        assert_eq!(task.name, "Completed task");
        assert_eq!(task.task_id, Some("abc.done-label".to_string()));
        assert_eq!(
            task.session_status.as_ref().unwrap().status,
            SessionStatus::Stopped
        );
    }

    #[test]
    fn test_task_id_case_sensitivity() {
        // Task IDs should be lowercase only
        let line = "- [ ] Task [ABC.Label]";
        let task = parse_task_line(line).unwrap();

        // Uppercase should not match task ID pattern
        assert!(task.task_id.is_none());
        assert_eq!(task.name, "Task [ABC.Label]");
    }

    #[test]
    fn test_ordering_task_id_between_name_and_badge() {
        let line = "- [ ] Task name [abc.label] [Running](todos://session/1)";
        let task = parse_task_line(line).unwrap();

        assert_eq!(task.name, "Task name");
        assert_eq!(task.task_id, Some("abc.label".to_string()));
        assert!(task.session_status.is_some());

        // Round-trip should preserve ordering
        let ss = TaskSessionStatus {
            status: SessionStatus::Waiting,
            session_id: 1,
        };
        let updated = update_task_session(line, Some(&ss));
        assert_eq!(
            updated,
            "- [ ] Task name [abc.label] [Waiting](todos://session/1)"
        );
    }

    // Cross-language parity test (bd-q85.4)
    #[test]
    fn test_cross_language_parity_with_typescript_parser() {
        use serde::Deserialize;

        #[derive(Debug, Deserialize)]
        struct ExpectedTask {
            name: String,
            complete: bool,
            #[serde(rename = "taskId")]
            task_id: Option<String>,
            #[serde(rename = "sessionStatus")]
            session_status: Option<ExpectedSessionStatus>,
        }

        #[derive(Debug, Deserialize)]
        struct ExpectedSessionStatus {
            status: String,
            #[serde(rename = "sessionId")]
            session_id: u64,
        }

        // Read the shared fixture
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let fixture_path = format!("{}/../test/fixtures/task-id-parsing.md", manifest_dir);
        let expected_path = format!(
            "{}/../test/fixtures/task-id-parsing.expected.json",
            manifest_dir
        );

        let fixture_content =
            std::fs::read_to_string(&fixture_path).expect("Failed to read fixture file");
        let expected_json =
            std::fs::read_to_string(&expected_path).expect("Failed to read expected JSON file");

        let expected_tasks: Vec<ExpectedTask> =
            serde_json::from_str(&expected_json).expect("Failed to parse expected JSON");

        // Parse with Rust parser
        let blocks = parse_body(&fixture_content);
        let tasks: Vec<&ParsedTask> = blocks
            .iter()
            .filter_map(|b| {
                if let MarkdownBlock::Task(task) = b {
                    Some(task)
                } else {
                    None
                }
            })
            .collect();

        // Compare with expected results
        assert_eq!(
            tasks.len(),
            expected_tasks.len(),
            "Number of tasks should match"
        );

        for (i, (actual, expected)) in tasks.iter().zip(expected_tasks.iter()).enumerate() {
            assert_eq!(actual.name, expected.name, "Task {} name should match", i);

            let actual_complete = actual.complete.is_some();
            assert_eq!(
                actual_complete, expected.complete,
                "Task {} complete status should match",
                i
            );

            assert_eq!(
                actual.task_id, expected.task_id,
                "Task {} task_id should match",
                i
            );

            match (&actual.session_status, &expected.session_status) {
                (None, None) => {}
                (Some(actual_ss), Some(expected_ss)) => {
                    let actual_status = format!("{}", actual_ss.status);
                    assert_eq!(
                        actual_status, expected_ss.status,
                        "Task {} session status should match",
                        i
                    );
                    assert_eq!(
                        actual_ss.session_id, expected_ss.session_id,
                        "Task {} session_id should match",
                        i
                    );
                }
                _ => panic!(
                    "Task {} session_status presence mismatch: actual={:?}, expected={:?}",
                    i, actual.session_status, expected.session_status
                ),
            }
        }
    }
}
