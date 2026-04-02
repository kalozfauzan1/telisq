#[cfg(test)]
mod tests {
    use plan::parser::PlanParser;
    use plan::tracker::PlanTracker;
    use std::fs;
    use std::path::PathBuf;
    use tempdir::TempDir;

    #[test]
    fn test_tracker_initialization() {
        let plan_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/plans/simple.plan.md");
        
        let content = fs::read_to_string(&plan_path).expect("Failed to read plan file");
        let plan = PlanParser::parse(&content).expect("Failed to parse plan");
        
        let temp_dir = TempDir::new("plan-tracker-test").expect("Failed to create temp dir");
        let tracker = PlanTracker::new(temp_dir.path(), &plan).expect("Failed to create tracker");
        
        assert!(!tracker.get_completed_tasks().is_empty());
        assert!(tracker.get_completed_tasks().values().all(|&completed| !completed));
    }

    #[test]
    fn test_task_completion_tracking() {
        let plan_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/plans/simple.plan.md");
        
        let content = fs::read_to_string(&plan_path).expect("Failed to read plan file");
        let plan = PlanParser::parse(&content).expect("Failed to parse plan");
        
        let temp_dir = TempDir::new("plan-tracker-test").expect("Failed to create temp dir");
        let mut tracker = PlanTracker::new(temp_dir.path(), &plan).expect("Failed to create tracker");
        
        let first_task_id = tracker.get_completed_tasks().keys().next().expect("No tasks found");
        tracker.mark_task_completed(first_task_id, true).expect("Failed to mark task completed");
        
        assert!(tracker.is_task_completed(first_task_id));
    }

    #[test]
    fn test_tracker_persistence() {
        let plan_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/plans/simple.plan.md");
        
        let content = fs::read_to_string(&plan_path).expect("Failed to read plan file");
        let plan = PlanParser::parse(&content).expect("Failed to parse plan");
        
        let temp_dir = TempDir::new("plan-tracker-test").expect("Failed to create temp dir");
        
        // Create and use tracker
        let mut tracker = PlanTracker::new(temp_dir.path(), &plan).expect("Failed to create tracker");
        let first_task_id = tracker.get_completed_tasks().keys().next().expect("No tasks found");
        tracker.mark_task_completed(first_task_id, true).expect("Failed to mark task completed");
        
        // Recreate tracker from same directory
        let new_tracker = PlanTracker::new(temp_dir.path(), &plan).expect("Failed to create tracker");
        
        assert!(new_tracker.is_task_completed(first_task_id));
    }

    #[test]
    fn test_task_uncompletion() {
        let plan_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/plans/simple.plan.md");
        
        let content = fs::read_to_string(&plan_path).expect("Failed to read plan file");
        let plan = PlanParser::parse(&content).expect("Failed to parse plan");
        
        let temp_dir = TempDir::new("plan-tracker-test").expect("Failed to create temp dir");
        let mut tracker = PlanTracker::new(temp_dir.path(), &plan).expect("Failed to create tracker");
        
        let first_task_id = tracker.get_completed_tasks().keys().next().expect("No tasks found");
        tracker.mark_task_completed(first_task_id, true).expect("Failed to mark task completed");
        tracker.mark_task_completed(first_task_id, false).expect("Failed to mark task uncompleted");
        
        assert!(!tracker.is_task_completed(first_task_id));
    }
}
