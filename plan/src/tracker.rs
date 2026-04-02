// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

use std::fs;
use std::path::Path;

use regex::Regex;
use tempfile::NamedTempFile;

use shared::errors::ParseError;
use shared::types::TaskStatus;

/// Marker tracker for updating task statuses in plan files atomically.
pub struct MarkerTracker;

impl MarkerTracker {
    /// Updates the status of a task in a plan file.
    pub fn update_marker<P: AsRef<Path>>(
        path: P,
        task_id: &str,
        status: TaskStatus,
    ) -> Result<(), ParseError> {
        let content = fs::read_to_string(&path).map_err(|e| ParseError::SyntaxError {
            line: 0,
            message: format!("Failed to read file: {}", e),
        })?;

        let updated_content = Self::update_marker_in_content(&content, task_id, status)?;

        // Write to temporary file first for atomicity
        let temp_file = NamedTempFile::new_in(path.as_ref().parent().unwrap()).map_err(|e| {
            ParseError::SyntaxError {
                line: 0,
                message: format!("Failed to create temp file: {}", e),
            }
        })?;

        fs::write(&temp_file, updated_content).map_err(|e| ParseError::SyntaxError {
            line: 0,
            message: format!("Failed to write temp file: {}", e),
        })?;

        // Replace original file with temporary file
        fs::rename(temp_file.path(), &path).map_err(|e| ParseError::SyntaxError {
            line: 0,
            message: format!("Failed to rename temp file: {}", e),
        })?;

        Ok(())
    }

    /// Updates the status of a task in plan content.
    pub fn update_marker_in_content(
        content: &str,
        task_id: &str,
        status: TaskStatus,
    ) -> Result<String, ParseError> {
        // Create a regex to match the task line with the specific id
        let task_re = Regex::new(&format!(
            r#"(?P<prefix>- \[)[^\]]+(?P<suffix>\] .*? \({}\))"#,
            regex::escape(task_id)
        ))
        .unwrap();

        // Get the status marker
        let status_char = match status {
            TaskStatus::Pending => " ",
            TaskStatus::InProgress => "~",
            TaskStatus::Completed => "x",
            TaskStatus::Failed => "!",
            TaskStatus::Skipped => "-",
        };

        // Replace the status marker in the task line
        let updated_content = task_re.replace_all(content, |caps: &regex::Captures| {
            format!("{}{}{}", &caps["prefix"], status_char, &caps["suffix"])
        });

        // Check if we actually made a replacement
        if updated_content == content {
            return Err(ParseError::SyntaxError {
                line: 0,
                message: format!("Task not found: {}", task_id),
            });
        }

        Ok(updated_content.into_owned())
    }

    /// Reads the status of a task from a plan file.
    pub fn read_marker<P: AsRef<Path>>(path: P, task_id: &str) -> Result<TaskStatus, ParseError> {
        let content = fs::read_to_string(&path).map_err(|e| ParseError::SyntaxError {
            line: 0,
            message: format!("Failed to read file: {}", e),
        })?;

        Self::read_marker_from_content(&content, task_id)
    }

    /// Reads the status of a task from plan content.
    pub fn read_marker_from_content(
        content: &str,
        task_id: &str,
    ) -> Result<TaskStatus, ParseError> {
        let task_re = Regex::new(&format!(
            r#"- \[(?P<status>[ x!~-])\] .*? \({}\)"#,
            regex::escape(task_id)
        ))
        .unwrap();

        if let Some(captures) = task_re.captures(content) {
            let status_char = captures.name("status").unwrap().as_str();
            let status = match status_char {
                " " => TaskStatus::Pending,
                "x" => TaskStatus::Completed,
                "!" => TaskStatus::Failed,
                "~" => TaskStatus::InProgress,
                "-" => TaskStatus::Skipped,
                _ => {
                    return Err(ParseError::SyntaxError {
                        line: 0,
                        message: format!("Invalid status marker: {}", status_char),
                    })
                }
            };
            Ok(status)
        } else {
            Err(ParseError::SyntaxError {
                line: 0,
                message: format!("Task not found: {}", task_id),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn test_update_marker() {
        let content = r#"
- [ ] Task 1 (1)
  Files: file1.txt
- [x] Task 2 (2)
  Files: file2.txt
"#;

        let updated_content =
            MarkerTracker::update_marker_in_content(content, "1", TaskStatus::InProgress).unwrap();
        assert!(updated_content.contains("- [~] Task 1 (1)"));
        assert!(updated_content.contains("- [x] Task 2 (2)"));
    }

    #[test]
    fn test_read_marker() {
        let content = r#"
- [ ] Task 1 (1)
- [x] Task 2 (2)
"#;

        let status1 = MarkerTracker::read_marker_from_content(content, "1").unwrap();
        assert_eq!(status1, TaskStatus::Pending);

        let status2 = MarkerTracker::read_marker_from_content(content, "2").unwrap();
        assert_eq!(status2, TaskStatus::Completed);
    }

    #[test]
    fn test_update_marker_in_file() {
        // Create temporary file
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap().to_string();

        let initial_content = r#"
- [ ] Task 1 (1)
- [x] Task 2 (2)
"#;

        fs::write(&path, initial_content).unwrap();

        // Update marker
        MarkerTracker::update_marker(&path, "1", TaskStatus::InProgress).unwrap();

        // Read and verify
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("- [~] Task 1 (1)"));
    }
}
