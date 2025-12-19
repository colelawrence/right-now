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
    /// The task name without the session badge
    pub name: String,
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

    // Extract session badge if present
    let (name, session_status) = if let Some(badge_caps) = SESSION_BADGE_RE.captures(full_name) {
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

    Some(ParsedTask {
        prefix,
        complete,
        name,
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
pub fn update_task_session(line: &str, session_status: Option<&TaskSessionStatus>) -> String {
    let Some(task) = parse_task_line(line) else {
        return line.to_string();
    };

    // Build the new line
    let checkbox = match task.complete {
        Some(c) => c.to_string(),
        None => " ".to_string(),
    };

    let badge = match session_status {
        Some(ss) => format_session_badge(ss.status, ss.session_id),
        None => String::new(),
    };

    format!("{}[{}] {}{}", task.prefix, checkbox, task.name, badge)
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

/// Find a task in the markdown content by matching the start of the task name
pub fn find_task_by_key<'a>(blocks: &'a [MarkdownBlock], task_key: &str) -> Option<&'a ParsedTask> {
    let key_lower = task_key.to_lowercase();

    for block in blocks {
        if let MarkdownBlock::Task(task) = block {
            if task.name.to_lowercase().starts_with(&key_lower) {
                return Some(task);
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
/// Note: Uses exact matching on task name (case-insensitive) to avoid
/// accidentally updating tasks that share a common prefix.
pub fn update_task_session_in_content(
    content: &str,
    task_name: &str,
    session_status: Option<&TaskSessionStatus>,
) -> UpdateResult {
    let name_lower = task_name.to_lowercase();
    let lines: Vec<&str> = content.lines().collect();
    let mut task_found = false;

    let updated_lines: Vec<String> = lines
        .into_iter()
        .map(|line| {
            if let Some(task) = parse_task_line(line) {
                // Use exact match (case-insensitive) on task name
                if task.name.to_lowercase() == name_lower {
                    task_found = true;
                    return update_task_session(line, session_status);
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
}
