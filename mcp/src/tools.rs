//! Agent-specific tool definitions and schemas.
//!
//! This module provides tool definitions for different Telisq agents (Plan, Code,
//! Review, Ask, Orchestrator) and enforces agent-specific constraints.

use crate::protocol::ToolDefinition;
use serde_json::json;

// ============================================
// Base File Tools
// ============================================

pub fn create_read_file_tool() -> ToolDefinition {
    ToolDefinition {
        name: "readFile".to_string(),
        description: "Read the contents of a file at the specified path".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file" }
            },
            "required": ["path"]
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "content": { "type": "string", "description": "File contents" }
            }
        }),
    }
}

pub fn create_write_file_tool() -> ToolDefinition {
    ToolDefinition {
        name: "writeFile".to_string(),
        description: "Write content to a file at the specified path".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file" },
                "content": { "type": "string", "description": "Content to write" },
                "overwrite": { "type": "boolean", "description": "Whether to overwrite existing file" }
            },
            "required": ["path", "content"]
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean", "description": "Whether the write operation succeeded" }
            }
        }),
    }
}

pub fn create_edit_file_tool() -> ToolDefinition {
    ToolDefinition {
        name: "editFile".to_string(),
        description: "Edit a file using search and replace".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file" },
                "search": { "type": "string", "description": "Text to search for" },
                "replace": { "type": "string", "description": "Text to replace with" }
            },
            "required": ["path", "search", "replace"]
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean", "description": "Whether the edit operation succeeded" }
            }
        }),
    }
}

pub fn create_delete_file_tool() -> ToolDefinition {
    ToolDefinition {
        name: "deleteFile".to_string(),
        description: "Delete a file at the specified path".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file" }
            },
            "required": ["path"]
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": { "type": "boolean", "description": "Whether the delete operation succeeded" }
            }
        }),
    }
}

// ============================================
// Base Command Tools
// ============================================

pub fn create_run_command_tool() -> ToolDefinition {
    ToolDefinition {
        name: "runCommand".to_string(),
        description: "Run a command in the specified working directory".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Command to execute" },
                "args": { "type": "array", "items": { "type": "string" }, "description": "Command arguments" },
                "cwd": { "type": "string", "description": "Working directory" },
                "blocking": { "type": "boolean", "description": "Whether to block until command completes" }
            },
            "required": ["command"]
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "stdout": { "type": "string", "description": "Standard output" },
                "stderr": { "type": "string", "description": "Standard error" },
                "exit_code": { "type": "integer", "description": "Exit code" }
            }
        }),
    }
}

// ============================================
// Plan Agent Tools
// ============================================

pub fn plan_agent_tools() -> Vec<ToolDefinition> {
    vec![
        create_read_file_tool(),
        create_run_command_tool(),
        // Plan-specific tools will be added here
        ToolDefinition {
            name: "parsePlan".to_string(),
            description: "Parse and analyze a plan document".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to the plan file" }
                },
                "required": ["path"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "tasks": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "title": { "type": "string" },
                                "status": { "type": "string" }
                            }
                        }
                    }
                }
            }),
        },
    ]
}

// ============================================
// Code Agent Tools
// ============================================

pub fn code_agent_tools() -> Vec<ToolDefinition> {
    vec![
        create_read_file_tool(),
        create_write_file_tool(),
        create_edit_file_tool(),
        create_delete_file_tool(),
        create_run_command_tool(),
        // Code-specific tools will be added here
        ToolDefinition {
            name: "analyzeCode".to_string(),
            description: "Analyze code for patterns, smells, and improvements".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to the code file or directory" }
                },
                "required": ["path"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "issues": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": { "type": "string" },
                                "severity": { "type": "string" },
                                "message": { "type": "string" },
                                "location": {
                                    "type": "object",
                                    "properties": {
                                        "path": { "type": "string" },
                                        "line": { "type": "integer" }
                                    }
                                }
                            }
                        }
                    }
                }
            }),
        },
    ]
}

// ============================================
// Review Agent Tools
// ============================================

pub fn review_agent_tools() -> Vec<ToolDefinition> {
    vec![
        create_read_file_tool(),
        create_run_command_tool(),
        // Review-specific tools will be added here
        ToolDefinition {
            name: "reviewCode".to_string(),
            description: "Review code changes for quality and correctness".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to the code file or directory" },
                    "branch": { "type": "string", "description": "Branch to review" }
                },
                "required": ["path"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "comments": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": { "type": "string" },
                                "severity": { "type": "string" },
                                "message": { "type": "string" },
                                "location": {
                                    "type": "object",
                                    "properties": {
                                        "path": { "type": "string" },
                                        "line": { "type": "integer" }
                                    }
                                }
                            }
                        }
                    }
                }
            }),
        },
    ]
}

// ============================================
// Ask Agent Tools
// ============================================

pub fn ask_agent_tools() -> Vec<ToolDefinition> {
    vec![
        create_read_file_tool(),
        create_run_command_tool(),
        // Ask-specific tools will be added here
        ToolDefinition {
            name: "searchDocumentation".to_string(),
            description: "Search documentation and provide answers to questions".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "context": { "type": "string", "description": "Additional context" }
                },
                "required": ["query"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "answer": { "type": "string", "description": "Answer to the question" },
                    "sources": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Sources used for the answer"
                    }
                }
            }),
        },
    ]
}

