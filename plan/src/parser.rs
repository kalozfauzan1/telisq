// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use shared::errors::ParseError;
use shared::types::{TaskSpec, TaskStatus};

/// Parses a Telisq plan file.
pub fn parse_plan<P: AsRef<Path>>(path: P) -> Result<Vec<TaskSpec>, ParseError> {
    let content = std::fs::read_to_string(&path).map_err(|e| ParseError::SyntaxError {
        line: 0,
        message: format!("Failed to read file: {}", e),
    })?;

    parse_plan_content(&content)
}

/// Parses Telisq plan content from a string.
pub fn parse_plan_content(content: &str) -> Result<Vec<TaskSpec>, ParseError> {
    let mut tasks = Vec::new();
    let mut task_ids = HashSet::new();
    let lines: Vec<&str> = content.lines().collect();

    // Regular expressions for parsing
    let task_re =
        Regex::new(r#"^- \[(?P<status>[ x!~-])\] (?P<title>.*?)(?: \((?P<id>[\w-]+)\))?$"#)
            .unwrap();
    let files_re = Regex::new(r#"^\s*Files:\s*(?P<files>.*)$"#).unwrap();
    let contract_re = Regex::new(r#"^\s*Contract:?\s*(?P<contract>.*)$"#).unwrap();
    let depends_re = Regex::new(r#"^\s*Depends on:?\s*(?P<depends>.*)$"#).unwrap();

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;

        // Skip empty lines and comments
        if line.trim().is_empty() || line.trim().starts_with('#') {
            continue;
        }

        // Parse task
        if let Some(captures) = task_re.captures(line) {
            let status = match captures.name("status").unwrap().as_str() {
                " " => TaskStatus::Pending,
                "x" => TaskStatus::Completed,
                "!" => TaskStatus::Failed,
                "~" => TaskStatus::InProgress,
                "-" => TaskStatus::Skipped,
                _ => {
                    return Err(ParseError::SyntaxError {
                        line: i as u32,
                        message: format!("Invalid status marker: {}", line),
                    })
                }
            };

            let title = captures.name("title").unwrap().as_str().trim();
            let id = match captures.name("id") {
                Some(c) => c.as_str().to_string(),
                None => {
                    return Err(ParseError::SyntaxError {
                        line: i as u32,
                        message: "Task missing identifier".into(),
                    })
                }
            };

            // Check for duplicate task IDs
            if task_ids.contains(&id) {
                return Err(ParseError::DuplicateId { line: i as u32, id });
            }
            task_ids.insert(id.clone());

            let mut task = TaskSpec::new(id, title);
            task.set_status(status);

            // Parse task details
            while i < lines.len() {
                let detail_line = lines[i];
                if detail_line.trim().is_empty() || task_re.is_match(detail_line) {
                    break;
                }
                if detail_line.trim().starts_with('#') {
                    i += 1;
                    continue;
                }

                // Parse files
                if let Some(captures) = files_re.captures(detail_line) {
                    let files_str = captures.name("files").unwrap().as_str().trim();
                    if !files_str.is_empty() {
                        for file in files_str.split(',') {
                            task.add_file(PathBuf::from(file.trim()));
                        }
                    }
                    i += 1;
                    continue;
                }

                // Parse contract
                if let Some(captures) = contract_re.captures(detail_line) {
                    let contract = captures.name("contract").unwrap().as_str().trim();
                    if !contract.is_empty() {
                        task.add_contract(contract);
                    }
                    i += 1;
                    continue;
                }

                // Parse depends on
                if let Some(captures) = depends_re.captures(detail_line) {
                    let depends_str = captures.name("depends").unwrap().as_str().trim();
                    if !depends_str.is_empty() {
                        for dep in depends_str.split(',') {
                            task.add_dependency(dep.trim());
                        }
                    }
                    i += 1;
                    continue;
                }

                // If we get here, it's an invalid line
                return Err(ParseError::SyntaxError {
                    line: (i + 1) as u32,
                    message: format!("Invalid line: {}", detail_line),
                });
            }

            tasks.push(task);
        } else {
            return Err(ParseError::SyntaxError {
                line: i as u32,
                message: format!("Invalid line format: {}", line),
            });
        }
    }

    // Validate sequential task IDs
    validate_sequential_ids(&tasks)?;

    Ok(tasks)
}

/// Validates that tasks have sequential numeric IDs.
fn validate_sequential_ids(tasks: &[TaskSpec]) -> Result<(), ParseError> {
    // Extract and parse numeric IDs
    let mut numeric_ids: Vec<u32> = Vec::new();
    for task in tasks {
        if let Ok(id_num) = task.id.parse::<u32>() {
            numeric_ids.push(id_num);
        } else {
            return Err(ParseError::SyntaxError {
                line: 0,
                message: format!("Task ID must be numeric: {}", task.id),
            });
        }
    }

    // Sort and check sequentiality
    let mut sorted_ids = numeric_ids.clone();
    sorted_ids.sort_unstable();

    for (i, &id) in sorted_ids.iter().enumerate() {
        let expected = (i + 1) as u32;
        if id != expected {
            return Err(ParseError::SyntaxError {
                line: 0,
                message: format!(
                    "Tasks must have sequential numeric IDs. Expected {} but found {}",
                    expected, id
                ),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_plan() {
        let content = r#"
- [ ] Implement login functionality (1)
  Files: src/login.rs, tests/login_tests.rs
  Contract: User must be able to login with valid credentials
  Depends on: 2

- [x] Set up database (2)
  Files: src/db.rs, migrations/001_create_users_table.sql
"#;

        let tasks = parse_plan_content(content).expect("Failed to parse plan");

        assert_eq!(tasks.len(), 2);

        // Check task 1
        let task1 = &tasks[0];
        assert_eq!(task1.id, "1");
        assert_eq!(task1.title, "Implement login functionality");
        assert_eq!(task1.status, TaskStatus::Pending);
        assert_eq!(
            task1.files,
            vec![
                PathBuf::from("src/login.rs"),
                PathBuf::from("tests/login_tests.rs")
            ]
        );
        assert_eq!(
            task1.contracts,
            vec!["User must be able to login with valid credentials"]
        );
        assert_eq!(task1.dependencies, vec!["2"]);

        // Check task 2
        let task2 = &tasks[1];
        assert_eq!(task2.id, "2");
        assert_eq!(task2.title, "Set up database");
        assert_eq!(task2.status, TaskStatus::Completed);
        assert_eq!(
            task2.files,
            vec![
                PathBuf::from("src/db.rs"),
                PathBuf::from("migrations/001_create_users_table.sql")
            ]
        );
        assert!(task2.contracts.is_empty());
        assert!(task2.dependencies.is_empty());
    }

    #[test]
    fn test_parse_empty_plan() {
        let content = "";
        let tasks = parse_plan_content(content).expect("Failed to parse plan");
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_parse_plan_with_comments() {
        let content = r#"
# This is a comment
- [ ] Task 1 (1)
  # Another comment
  Files: file1.txt

- [x] Task 2 (2)
"#;

        let tasks = parse_plan_content(content).expect("Failed to parse plan");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].title, "Task 1");
        assert_eq!(tasks[1].title, "Task 2");
    }
}
