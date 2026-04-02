//! MCP JSON-RPC protocol definitions and serialization.
//!
//! This module defines the JSON-RPC 2.0 protocol used for communication between
//! Telisq and MCP servers. It includes types for requests, responses, tool calls,
//! and the initialize handshake.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

// MCP Protocol Version
pub const MCP_PROTOCOL_VERSION: &str = "2023-11-27";

// ============================================
// JSON-RPC Base Types
// ============================================

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: String,
    pub result: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcErrorResponse {
    pub jsonrpc: String,
    pub id: String,
    pub error: JsonRpcError,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

// ============================================
// Initialize Handshake
// ============================================

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub server_info: ServerInfo,
    pub capabilities: Capabilities,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub tools: Vec<ToolDefinition>,
}

// ============================================
// Tool Types
// ============================================

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,  // JSON Schema
    pub output_schema: Value, // JSON Schema
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallParams {
    pub tool_call: ToolCall,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    pub content: Value,
}

// ============================================
// Protocol Errors
// ============================================

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("JSON serialization failed: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Protocol version mismatch. Expected {expected}, got {actual}")]
    VersionMismatch { expected: String, actual: String },

    #[error("Invalid JSON-RPC message format")]
    InvalidMessageFormat,

    #[error("Server returned error: {code} - {message}")]
    ServerError { code: i32, message: String },

    #[error("Initialize handshake failed: {0}")]
    InitializeFailed(String),

    #[error("Tool '{0}' not found")]
    ToolNotFound(String),

    #[error("Tool call failed: {0}")]
    ToolCallFailed(String),
}

// ============================================
// Protocol Helper Functions
// ============================================

pub fn generate_request_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn create_initialize_request() -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: generate_request_id(),
        method: "initialize".to_string(),
        params: serde_json::to_value(InitializeParams {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
        })
        .unwrap(),
    }
}

pub fn create_tool_call_request(tool_name: &str, arguments: Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: generate_request_id(),
        method: "toolCall".to_string(),
        params: serde_json::to_value(ToolCallParams {
            tool_call: ToolCall {
                name: tool_name.to_string(),
                arguments,
            },
        })
        .unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_request_id() {
        let id1 = generate_request_id();
        let id2 = generate_request_id();
        assert_ne!(id1, id2);
        assert!(uuid::Uuid::parse_str(&id1).is_ok());
    }

    #[test]
    fn test_create_initialize_request() {
        let req = create_initialize_request();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "initialize");

        let params: InitializeParams = serde_json::from_value(req.params).unwrap();
        assert_eq!(params.protocol_version, MCP_PROTOCOL_VERSION);
    }

    #[test]
    fn test_create_tool_call_request() {
        let args = serde_json::json!({ "key": "value" });
        let req = create_tool_call_request("testTool", args.clone());

        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "toolCall");

        let params: ToolCallParams = serde_json::from_value(req.params).unwrap();
        assert_eq!(params.tool_call.name, "testTool");
        assert_eq!(params.tool_call.arguments, args);
    }
}