// ============================================
// Orchestrator Agent Tools
// ============================================

pub fn orchestrator_agent_tools() -> Vec<ToolDefinition> {
    vec![
        create_read_file_tool(),
        create_run_command_tool(),
        // Orchestrator-specific tools will be added here
        ToolDefinition {
            name: "manageTasks".to_string(),
            description: "Manage task execution and dependencies".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": { "type": "string", "description": "Action to perform (start/stop/status)" },
                    "task_id": { "type": "string", "description": "Task ID" }
                },
                "required": ["action"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "success": { "type": "boolean", "description": "Whether the action succeeded" },
                    "result": { "type": "object", "description": "Action result" }
                }
            }),
        },
    ]
}

// ============================================
// Helper Functions
// ============================================

pub fn get_tools_for_agent(agent_type: &str) -> Vec<ToolDefinition> {
    match agent_type {
        "plan" => plan_agent_tools(),
        "code" => code_agent_tools(),
        "review" => review_agent_tools(),
        "ask" => ask_agent_tools(),
        "orchestrator" => orchestrator_agent_tools(),
        _ => vec![create_read_file_tool(), create_run_command_tool()],
    }
}

pub fn validate_agent_file_access(agent_type: &str, path: &str) -> bool {
    // Enforce file access constraints based on agent type
    match agent_type {
        "code" => {
            // Code agent constraints: can only modify certain files
            path.ends_with(".rs")
                || path.ends_with(".toml")
                || path.ends_with(".json")
                || path.ends_with(".md")
        }
        "plan" => {
            // Plan agent constraints: can only read plan files
            path.ends_with(".md")
                && (path.contains("/plans/")
                    || path.contains("plans\\")
                    || path.starts_with("plans/"))
        }
        "review" => {
            // Review agent constraints: limited file access
            path.ends_with(".rs") || path.ends_with(".md")
        }
        "ask" => {
            // Ask agent constraints: read-only access to documentation files
            path.ends_with(".md")
        }
        "orchestrator" => {
            // Orchestrator agent constraints: broader access
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tools_for_agent() {
        let plan_tools = get_tools_for_agent("plan");
        assert!(!plan_tools.is_empty());
        assert!(plan_tools.iter().any(|t| t.name == "parsePlan"));

        let code_tools = get_tools_for_agent("code");
        assert!(!code_tools.is_empty());
        assert!(code_tools.iter().any(|t| t.name == "analyzeCode"));

        let review_tools = get_tools_for_agent("review");
        assert!(!review_tools.is_empty());
        assert!(review_tools.iter().any(|t| t.name == "reviewCode"));

        let ask_tools = get_tools_for_agent("ask");
        assert!(!ask_tools.is_empty());
        assert!(ask_tools.iter().any(|t| t.name == "searchDocumentation"));

        let orchestrator_tools = get_tools_for_agent("orchestrator");
        assert!(!orchestrator_tools.is_empty());
        assert!(orchestrator_tools.iter().any(|t| t.name == "manageTasks"));
    }

    #[test]
    fn test_validate_agent_file_access() {
        // Code agent should allow Rust and config files
        assert!(validate_agent_file_access("code", "src/main.rs"));
        assert!(validate_agent_file_access("code", "Cargo.toml"));
        assert!(!validate_agent_file_access("code", "secret.txt"));

        // Plan agent should only allow plan files
        assert!(validate_agent_file_access(
            "plan",
            "plans/00-master-plan.md"
        ));
        assert!(!validate_agent_file_access("plan", "src/main.rs"));

        // Review agent should allow code and documentation files
        assert!(validate_agent_file_access("review", "src/main.rs"));
        assert!(validate_agent_file_access("review", "README.md"));
        assert!(!validate_agent_file_access("review", "secret.txt"));

        // Ask agent should only allow documentation files
        assert!(validate_agent_file_access("ask", "README.md"));
        assert!(!validate_agent_file_access("ask", "src/main.rs"));

        // Orchestrator agent should allow all files
        assert!(validate_agent_file_access("orchestrator", "src/main.rs"));
        assert!(validate_agent_file_access("orchestrator", "secret.txt"));
    }

    #[test]
    fn test_file_tool_definitions() {
        let read_tool = create_read_file_tool();
        assert_eq!(read_tool.name, "readFile");
        assert!(!read_tool.description.is_empty());

        let write_tool = create_write_file_tool();
        assert_eq!(write_tool.name, "writeFile");
        assert!(!write_tool.description.is_empty());

        let edit_tool = create_edit_file_tool();
        assert_eq!(edit_tool.name, "editFile");
        assert!(!edit_tool.description.is_empty());

        let delete_tool = create_delete_file_tool();
        assert_eq!(delete_tool.name, "deleteFile");
        assert!(!delete_tool.description.is_empty());

        let run_tool = create_run_command_tool();
        assert_eq!(run_tool.name, "runCommand");
        assert!(!run_tool.description.is_empty());
    }
}
