use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

const INITIAL_DELAY_MS: u64 = 50;
const MAX_RETRIES: u32 = 10;
const MAX_DELAY_MS: u64 = 1_000;

/// Error returned when waiting for file content times out.
#[derive(Debug)]
pub struct WaitError {
    path: PathBuf,
    attempts: u32,
    waited: Duration,
    last_content: Option<String>,
    last_error: Option<String>,
}

impl WaitError {
    fn new(
        path: &Path,
        attempts: u32,
        waited: Duration,
        last_content: Option<String>,
        last_error: Option<String>,
    ) -> Self {
        Self {
            path: path.to_path_buf(),
            attempts,
            waited,
            last_content,
            last_error,
        }
    }
}

impl fmt::Display for WaitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Timed out after {} attempts over {:?} waiting for {}. Last content: {}. Last error: {}",
            self.attempts,
            self.waited,
            self.path.display(),
            self.last_content
                .as_deref()
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or("<empty>"),
            self.last_error
                .as_deref()
                .unwrap_or("file not created or unreadable")
        )
    }
}

impl std::error::Error for WaitError {}

/// Poll a file for content until the provided predicate returns true.
///
/// Uses exponential backoff starting at 50ms with a maximum of 10 retries.
pub fn wait_for_file_content<P, F>(
    path: P,
    predicate: F,
    timeout: Duration,
) -> Result<String, WaitError>
where
    P: AsRef<Path>,
    F: Fn(&str) -> bool,
{
    let path = path.as_ref();
    let start = Instant::now();
    let mut delay = Duration::from_millis(INITIAL_DELAY_MS);
    let mut attempts = 0;
    let mut last_error: Option<String> = None;
    let mut last_content: Option<String> = None;

    loop {
        attempts += 1;
        match fs::read_to_string(path) {
            Ok(content) => {
                if predicate(&content) {
                    return Ok(content);
                }
                last_content = Some(content);
            }
            Err(err) => {
                if err.kind() != ErrorKind::NotFound {
                    last_error = Some(err.to_string());
                }
            }
        }

        if attempts >= MAX_RETRIES || start.elapsed() >= timeout {
            break;
        }

        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            break;
        }

        thread::sleep(delay.min(remaining));
        delay = delay
            .checked_mul(2)
            .unwrap_or_else(|| Duration::from_millis(MAX_DELAY_MS))
            .min(Duration::from_millis(MAX_DELAY_MS));
    }

    Err(WaitError::new(
        path,
        attempts,
        start.elapsed(),
        last_content,
        last_error,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn wait_for_file_content_returns_existing_content() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("value.txt");
        fs::write(&file, "ready").unwrap();

        let content =
            wait_for_file_content(&file, |text| text.contains("ready"), Duration::from_secs(1))
                .expect("should read file immediately");
        assert_eq!(content, "ready");
    }

    #[test]
    fn wait_for_file_content_times_out_with_context() {
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("missing.txt");

        let err = wait_for_file_content(
            &file,
            |text| text.contains("anything"),
            Duration::from_millis(1),
        )
        .expect_err("should time out");
        assert!(
            err.to_string().contains("missing.txt"),
            "error message should reference file path"
        );
    }
}
