// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

use petgraph::algo::toposort;
use petgraph::graph::DiGraph;
use petgraph::prelude::NodeIndex;
use std::collections::HashMap;

use shared::errors::ParseError;
use shared::types::{TaskId, TaskSpec, TaskStatus};

/// Dependency graph for tasks in a plan.
pub struct TaskGraph {
    graph: DiGraph<TaskSpec, ()>,
    id_to_index: HashMap<TaskId, NodeIndex>,
}

impl TaskGraph {
    /// Creates a new task graph from a list of tasks.
    pub fn new(tasks: Vec<TaskSpec>) -> Result<Self, ParseError> {
        let mut graph = DiGraph::new();
        let mut id_to_index = HashMap::new();
        let mut task_list = Vec::new();

        // Add all nodes to the graph and collect tasks
        for task in tasks {
            let index = graph.add_node(task.clone());
            id_to_index.insert(task.id.clone(), index);
            task_list.push(task);
        }

        // Add all edges to the graph
        for task in task_list {
            let task_index = *id_to_index.get(&task.id).unwrap();
            for dep_id in &task.dependencies {
                match id_to_index.get(dep_id) {
                    Some(&dep_index) => {
                        graph.add_edge(task_index, dep_index, ());
                    }
                    None => {
                        return Err(ParseError::SyntaxError {
                            line: 0,
                            message: format!("Task {} depends on unknown task {}", task.id, dep_id),
                        });
                    }
                }
            }
        }

        Ok(Self { graph, id_to_index })
    }

    /// Validates the task graph for cycles and other errors.
    pub fn validate(&self) -> Result<(), ParseError> {
        // Check for cycles
        if toposort(&self.graph, None).is_err() {
            return Err(ParseError::SyntaxError {
                line: 0,
                message: "Plan contains cycles in task dependencies".into(),
            });
        }

        Ok(())
    }

    /// Gets all tasks that are ready to be run (i.e., all dependencies are completed).
    pub fn get_runnable_tasks(&self) -> Vec<&TaskSpec> {
        let mut runnable = Vec::new();

        for task in self.graph.node_weights() {
            // If task is already completed or in progress, skip
            if task.status == TaskStatus::Completed || task.status == TaskStatus::InProgress {
                continue;
            }

            // Check if all dependencies are completed
            let mut all_deps_completed = true;
            for dep_id in &task.dependencies {
                if let Some(&dep_index) = self.id_to_index.get(dep_id) {
                    let dep_task = &self.graph[dep_index];
                    if dep_task.status != TaskStatus::Completed {
                        all_deps_completed = false;
                        break;
                    }
                }
            }

            if all_deps_completed {
                runnable.push(task);
            }
        }

        runnable
    }

    /// Gets a task by its ID.
    pub fn get_task(&self, id: &str) -> Option<&TaskSpec> {
        self.id_to_index.get(id).map(|&index| &self.graph[index])
    }

    /// Returns an iterator over all tasks in the graph.
    pub fn tasks(&self) -> impl Iterator<Item = &TaskSpec> {
        self.graph.node_weights()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::types::TaskSpec;

    #[test]
    fn test_simple_dependency_graph() {
        let mut task1 = TaskSpec::new("1", "Task 1");
        task1.set_status(TaskStatus::Completed);

        let mut task2 = TaskSpec::new("2", "Task 2");
        task2.set_status(TaskStatus::Pending);
        task2.add_dependency("1");

        let tasks = vec![task1, task2];

        let graph = TaskGraph::new(tasks).unwrap();
        graph.validate().unwrap();

        let runnable = graph.get_runnable_tasks();
        assert_eq!(runnable.len(), 1);
        assert_eq!(runnable[0].id, "2");
    }

    #[test]
    fn test_cyclic_dependency() {
        let mut task1 = TaskSpec::new("1", "Task 1");
        task1.add_dependency("2");

        let mut task2 = TaskSpec::new("2", "Task 2");
        task2.add_dependency("1");

        let tasks = vec![task1, task2];

        let graph = TaskGraph::new(tasks).unwrap();
        assert!(graph.validate().is_err());
    }

    #[test]
    fn test_unknown_dependency() {
        let mut task1 = TaskSpec::new("1", "Task 1");
        task1.add_dependency("unknown");

        let tasks = vec![task1];

        let result = TaskGraph::new(tasks);
        assert!(result.is_err());
    }
}
