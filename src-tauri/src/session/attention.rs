//! Attention detection for PTY session output.
//!
//! Scans PTY output for patterns that indicate user attention is needed
//! (prompts, errors, completion messages) and extracts preview context.
//!
//! See `docs/attention-notifications.md` for architecture overview.

use crate::session::protocol::AttentionType;
use once_cell::sync::Lazy;
use regex::Regex;
use std::{borrow::Cow, ops::Range};

/// Default attention profiles compiled lazily
static DEFAULT_PROFILES: Lazy<Vec<AttentionProfile>> = Lazy::new(|| {
    vec![
        AttentionProfile {
            name: "claude-code",
            triggers: vec![
                AttentionTrigger::literal(
                    "✔ Submit",
                    true,
                    AttentionType::DecisionPoint,
                    PreviewStrategy::LastLines(3),
                ),
                AttentionTrigger::literal(
                    "Enter to select",
                    true,
                    AttentionType::InputRequired,
                    PreviewStrategy::LastLines(3),
                ),
                AttentionTrigger::literal(
                    "❯",
                    false,
                    AttentionType::InputRequired,
                    PreviewStrategy::Surround {
                        before: 40,
                        after: 0,
                    },
                ),
            ],
        },
        AttentionProfile {
            name: "build-tools",
            triggers: vec![
                AttentionTrigger::regex(
                    r"(?i)build (succeeded|complete|passed)",
                    AttentionType::Completed,
                    PreviewStrategy::LastLines(5),
                ),
                AttentionTrigger::regex(
                    r"(?i)(error|failed|failure):",
                    AttentionType::Error,
                    PreviewStrategy::Surround {
                        before: 0,
                        after: 80,
                    },
                ),
            ],
        },
    ]
});

/// Definition of triggers for a set of related commands/tools.
#[derive(Debug, Clone)]
pub struct AttentionProfile {
    pub name: &'static str,
    pub triggers: Vec<AttentionTrigger>,
}

#[derive(Debug, Clone)]
pub struct AttentionTrigger {
    matcher: Regex,
    attention_type: AttentionType,
    preview: PreviewStrategy,
}

impl AttentionTrigger {
    fn literal(
        pattern: &str,
        case_insensitive: bool,
        attention_type: AttentionType,
        preview: PreviewStrategy,
    ) -> Self {
        let mut escaped = regex::escape(pattern);
        if case_insensitive {
            escaped = format!("(?i){escaped}");
        }
        let regex = Regex::new(&escaped).expect("failed to compile literal attention matcher");
        Self {
            matcher: regex,
            attention_type,
            preview,
        }
    }

    fn regex(pattern: &str, attention_type: AttentionType, preview: PreviewStrategy) -> Self {
        let regex = Regex::new(pattern).expect("failed to compile attention matcher");
        Self {
            matcher: regex,
            attention_type,
            preview,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PreviewStrategy {
    /// Use the last N lines of output.
    LastLines(usize),
    /// Capture bytes around the match.
    Surround { before: usize, after: usize },
}

impl PreviewStrategy {
    fn render(&self, text: &str, range: Range<usize>) -> String {
        match self {
            PreviewStrategy::LastLines(lines) => {
                if *lines == 0 {
                    return String::new();
                }
                let snippet: Vec<&str> = text
                    .lines()
                    .rev()
                    .take(*lines)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                snippet.join("\n").trim().to_string()
            }
            PreviewStrategy::Surround { before, after } => {
                let start = range.start.saturating_sub(*before);
                let end = usize::min(text.len(), range.end + *after);
                text[start..end].trim().to_string()
            }
        }
    }
}

/// Result of matching a profile.
#[derive(Debug, Clone)]
pub struct AttentionMatch {
    pub profile: &'static str,
    pub attention_type: AttentionType,
    pub preview: String,
}

/// Returns the compiled default profiles.
pub fn default_profiles() -> &'static [AttentionProfile] {
    &DEFAULT_PROFILES
}

/// Attempts to detect an attention event within the provided chunk of text.
pub fn detect_attention(text: &str) -> Option<AttentionMatch> {
    let sanitized = sanitize_text(text);
    let haystack = sanitized.as_ref();

    for profile in default_profiles() {
        for trigger in &profile.triggers {
            if let Some(found) = trigger.matcher.find(haystack) {
                let preview = trigger
                    .preview
                    .render(haystack, found.start()..found.end())
                    .trim()
                    .to_string();
                if preview.is_empty() {
                    continue;
                }
                return Some(AttentionMatch {
                    profile: profile.name,
                    attention_type: trigger.attention_type,
                    preview,
                });
            }
        }
    }
    None
}

/// Default sliding window size for chunked attention detection (8 KiB).
pub const ATTENTION_WINDOW_BYTES: usize = 8 * 1024;

/// Streaming helper that accumulates PTY output across arbitrary chunk boundaries
/// and surfaces attention matches as they appear.
#[derive(Debug)]
pub struct AttentionAccumulator {
    buffer: Vec<u8>,
    max_bytes: usize,
}

impl AttentionAccumulator {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(max_bytes),
            max_bytes,
        }
    }

