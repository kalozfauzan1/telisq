#[cfg(test)]
mod tests {
    use plan::parser::PlanParser;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_parse_simple_plan() {
        let plan_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/plans/simple.plan.md");
        
        let content = fs::read_to_string(&plan_path).expect("Failed to read plan file");
        let plan = PlanParser::parse(&content).expect("Failed to parse plan");
        
        assert_eq!(plan.phases.len(), 3);
        assert_eq!(plan.phases[0].tasks.len(), 2);
        assert_eq!(plan.phases[1].tasks.len(), 2);
        assert_eq!(plan.phases[2].tasks.len(), 2);
        
        assert_eq!(plan.phases[0].name, "Phase 1: Initialization");
        assert_eq!(plan.phases[0].tasks[0].name, "Task 1: Set up project structure");
    }

    #[test]
    fn test_parse_dependency_plan() {
        let plan_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/plans/dependency.plan.md");
        
        let content = fs::read_to_string(&plan_path).expect("Failed to read plan file");
        let plan = PlanParser::parse(&content).expect("Failed to parse plan");
        
        assert_eq!(plan.phases.len(), 3);
        assert_eq!(plan.phases[0].tasks.len(), 2);
        assert_eq!(plan.phases[1].tasks.len(), 2);
        assert_eq!(plan.phases[2].tasks.len(), 2);
        
        // Check dependencies
        assert!(!plan.phases[1].tasks[0].dependencies.is_empty());
        assert!(!plan.phases[1].tasks[1].dependencies.is_empty());
        assert!(!plan.phases[2].tasks[0].dependencies.is_empty());
        assert!(!plan.phases[2].tasks[1].dependencies.is_empty());
    }

    #[test]
    fn test_parse_invalid_plan() {
        let invalid_content = "# Invalid Plan\nThis is not a valid plan format";
        let result = PlanParser::parse(invalid_content);
        assert!(result.is_err());
    }
}
