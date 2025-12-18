//! Attention detection for PTY session output.
//!
//! Scans PTY output for patterns that indicate user attention is needed
//! (prompts, errors, completion messages) and extracts preview context.
//!
//! See `docs/attention-notifications.md` for architecture overview.

use crate::session::protocol::AttentionType;
use once_cell::sync::Lazy;
use regex::Regex;
use std::ops::Range;

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
    for profile in default_profiles() {
        for trigger in &profile.triggers {
            if let Some(found) = trigger.matcher.find(text) {
                let preview = trigger
                    .preview
                    .render(text, found.start()..found.end())
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
}