    /// Push a new chunk of PTY bytes and return any matches that formed across
    /// this chunk boundary.
    pub fn push_chunk(&mut self, chunk: &[u8]) -> Vec<AttentionMatch> {
        let mut matches = Vec::new();
        for &byte in chunk {
            self.buffer.push(byte);
            self.trim_window();
            if let Some(found) = self.detect_current() {
                matches.push(found);
                self.buffer.clear();
            }
        }
        matches
    }

    fn trim_window(&mut self) {
        if self.buffer.len() > self.max_bytes {
            let drain = self.buffer.len() - self.max_bytes;
            self.buffer.drain(..drain);
        }
    }

    fn detect_current(&self) -> Option<AttentionMatch> {
        if self.buffer.is_empty() {
            return None;
        }
        let text = String::from_utf8_lossy(&self.buffer);
        detect_attention(&text)
    }
}

impl Default for AttentionAccumulator {
    fn default() -> Self {
        Self::new(ATTENTION_WINDOW_BYTES)
    }
}

/// Remove ANSI escape sequences from PTY output while preserving readable text.
pub fn strip_ansi_codes(text: &str) -> String {
    if !contains_escape_sequences(text) {
        return text.to_string();
    }

    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\x1b' => handle_escape_sequence(&mut chars),
            '\u{009b}' => skip_csi(&mut chars),
            '\u{009d}' => skip_osc(&mut chars),
            '\u{0090}' | '\u{0098}' => skip_st_terminated(&mut chars),
            '\r' => {}
            _ => output.push(ch),
        }
    }

    output
}

fn sanitize_text<'a>(text: &'a str) -> Cow<'a, str> {
    if contains_escape_sequences(text) {
        Cow::Owned(strip_ansi_codes(text))
    } else {
        Cow::Borrowed(text)
    }
}

fn contains_escape_sequences(text: &str) -> bool {
    text.as_bytes()
        .iter()
        .any(|b| matches!(b, 0x1b | 0x90..=0x9d))
}

type CharIter<'a> = std::iter::Peekable<std::str::Chars<'a>>;

fn handle_escape_sequence(chars: &mut CharIter<'_>) {
    match chars.next() {
        Some('[') => skip_csi(chars),
        Some(']') => skip_osc(chars),
        Some('P') | Some('X') | Some('^') | Some('_') => skip_st_terminated(chars),
        Some('%') | Some('(') | Some(')') | Some('*') | Some('+') | Some('-') | Some('.')
        | Some('/') => {
            // Skip one additional character for charset selection commands.
            let _ = chars.next();
        }
        Some(_) | None => {}
    }
}

fn skip_csi(chars: &mut CharIter<'_>) {
    while let Some(ch) = chars.next() {
        if ('@'..='~').contains(&ch) {
            break;
        }
    }
}

