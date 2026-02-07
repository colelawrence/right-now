// Shell integration for Right Now
//
// Provides prompt and terminal title integration for various shells.
// Detects shell type, generates snippets, and manages rc file installation.

use anyhow::{anyhow, Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;

/// Marker comments for identifying our integration block
const MARKER_START: &str = "# >>> Right Now >>>";
const MARKER_END: &str = "# <<< Right Now <<<";

/// Supported shell types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShellType {
    Zsh,
    Bash,
    Fish,
}

impl ShellType {
    /// Detect shell type from $SHELL environment variable
    pub fn detect() -> Option<Self> {
        let shell = env::var("SHELL").ok()?;
        Self::from_path(&shell)
    }

    /// Parse shell type from a path
    pub fn from_path(path: &str) -> Option<Self> {
        let shell_name = path.rsplit('/').next()?;
        match shell_name {
            "zsh" => Some(ShellType::Zsh),
            "bash" => Some(ShellType::Bash),
            "fish" => Some(ShellType::Fish),
            _ => None,
        }
    }

    /// Get the default RC file path for this shell
    pub fn rc_file_path(&self) -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;

        let path = match self {
            ShellType::Zsh => home.join(".zshrc"),
            ShellType::Bash => {
                // Prefer .bashrc, but use .bash_profile if .bashrc doesn't exist
                let bashrc = home.join(".bashrc");
                if bashrc.exists() {
                    bashrc
                } else {
                    home.join(".bash_profile")
                }
            }
            ShellType::Fish => home.join(".config/fish/config.fish"),
        };

        Ok(path)
    }

    /// Generate the shell integration snippet for this shell type
    pub fn integration_snippet(&self) -> &'static str {
        match self {
            ShellType::Zsh => ZSH_SNIPPET,
            ShellType::Bash => BASH_SNIPPET,
            ShellType::Fish => FISH_SNIPPET,
        }
    }
}

impl std::fmt::Display for ShellType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellType::Zsh => write!(f, "zsh"),
            ShellType::Bash => write!(f, "bash"),
            ShellType::Fish => write!(f, "fish"),
        }
    }
}

impl std::str::FromStr for ShellType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "zsh" => Ok(ShellType::Zsh),
            "bash" => Ok(ShellType::Bash),
            "fish" => Ok(ShellType::Fish),
            _ => Err(anyhow!(
                "Unknown shell type: {}. Supported: zsh, bash, fish",
                s
            )),
        }
    }
}

/// Zsh integration snippet
const ZSH_SNIPPET: &str = r#"# >>> Right Now >>>
# Shell integration for Right Now task manager
# Shows current task in prompt and terminal title
_right_now_prompt() {
  if [[ -n "$RIGHT_NOW_SESSION_ID" ]]; then
    local _rn_task="${RIGHT_NOW_TASK_DISPLAY:-$RIGHT_NOW_TASK_KEY}"
    echo "[#$RIGHT_NOW_SESSION_ID: $_rn_task] "
  fi
}
PROMPT='$(_right_now_prompt)'$PROMPT

# Set terminal title
_right_now_precmd() {
  if [[ -n "$RIGHT_NOW_SESSION_ID" ]]; then
    local _rn_task="${RIGHT_NOW_TASK_DISPLAY:-$RIGHT_NOW_TASK_KEY}"
    printf '\033]0;#%s: %s\007' "$RIGHT_NOW_SESSION_ID" "$_rn_task"
  fi
}
precmd_functions+=(_right_now_precmd)
# <<< Right Now <<<"#;

/// Bash integration snippet
const BASH_SNIPPET: &str = r#"# >>> Right Now >>>
# Shell integration for Right Now task manager
# Shows current task in prompt and terminal title
_right_now_prompt() {
  if [[ -n "$RIGHT_NOW_SESSION_ID" ]]; then
    local _rn_task="${RIGHT_NOW_TASK_DISPLAY:-$RIGHT_NOW_TASK_KEY}"
    echo "[#$RIGHT_NOW_SESSION_ID: $_rn_task] "
  fi
}
PS1='$(_right_now_prompt)'$PS1

