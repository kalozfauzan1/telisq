//! MCP server process management and communication.
//!
//! This module handles spawning MCP server processes, managing stdio pipes,
//! and performing the JSON-RPC communication with servers.

use crate::protocol::*;
use serde_json::Value;
use shared::errors::TelisqError;
use std::collections::HashMap;
use std::process::Stdio;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tracing::*;

#[derive(Error, Debug)]
pub enum McpServerError {
    #[error("Process failed to start: {0}")]
    ProcessStartFailed(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    ProtocolError(#[from] ProtocolError),

    #[error("Server communication failed: {0}")]
    CommunicationFailed(String),

    #[error("Server process died unexpectedly")]
    ProcessDied,

    #[error("Response timeout")]
    Timeout,
}

impl From<McpServerError> for TelisqError {
    fn from(e: McpServerError) -> Self {
        TelisqError::Mcp(shared::errors::McpError::ConnectionError(e.to_string()))
    }
}

pub struct McpServer {
    name: String,
    child: Child,
    _response_senders:
        HashMap<String, mpsc::UnboundedSender<Result<JsonRpcResponse, McpServerError>>>,
    response_receiver: mpsc::UnboundedReceiver<String>,
    capabilities: Capabilities,
}

impl McpServer {
    /// Spawns a new MCP server process and performs the initialize handshake.
    pub async fn spawn(
        name: String,
        command: String,
        args: Vec<String>,
    ) -> Result<Self, McpServerError> {
        info!(%name, command, args = ?args, "Spawning MCP server process");

        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(McpServerError::ProcessStartFailed)?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpServerError::CommunicationFailed("No stdout pipe".to_string()))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpServerError::CommunicationFailed("No stdin pipe".to_string()))?;

        let (tx, mut rx) = mpsc::unbounded_channel::<String>();

        // Start reading from stdout in a background task
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Err(e) = tx.send(line) {
                    error!("Failed to send line: {}", e);
                    break;
                }
            }
        });

        // Perform initialize handshake
        let initialize_req = create_initialize_request();
        let mut initialize_json =
            serde_json::to_string(&initialize_req).map_err(ProtocolError::from)?;
        initialize_json.push('\n');
        stdin
            .write_all(initialize_json.as_bytes())
            .await
            .map_err(McpServerError::ProcessStartFailed)?;

        // Wait for response
        let response_line = rx.recv().await.ok_or(McpServerError::ProcessDied)?;
        let response: JsonRpcResponse =
            serde_json::from_str(&response_line).map_err(ProtocolError::from)?;

        let initialize_result: InitializeResult =
            serde_json::from_value(response.result).map_err(ProtocolError::from)?;

        // Validate protocol version
        if initialize_result.server_info.version != MCP_PROTOCOL_VERSION {
            return Err(McpServerError::ProtocolError(
                ProtocolError::VersionMismatch {
                    expected: MCP_PROTOCOL_VERSION.to_string(),
                    actual: initialize_result.server_info.version,
                },
            ));
        }

        info!(
            %name,
            server_name = %initialize_result.server_info.name,
            server_version = %initialize_result.server_info.version,
            tool_count = %initialize_result.capabilities.tools.len(),
            "MCP server initialized successfully"
        );

        Ok(Self {
            name,
            child,
            _response_senders: HashMap::new(),
            response_receiver: rx,
            capabilities: initialize_result.capabilities,
        })
    }

    /// Returns the server name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the server's capabilities
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// Returns available tools from the server
    pub fn available_tools(&self) -> Vec<&ToolDefinition> {
        self.capabilities.tools.iter().collect()
    }

    /// Calls a tool on the server
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<ToolCallResult, McpServerError> {
        // Check if tool exists
        if !self.capabilities.tools.iter().any(|t| t.name == tool_name) {
            return Err(McpServerError::ProtocolError(ProtocolError::ToolNotFound(
                tool_name.to_string(),
            )));
        }

        let req = create_tool_call_request(tool_name, arguments);
        let req_json = serde_json::to_string(&req).map_err(ProtocolError::from)?;

        // Get stdin pipe
        let stdin = self
            .child
            .stdin
            .as_mut()
            .ok_or_else(|| McpServerError::CommunicationFailed("stdin closed".to_string()))?;

        // Send request
        let mut req_json = req_json;
        req_json.push('\n');
        stdin
            .write_all(req_json.as_bytes())
            .await
            .map_err(McpServerError::ProcessStartFailed)?;

        // Wait for response
        let response_line = self
            .response_receiver
            .recv()
            .await
            .ok_or(McpServerError::ProcessDied)?;

        let response: JsonRpcResponse =
            serde_json::from_str(&response_line).map_err(ProtocolError::from)?;

        let result: ToolCallResult =
            serde_json::from_value(response.result).map_err(ProtocolError::from)?;

        Ok(result)
    }

    /// Checks if the server process is still running
    pub async fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(status)) => {
                info!(name = %self.name, status = %status, "MCP server process exited");
                false
            }
            Ok(None) => true,
            Err(e) => {
                error!(name = %self.name, error = %e, "Failed to check process status");
                false
            }
        }
    }

    /// Kills the server process
    pub async fn kill(&mut self) -> Result<(), McpServerError> {
        if let Err(e) = self.child.kill().await {
            error!(name = %self.name, error = %e, "Failed to kill process");
            return Err(McpServerError::ProcessStartFailed(e));
        }
        Ok(())
    }
}

impl Drop for McpServer {
    fn drop(&mut self) {
        // Attempt to kill the child process if it's still running
        std::mem::drop(self.child.kill());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spawn_server() {
        // This test would require a test MCP server implementation.
        // Kept as a placeholder until a deterministic test server fixture is added.
    }

    #[test]
    fn test_available_tools() {
        // Test capabilities directly without McpServer struct
        // (McpServer requires a real child process which is hard to mock)
        let capabilities = Capabilities {
            tools: vec![
                ToolDefinition {
                    name: "tool1".to_string(),
                    description: "Test tool 1".to_string(),
                    input_schema: serde_json::json!({}),
                    output_schema: serde_json::json!({}),
                },
                ToolDefinition {
                    name: "tool2".to_string(),
                    description: "Test tool 2".to_string(),
                    input_schema: serde_json::json!({}),
                    output_schema: serde_json::json!({}),
                },
            ],
        };

        let tools: Vec<&ToolDefinition> = capabilities.tools.iter().collect();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "tool1");
        assert_eq!(tools[1].name, "tool2");
    }
}