fn skip_osc(chars: &mut CharIter<'_>) {
    while let Some(ch) = chars.next() {
        if ch == '\x07' {
            break;
        }
        if ch == '\x1b' {
            if matches!(chars.peek(), Some('\\')) {
                chars.next();
                break;
            }
        }
    }
}

fn skip_st_terminated(chars: &mut CharIter<'_>) {
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && matches!(chars.peek(), Some('\\')) {
            chars.next();
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_literal_trigger() {
        let text = "Build ready\n✔ Submit\n";
        let matched = detect_attention(text).expect("should detect attention");
        assert_eq!(matched.profile, "claude-code");
        assert_eq!(matched.attention_type, AttentionType::DecisionPoint);
        assert!(matched.preview.contains("✔ Submit"));
    }

    #[test]
    fn detects_regex_trigger() {
        let text = "error: failed to compile";
        let matched = detect_attention(text).expect("should detect error");
        assert_eq!(matched.profile, "build-tools");
        assert_eq!(matched.attention_type, AttentionType::Error);
        assert!(matched.preview.contains("failed to compile"));
    }

    #[test]
    fn returns_none_when_no_profiles_match() {
        let text = "all good";
        assert!(detect_attention(text).is_none());
    }

    #[test]
    fn strip_ansi_removes_color_and_cursor_codes() {
        let text = "\x1b[32m✔\x1b[0m Submit\n\x1b[2K\rNext line";
        assert_eq!(strip_ansi_codes(text), "✔ Submit\nNext line");
    }

    #[test]
    fn strip_ansi_removes_osc_sequences() {
        let text = "\x1b]0;#123: Demo task\x07Prompt ready";
        assert_eq!(strip_ansi_codes(text), "Prompt ready");
    }

    #[test]
    fn strip_ansi_handles_partial_sequences_gracefully() {
        let text = "partial \x1b[32mstring\x1b[";
        assert_eq!(strip_ansi_codes(text), "partial string");
    }

    #[test]
    fn detects_literal_trigger_with_ansi_codes() {
        let text = "\x1b[32m✔\x1b[0m Submit";
        let matched = detect_attention(text).expect("should detect attention with colors");
        assert_eq!(matched.profile, "claude-code");
        assert!(matched.preview.contains("✔ Submit"));
    }

    #[test]
    fn detects_regex_trigger_with_ansi_codes() {
        let text = "compile output\n\x1b[31merror:\x1b[0m failed to build";
        let matched = detect_attention(text).expect("should detect ansi wrapped error");
        assert_eq!(matched.profile, "build-tools");
        assert_eq!(matched.attention_type, AttentionType::Error);
        assert!(matched.preview.contains("failed to build"));
    }

    #[test]
    fn accumulator_detects_split_literal_trigger() {
        let mut acc = AttentionAccumulator::new(64);
        assert!(acc.push_chunk("Build succ".as_bytes()).is_empty());
        let matches = acc.push_chunk("eeded\n".as_bytes());
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].profile, "build-tools");
    }

    #[test]
    fn accumulator_handles_multibyte_boundaries() {
        let mut acc = AttentionAccumulator::new(64);
        let check = "✔".as_bytes();
        assert!(acc.push_chunk(&check[..1]).is_empty());
        assert!(acc.push_chunk(&check[1..2]).is_empty());
        let mut tail = Vec::new();
        tail.extend_from_slice(&check[2..]);
        tail.extend_from_slice(" Submit".as_bytes());
        let matches = acc.push_chunk(&tail);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].profile, "claude-code");
    }

    #[test]
    fn accumulator_detects_multiple_matches_in_one_chunk() {
        let mut acc = AttentionAccumulator::new(256);
        let chunk = b"Build succeeded\nerror: failed to compile\n";
        let matches = acc.push_chunk(chunk);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].profile, "build-tools");
        assert_eq!(matches[0].attention_type, AttentionType::Completed);
        assert_eq!(matches[1].attention_type, AttentionType::Error);
    }
}