# Set terminal title via PROMPT_COMMAND
_right_now_title() {
  if [[ -n "$RIGHT_NOW_SESSION_ID" ]]; then
    local _rn_task="${RIGHT_NOW_TASK_DISPLAY:-$RIGHT_NOW_TASK_KEY}"
    printf '\033]0;#%s: %s\007' "$RIGHT_NOW_SESSION_ID" "$_rn_task"
  fi
}
PROMPT_COMMAND="_right_now_title${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
# <<< Right Now <<<"#;

/// Fish integration snippet
const FISH_SNIPPET: &str = r#"# >>> Right Now >>>
# Shell integration for Right Now task manager
# Shows current task in prompt and terminal title
function _right_now_prompt
  if set -q RIGHT_NOW_SESSION_ID
    set -l _rn_task $RIGHT_NOW_TASK_DISPLAY
    if test -z "$_rn_task"
      set _rn_task $RIGHT_NOW_TASK_KEY
    end
    echo "[#$RIGHT_NOW_SESSION_ID: $_rn_task] "
  end
end

# Wrap existing fish_prompt if not already wrapped
if not functions -q _right_now_original_fish_prompt
  functions -c fish_prompt _right_now_original_fish_prompt
  function fish_prompt
    _right_now_prompt
    _right_now_original_fish_prompt
  end
end

# Set terminal title
function _right_now_title --on-event fish_prompt
  if set -q RIGHT_NOW_SESSION_ID
    set -l _rn_task $RIGHT_NOW_TASK_DISPLAY
    if test -z "$_rn_task"
      set _rn_task $RIGHT_NOW_TASK_KEY
    end
    printf '\033]0;#%s: %s\007' $RIGHT_NOW_SESSION_ID $_rn_task
  end
end
# <<< Right Now <<<"#;

/// Install shell integration by appending to the rc file
pub fn install(shell: ShellType, rc_path: Option<PathBuf>) -> Result<PathBuf> {
    let rc_file = rc_path.map(Ok).unwrap_or_else(|| shell.rc_file_path())?;

    // Read existing content (or empty if file doesn't exist)
    let existing = if rc_file.exists() {
        fs::read_to_string(&rc_file)
            .with_context(|| format!("Failed to read {}", rc_file.display()))?
    } else {
        // For fish, we may need to create the config directory
        if shell == ShellType::Fish {
            if let Some(parent) = rc_file.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create {}", parent.display()))?;
            }
        }
        String::new()
    };

    // Check if already installed
    if existing.contains(MARKER_START) {
        // Already installed, do nothing (idempotent)
        return Ok(rc_file);
    }

    // Append the integration snippet
    let snippet = shell.integration_snippet();
    let new_content = if existing.is_empty() || existing.ends_with('\n') {
        format!("{}{}\n", existing, snippet)
    } else {
        format!("{}\n\n{}\n", existing, snippet)
    };

    fs::write(&rc_file, new_content)
        .with_context(|| format!("Failed to write to {}", rc_file.display()))?;

    Ok(rc_file)
}

