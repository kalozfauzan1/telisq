//! LLM request/response types and models.
//!
//! This module defines the data structures for chat completions, messages,
//! tool calls, and other LLM-related types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ============================================
// Message Types
// ============================================

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Function,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub name: Option<String>,
}

// ============================================
// Tool Types
// ============================================

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub type_: String,
    pub function: FunctionDefinition,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value, // JSON Schema
    pub required: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub type_: String,
    pub function: FunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

// ============================================
// Chat Completion Request
// ============================================

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatCompletionRequest {
    pub messages: Vec<Message>,
    pub tools: Option<Vec<Tool>>,
    pub tool_choice: Option<ToolChoice>,
    pub stream: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ToolChoice {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "required")]
    Required,
    Function(FunctionCallChoice),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCallChoice {
    pub name: String,
}

// ============================================
// Chat Completion Response
// ============================================

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatCompletionChoice>,
    pub usage: Usage,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatCompletionChoice {
    pub index: u32,
    pub message: CompletionMessage,
    pub finish_reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompletionMessage {
    pub role: Role,
    pub content: Option<String>,
    pub function_call: Option<FunctionCall>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ============================================
// Helper Functions
// ============================================

impl ChatCompletionRequest {
    /// Creates a new chat completion request with messages.
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            tools: None,
            tool_choice: None,
            stream: false,
        }
    }

    /// Adds tools to the request.
    pub fn with_tools(mut self, tools: Vec<Tool>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Sets tool choice strategy.
    pub fn with_tool_choice(mut self, tool_choice: ToolChoice) -> Self {
        self.tool_choice = Some(tool_choice);
        self
    }

    /// Enables streaming responses.
    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }
}

impl Message {
    /// Creates a new system message.
    pub fn system(content: &str) -> Self {
        Self {
            role: Role::System,
            content: content.to_string(),
            name: None,
        }
    }

    /// Creates a new user message.
    pub fn user(content: &str) -> Self {
        Self {
            role: Role::User,
            content: content.to_string(),
            name: None,
        }
    }

    /// Creates a new assistant message.
    pub fn assistant(content: &str) -> Self {
        Self {
            role: Role::Assistant,
            content: content.to_string(),
            name: None,
        }
    }

    /// Creates a new function response message.
    pub fn function(name: &str, content: &str) -> Self {
        Self {
            role: Role::Function,
            content: content.to_string(),
            name: Some(name.to_string()),
        }
    }
}

impl Tool {
    /// Creates a new tool definition from an MCP tool definition.
    pub fn from_mcp(tool: &mcp::protocol::ToolDefinition) -> Self {
        Self {
            type_: "function".to_string(),
            function: FunctionDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.input_schema.clone(),
                required: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_message_creation() {
        let system_msg = Message::system("You are a helpful assistant");
        assert_eq!(system_msg.role, Role::System);
        assert!(system_msg.content.contains("helpful assistant"));

        let user_msg = Message::user("Hello!");
        assert_eq!(user_msg.role, Role::User);
        assert_eq!(user_msg.content, "Hello!");

        let assistant_msg = Message::assistant("How can I help you?");
        assert_eq!(assistant_msg.role, Role::Assistant);
        assert_eq!(assistant_msg.content, "How can I help you?");

        let function_msg = Message::function("search", "{\"results\": []}");
        assert_eq!(function_msg.role, Role::Function);
        assert_eq!(function_msg.name, Some("search".to_string()));
    }

    #[test]
    fn test_chat_completion_request_builder() {
        let messages = vec![Message::user("Hello!")];
        let request = ChatCompletionRequest::new(messages).with_stream(true);

        assert_eq!(request.messages.len(), 1);
        assert!(request.stream);
        assert!(request.tools.is_none());
        assert!(request.tool_choice.is_none());
    }

    #[test]
    fn test_tool_from_mcp() {
        let mcp_tool = mcp::protocol::ToolDefinition {
            name: "search".to_string(),
            description: "Search the web".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "results": { "type": "array", "items": { "type": "string" } }
                }
            }),
        };

        let tool = Tool::from_mcp(&mcp_tool);
        assert_eq!(tool.type_, "function");
        assert_eq!(tool.function.name, "search");
        assert_eq!(tool.function.description, "Search the web");
        assert!(tool.function.parameters.is_object());
    }

    #[test]
    fn test_serde_roles() {
        let system_role = Role::System;
        let system_json = serde_json::to_string(&system_role).unwrap();
        assert_eq!(system_json, "\"system\"");

        let user_role = Role::User;
        let user_json = serde_json::to_string(&user_role).unwrap();
        assert_eq!(user_json, "\"user\"");

        let assistant_role = Role::Assistant;
        let assistant_json = serde_json::to_string(&assistant_role).unwrap();
        assert_eq!(assistant_json, "\"assistant\"");

        let function_role = Role::Function;
        let function_json = serde_json::to_string(&function_role).unwrap();
        assert_eq!(function_json, "\"function\"");
    }
}
