#[cfg(test)]
mod tests {
    use core::orchestrator::Orchestrator;
    use plan::parser::PlanParser;
    use std::fs;
    use std::path::PathBuf;
    use tempdir::TempDir;

    #[test]
    fn test_orchestrator_initialization() {
        let temp_dir = TempDir::new("orchestrator-test").expect("Failed to create temp dir");
        
        let plan_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/plans/simple.plan.md");
        
        let content = fs::read_to_string(&plan_path).expect("Failed to read plan file");
        let plan = PlanParser::parse(&content).expect("Failed to parse plan");
        
        let orchestrator = Orchestrator::new(temp_dir.path(), plan);
        
        assert!(orchestrator.is_ok());
    }

    #[test]
    fn test_orchestrator_task_ordering() {
        let temp_dir = TempDir::new("orchestrator-test").expect("Failed to create temp dir");
        
        let plan_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/plans/dependency.plan.md");
        
        let content = fs::read_to_string(&plan_path).expect("Failed to read plan file");
        let plan = PlanParser::parse(&content).expect("Failed to parse plan");
        
        let orchestrator = Orchestrator::new(temp_dir.path(), plan).expect("Failed to create orchestrator");
        
        // Get the task execution order
        let task_order = orchestrator.get_task_order();
        
        assert!(!task_order.is_empty());
        
        // Check that dependencies are ordered correctly
        let task_names: Vec<String> = task_order
            .iter()
            .map(|task| task.name.clone())
            .collect();
        
        // Task 3 depends on Task 1
        let task1_index = task_names
            .iter()
            .position(|n| n.contains("Task 1"))
            .expect("Task 1 not found");
        let task3_index = task_names
            .iter()
            .position(|n| n.contains("Task 3"))
            .expect("Task 3 not found");
        assert!(task1_index < task3_index);
        
        // Task 4 depends on Task 3
        let task4_index = task_names
            .iter()
            .position(|n| n.contains("Task 4"))
            .expect("Task 4 not found");
        assert!(task3_index < task4_index);
        
        // Task 6 depends on Task 5
        let task5_index = task_names
            .iter()
            .position(|n| n.contains("Task 5"))
            .expect("Task 5 not found");
        let task6_index = task_names
            .iter()
            .position(|n| n.contains("Task 6"))
            .expect("Task 6 not found");
        assert!(task5_index < task6_index);
    }

    #[test]
    fn test_orchestrator_progress_tracking() {
        let temp_dir = TempDir::new("orchestrator-test").expect("Failed to create temp dir");
        
        let plan_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/plans/simple.plan.md");
        
        let content = fs::read_to_string(&plan_path).expect("Failed to read plan file");
        let plan = PlanParser::parse(&content).expect("Failed to parse plan");
        
        let mut orchestrator = Orchestrator::new(temp_dir.path(), plan).expect("Failed to create orchestrator");
        
        // Check initial progress is 0%
        assert_eq!(orchestrator.get_progress(), 0.0);
        
        // Mark first task as completed
        let first_task_id = orchestrator.get_task_order().first().unwrap().id.clone();
        orchestrator.mark_task_completed(&first_task_id).expect("Failed to mark task completed");
        
        // Progress should be around 16.67% (1/6 tasks)
        assert!(orchestrator.get_progress() > 0.0 && orchestrator.get_progress() < 1.0);
    }
}
