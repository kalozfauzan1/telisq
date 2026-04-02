// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

use shared::errors::ParseError;
use shared::types::TaskSpec;

use crate::graph::TaskGraph;

/// Validates a Telisq plan.
pub fn validate_plan(tasks: &[TaskSpec]) -> Result<(), ParseError> {
    // Check for duplicate task IDs
    let mut task_ids = std::collections::HashSet::new();
    for task in tasks {
        if task_ids.contains(&task.id) {
            return Err(ParseError::DuplicateId {
                line: 0,
                id: task.id.clone(),
            });
        }
        task_ids.insert(task.id.clone());
    }

    // Check for valid task dependencies
    for task in tasks {
        for dep_id in &task.dependencies {
            if !task_ids.contains(dep_id) {
                return Err(ParseError::SyntaxError {
                    line: 0,
                    message: format!("Task {} depends on unknown task {}", task.id, dep_id),
                });
            }
        }
    }

    // Create task graph and validate it
    let graph = TaskGraph::new(tasks.to_vec())?;
    graph.validate()?;

    Ok(())
}

/// Validates a task's files.
pub fn validate_task_files(task: &TaskSpec) -> Result<(), ParseError> {
    for file in &task.files {
        if file.to_str().is_none() {
            return Err(ParseError::SyntaxError {
                line: 0,
                message: format!("Invalid file path: {:?}", file),
            });
        }
    }

    Ok(())
}

/// Validates a task's contracts.
pub fn validate_task_contracts(task: &TaskSpec) -> Result<(), ParseError> {
    for contract in &task.contracts {
        if contract.trim().is_empty() {
            return Err(ParseError::SyntaxError {
                line: 0,
                message: "Contract cannot be empty".into(),
            });
        }
    }

    Ok(())
}

/// Validates all tasks in a plan.
pub fn validate_all_tasks(tasks: &[TaskSpec]) -> Vec<ParseError> {
    let mut errors = Vec::new();

    // Validate each task's files and contracts
    for task in tasks {
        if let Err(e) = validate_task_files(task) {
            errors.push(e);
        }
        if let Err(e) = validate_task_contracts(task) {
            errors.push(e);
        }
    }

    // Validate the plan structure
    if let Err(e) = validate_plan(tasks) {
        errors.push(e);
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::types::TaskSpec;

    #[test]
    fn test_validate_valid_plan() {
        let task1 = TaskSpec::new("1", "Task 1");
        let task2 = TaskSpec::new("2", "Task 2");

        let errors = validate_all_tasks(&[task1, task2]);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_duplicate_task_ids() {
        let task1 = TaskSpec::new("1", "Task 1");
        let task2 = TaskSpec::new("1", "Task 2");

        let errors = validate_all_tasks(&[task1, task2]);
        assert!(!errors.is_empty());
        assert!(errors
            .iter()
            .any(|e| matches!(e, ParseError::DuplicateId { .. })));
    }

    #[test]
    fn test_validate_unknown_dependency() {
        let mut task1 = TaskSpec::new("1", "Task 1");
        task1.add_dependency("unknown");

        let errors = validate_all_tasks(&[task1]);
        assert!(!errors.is_empty());
        assert!(errors
            .iter()
            .any(|e| matches!(e, ParseError::SyntaxError { .. })));
    }

    #[test]
    fn test_validate_empty_contract() {
        let mut task1 = TaskSpec::new("1", "Task 1");
        task1.add_contract("");

        let errors = validate_all_tasks(&[task1]);
        assert!(!errors.is_empty());
        assert!(errors
            .iter()
            .any(|e| matches!(e, ParseError::SyntaxError { .. })));
    }
}