/// Uninstall shell integration by removing the marked block from the rc file
pub fn uninstall(rc_path: &PathBuf) -> Result<bool> {
    if !rc_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(rc_path)
        .with_context(|| format!("Failed to read {}", rc_path.display()))?;

    // Find and remove the integration block
    let start_idx = content.find(MARKER_START);
    let end_idx = content.find(MARKER_END);

    match (start_idx, end_idx) {
        (Some(start), Some(end)) if start < end => {
            // Find the end of the marker line
            let block_end = content[end..]
                .find('\n')
                .map(|i| end + i + 1)
                .unwrap_or(content.len());

            // Remove any leading newlines before the block
            let mut block_start = start;
            while block_start > 0 && content.as_bytes()[block_start - 1] == b'\n' {
                block_start -= 1;
            }
            // Keep at least one newline if there's content before
            if block_start > 0 {
                block_start += 1;
            }

            let new_content = format!("{}{}", &content[..block_start], &content[block_end..]);

            // Trim trailing whitespace but keep one final newline
            let trimmed = new_content.trim_end();
            let final_content = if trimmed.is_empty() {
                String::new()
            } else {
                format!("{}\n", trimmed)
            };

            fs::write(rc_path, final_content)
                .with_context(|| format!("Failed to write to {}", rc_path.display()))?;

            Ok(true)
        }
        _ => {
            // Not installed or malformed
            Ok(false)
        }
    }
}

