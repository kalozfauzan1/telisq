//! Tool calling dispatch and execution.
//!
//! This module provides functions for dispatching tool calls from LLM responses
//! to MCP servers and handling the results.

use crate::llm::types::ToolCall;
use mcp::registry::McpRegistry;
use serde_json::Value;
use shared::errors::{LlmError, TelisqError};
use tracing::*;

/// Executes a single tool call by dispatching to the MCP registry.
///
/// # Arguments
/// * `tool_call` - The tool call to execute
/// * `registry` - The MCP registry to dispatch to
///
/// # Returns
/// The tool execution result as a string, or an error message if execution failed.
pub async fn execute_tool_call(
    tool_call: &ToolCall,
    registry: &McpRegistry,
) -> Result<String, TelisqError> {
    let tool_name = &tool_call.function.name;
    let arguments_str = &tool_call.function.arguments;

    info!(
        tool = %tool_name,
        arguments = %arguments_str,
        "Executing tool call"
    );

    // Parse arguments as JSON
    let arguments: Value = serde_json::from_str(arguments_str).map_err(|e| {
        LlmError::ParseError(format!(
            "Failed to parse tool call arguments for '{}': {}",
            tool_name, e
        ))
    })?;

    // Dispatch to MCP registry
    let result = registry
        .dispatch_tool_call(tool_name, arguments)
        .await
        .map_err(|e| {
            error!(
                tool = %tool_name,
                error = %e,
                "Tool call dispatch failed"
            );
            LlmError::ApiError(format!("Tool call failed for '{}': {}", tool_name, e))
        })?;

    // Convert result to string
    let result_str = serde_json::to_string(&result).map_err(|e| {
        LlmError::ParseError(format!(
            "Failed to serialize tool result for '{}': {}",
            tool_name, e
        ))
    })?;

    info!(
        tool = %tool_name,
        result_length = %result_str.len(),
        "Tool call completed successfully"
    );

    Ok(result_str)
}

/// Executes multiple tool calls and returns their results.
///
/// # Arguments
/// * `tool_calls` - The list of tool calls to execute
/// * `registry` - The MCP registry to dispatch to
///
/// # Returns
/// A vector of (tool_call_id, result_or_error) pairs.
pub async fn execute_tool_calls(
    tool_calls: &[ToolCall],
    registry: &McpRegistry,
) -> Vec<(String, Result<String, TelisqError>)> {
    let mut results = Vec::with_capacity(tool_calls.len());

    for tool_call in tool_calls {
        let id = tool_call.id.clone();
        let result = execute_tool_call(tool_call, registry).await;
        results.push((id, result));
    }

    results
}
