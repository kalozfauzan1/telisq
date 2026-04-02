#[cfg(test)]
mod tests {
    use plan::graph::PlanGraph;
    use plan::parser::PlanParser;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_graph_construction_simple_plan() {
        let plan_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/plans/simple.plan.md");
        
        let content = fs::read_to_string(&plan_path).expect("Failed to read plan file");
        let plan = PlanParser::parse(&content).expect("Failed to parse plan");
        
        let graph = PlanGraph::from_plan(&plan);
        assert!(!graph.nodes.is_empty());
        
        // Check that all tasks are in the graph
        let mut task_count = 0;
        for phase in &plan.phases {
            task_count += phase.tasks.len();
        }
        
        assert_eq!(graph.nodes.len(), task_count);
    }

    #[test]
    fn test_graph_construction_dependency_plan() {
        let plan_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/plans/dependency.plan.md");
        
        let content = fs::read_to_string(&plan_path).expect("Failed to read plan file");
        let plan = PlanParser::parse(&content).expect("Failed to parse plan");
        
        let graph = PlanGraph::from_plan(&plan);
        assert!(!graph.nodes.is_empty());
        
        // Check that dependencies are properly represented
        let mut has_dependencies = false;
        for node in &graph.nodes {
            if !node.dependencies.is_empty() {
                has_dependencies = true;
                break;
            }
        }
        
        assert!(has_dependencies);
    }

    #[test]
    fn test_topological_sorting() {
        let plan_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/plans/dependency.plan.md");
        
        let content = fs::read_to_string(&plan_path).expect("Failed to read plan file");
        let plan = PlanParser::parse(&content).expect("Failed to parse plan");
        
        let graph = PlanGraph::from_plan(&plan);
        let sorted_nodes = graph.topological_sort();
        assert!(!sorted_nodes.is_empty());
        
        // Verify dependencies come before dependent tasks
        for node in &sorted_nodes {
            for dep_id in &node.dependencies {
                let dep_index = sorted_nodes
                    .iter()
                    .position(|n| n.id == *dep_id)
                    .expect("Dependency not found in sorted list");
                let node_index = sorted_nodes
                    .iter()
                    .position(|n| n.id == node.id)
                    .expect("Node not found in sorted list");
                
                assert!(dep_index < node_index, "Dependency should come before dependent task");
            }
        }
    }
}