/// Check if shell integration is installed
pub fn is_installed(rc_path: &PathBuf) -> Result<bool> {
    if !rc_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(rc_path)
        .with_context(|| format!("Failed to read {}", rc_path.display()))?;

    Ok(content.contains(MARKER_START) && content.contains(MARKER_END))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_shell_type_detection() {
        assert_eq!(ShellType::from_path("/bin/zsh"), Some(ShellType::Zsh));
        assert_eq!(
            ShellType::from_path("/usr/local/bin/bash"),
            Some(ShellType::Bash)
        );
        assert_eq!(
            ShellType::from_path("/opt/homebrew/bin/fish"),
            Some(ShellType::Fish)
        );
        assert_eq!(ShellType::from_path("/bin/sh"), None);
    }

    #[test]
    fn test_shell_type_from_str() {
        assert_eq!("zsh".parse::<ShellType>().unwrap(), ShellType::Zsh);
        assert_eq!("BASH".parse::<ShellType>().unwrap(), ShellType::Bash);
        assert_eq!("Fish".parse::<ShellType>().unwrap(), ShellType::Fish);
        assert!("sh".parse::<ShellType>().is_err());
    }

    #[test]
    fn test_install_to_empty_file() {
        let temp = TempDir::new().unwrap();
        let rc_file = temp.path().join(".zshrc");

        let result = install(ShellType::Zsh, Some(rc_file.clone())).unwrap();
        assert_eq!(result, rc_file);

        let content = fs::read_to_string(&rc_file).unwrap();
        assert!(content.contains(MARKER_START));
        assert!(content.contains(MARKER_END));
        assert!(content.contains("RIGHT_NOW_SESSION_ID"));
    }

    #[test]
    fn test_install_to_existing_file() {
        let temp = TempDir::new().unwrap();
        let rc_file = temp.path().join(".zshrc");
        fs::write(&rc_file, "# Existing config\nPROMPT='> '\n").unwrap();

        install(ShellType::Zsh, Some(rc_file.clone())).unwrap();

        let content = fs::read_to_string(&rc_file).unwrap();
        assert!(content.contains("# Existing config"));
        assert!(content.contains(MARKER_START));
    }

    #[test]
    fn test_install_is_idempotent() {
        let temp = TempDir::new().unwrap();
        let rc_file = temp.path().join(".zshrc");

        install(ShellType::Zsh, Some(rc_file.clone())).unwrap();
        install(ShellType::Zsh, Some(rc_file.clone())).unwrap();

        let content = fs::read_to_string(&rc_file).unwrap();
        // Should only have ONE block, not two
        assert_eq!(content.matches(MARKER_START).count(), 1);
    }

    #[test]
    fn test_uninstall() {
        let temp = TempDir::new().unwrap();
        let rc_file = temp.path().join(".zshrc");
        fs::write(&rc_file, "# Existing config\nPROMPT='> '\n").unwrap();

        install(ShellType::Zsh, Some(rc_file.clone())).unwrap();
        let removed = uninstall(&rc_file).unwrap();

        assert!(removed);
        let content = fs::read_to_string(&rc_file).unwrap();
        assert!(!content.contains(MARKER_START));
        assert!(content.contains("# Existing config"));
    }

    #[test]
    fn test_uninstall_not_installed() {
        let temp = TempDir::new().unwrap();
        let rc_file = temp.path().join(".zshrc");
        fs::write(&rc_file, "# Just config\n").unwrap();

        let removed = uninstall(&rc_file).unwrap();
        assert!(!removed);
    }

    #[test]
    fn test_is_installed() {
        let temp = TempDir::new().unwrap();
        let rc_file = temp.path().join(".zshrc");

        assert!(!is_installed(&rc_file).unwrap());

        install(ShellType::Zsh, Some(rc_file.clone())).unwrap();
        assert!(is_installed(&rc_file).unwrap());

        uninstall(&rc_file).unwrap();
        assert!(!is_installed(&rc_file).unwrap());
    }

    /// E2E test: verify bash can actually source the integration without syntax errors
    /// and that the prompt function produces expected output
    #[test]
    fn test_bash_integration_e2e() {
        let temp = TempDir::new().unwrap();
        let rc_file = temp.path().join(".bashrc");

        install(ShellType::Bash, Some(rc_file.clone())).unwrap();

        // Run bash, explicitly source the file, then call the function
        // --rcfile only works in interactive mode, so we source explicitly
        let script = format!("source '{}' && _right_now_prompt", rc_file.display());
        let output = std::process::Command::new("bash")
            .arg("-c")
            .arg(&script)
            .env("RIGHT_NOW_SESSION_ID", "42")
            .env("RIGHT_NOW_TASK_KEY", "Test task")
            .output()
            .expect("Failed to run bash");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "Bash failed to source rc file. stderr: {}",
            stderr
        );
        assert!(
            stdout.contains("[#42: Test task]"),
            "Expected prompt format not found. stdout: '{}', stderr: '{}'",
            stdout,
            stderr
        );
    }

    /// E2E test: verify the prompt function returns nothing when env vars are NOT set
    #[test]
    fn test_bash_integration_no_env_vars() {
        let temp = TempDir::new().unwrap();
        let rc_file = temp.path().join(".bashrc");

        install(ShellType::Bash, Some(rc_file.clone())).unwrap();

        // Run without RIGHT_NOW env vars - prompt function should output nothing
        let script = format!("source '{}' && _right_now_prompt", rc_file.display());
        let output = std::process::Command::new("bash")
            .arg("-c")
            .arg(&script)
            // Explicitly unset the vars in case they exist in test environment
            .env_remove("RIGHT_NOW_SESSION_ID")
            .env_remove("RIGHT_NOW_TASK_KEY")
            .output()
            .expect("Failed to run bash");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "Bash failed when env vars not set. stderr: {}",
            stderr
        );
        assert!(
            stdout.trim().is_empty(),
            "Prompt should be empty when env vars not set, got: '{}'",
            stdout
        );
    }

    #[test]
    fn test_bash_integration_prefers_display_env() {
        let temp = TempDir::new().unwrap();
        let rc_file = temp.path().join(".bashrc");

        install(ShellType::Bash, Some(rc_file.clone())).unwrap();

        let script = format!("source '{}' && _right_now_prompt", rc_file.display());
        let output = std::process::Command::new("bash")
            .arg("-c")
            .arg(&script)
            .env("RIGHT_NOW_SESSION_ID", "77")
            .env("RIGHT_NOW_TASK_KEY", "$(whoami)")
            .env("RIGHT_NOW_TASK_DISPLAY", "Literal Task")
            .output()
            .expect("Failed to run bash");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "Bash failed to source rc file. stderr: {}",
            stderr
        );
        assert!(
            stdout.contains("[#77: Literal Task]"),
            "Display env var should override raw task key. stdout: '{}', stderr: '{}'",
            stdout,
            stderr
        );
    }
}
