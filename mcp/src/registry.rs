//! MCP server registry and lifecycle management.
//!
//! This module implements the MCP registry, which manages the lifecycle of all
//! configured MCP servers, tracks their availability, and handles dispatch of
//! tool calls to available servers.

use crate::protocol::ToolDefinition;
use crate::server::{McpServer, McpServerError};
use shared::config::McpServerConfig;
use shared::errors::TelisqError;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use tracing::*;

#[derive(Error, Debug)]
pub enum McpRegistryError {
    #[error("Server not found: {0}")]
    ServerNotFound(String),

    #[error("Failed to dispatch tool call: {0}")]
    DispatchFailed(String),

    #[error("MCP server error: {0}")]
    ServerError(#[from] McpServerError),

    #[error("No available server provides tool '{0}'")]
    ToolNotAvailable(String),
}

impl From<McpRegistryError> for TelisqError {
    fn from(e: McpRegistryError) -> Self {
        TelisqError::Mcp(shared::errors::McpError::ConnectionError(e.to_string()))
    }
}

struct ServerState {
    server: McpServer,
    failed_attempts: usize,
    is_degraded: bool,
}

pub struct McpRegistry {
    config: Vec<McpServerConfig>,
    servers: RwLock<HashMap<String, Arc<Mutex<ServerState>>>>,
    tool_index: RwLock<HashMap<String, Vec<String>>>, // tool name -> server names
    max_respawn_attempts: usize,
}

impl McpRegistry {
    /// Creates a new MCP registry from configuration.
    pub fn new(config: Vec<McpServerConfig>) -> Self {
        Self {
            config,
            servers: RwLock::new(HashMap::new()),
            tool_index: RwLock::new(HashMap::new()),
            max_respawn_attempts: 1, // One respawn attempt on failure
        }
    }

    /// Starts all configured MCP servers.
    /// Degrades gracefully if servers fail to start.
    pub async fn start_all(&self) -> Vec<String> {
        let mut failed_servers = Vec::new();

        for server_config in &self.config {
            match self.start_server(server_config.clone()).await {
                Ok(_) => info!(name = %server_config.name, "MCP server started successfully"),
                Err(e) => {
                    error!(
                        name = %server_config.name,
                        error = %e,
                        "Failed to start MCP server"
                    );
                    failed_servers.push(server_config.name.clone());
                }
            }
        }

        failed_servers
    }

    async fn start_server(&self, config: McpServerConfig) -> Result<(), McpServerError> {
        let server = McpServer::spawn(
            config.name.clone(),
            config.command.clone(),
            config.args.clone(),
        )
        .await?;

        let state = Arc::new(Mutex::new(ServerState {
            server,
            failed_attempts: 0,
            is_degraded: false,
        }));

        // Add to servers map
        let mut servers = self.servers.write().await;
        servers.insert(config.name.clone(), state.clone());

        // Update tool index
        let mut tool_index = self.tool_index.write().await;
        let server_state = state.lock().await;
        for tool in server_state.server.available_tools() {
            tool_index
                .entry(tool.name.clone())
                .or_insert_with(Vec::new)
                .push(config.name.clone());
        }

        Ok(())
    }

    /// Returns all available tool names from all servers.
    pub async fn available_tools(&self) -> Vec<String> {
        self.tool_index.read().await.keys().cloned().collect()
    }

    /// Returns tool definitions for all available tools.
    pub async fn tool_definitions(&self) -> Vec<ToolDefinition> {
        let servers = self.servers.read().await;
        let mut definitions = Vec::new();

        for (_, state) in servers.iter() {
            let server_state = state.lock().await;
            if !server_state.is_degraded {
                definitions.extend(server_state.server.capabilities().tools.clone());
            }
        }

        definitions
    }

    /// Returns tool definitions from available servers for a specific agent type.
    pub async fn tool_definitions_for_agent(&self, agent_type: &str) -> Vec<ToolDefinition> {
        let all_definitions = self.tool_definitions().await;

        // Filter tools based on agent type
        all_definitions
            .into_iter()
            .filter(|tool| self.is_tool_available_for_agent(tool, agent_type))
            .collect()
    }

    /// Dispatches a tool call to an available server.
    pub async fn dispatch_tool_call(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpRegistryError> {
        let server_names = {
            let tool_index = self.tool_index.read().await;
            tool_index
                .get(tool_name)
                .ok_or_else(|| McpRegistryError::ToolNotAvailable(tool_name.to_string()))?
                .clone()
        };

        // Try each server that provides this tool
        for server_name in server_names {
            let state = {
                let servers = self.servers.read().await;
                match servers.get(&server_name) {
                    Some(state) => state.clone(),
                    None => continue, // Server might have been removed
                }
            };

            {
                let mut server_state = state.lock().await;
                if server_state.is_degraded {
                    continue; // Skip degraded servers
                }

                // Call the tool
                match server_state
                    .server
                    .call_tool(tool_name, arguments.clone())
                    .await
                {
                    Ok(result) => return Ok(result.content),
                    Err(e) => {
                        error!(
                            tool = %tool_name,
                            server = %server_name,
                            error = %e,
                            "Tool call failed on server"
                        );
                        // Handle failure inline
                        server_state.failed_attempts += 1;
                        if server_state.failed_attempts <= self.max_respawn_attempts {
                            info!(
                                server = %server_name,
                                attempts = %server_state.failed_attempts,
                                max = %self.max_respawn_attempts,
                                "Attempting to respawn server"
                            );
                            drop(server_state); // Release lock before respawn
                            if let Err(e) = self.respawn_server(&server_name).await {
                                error!(server = %server_name, error = %e, "Server respawn failed");
                                // Mark as degraded if respawn fails
                                let mut state = state.lock().await;
                                state.is_degraded = true;
                            }
                        } else {
                            warn!(
                                server = %server_name,
                                "Server failed too many times, marking as degraded"
                            );
                            server_state.is_degraded = true;
                        }
                    }
                }
            }
        }

        Err(McpRegistryError::DispatchFailed(format!(
            "No available server could handle tool '{}'",
            tool_name
        )))
    }

    async fn respawn_server(&self, server_name: &str) -> Result<(), McpServerError> {
        // Find the server configuration
        let config = match self.config.iter().find(|c| c.name == server_name) {
            Some(config) => config.clone(),
            None => {
                error!(server = %server_name, "Server configuration not found for respawn");
                return Err(McpServerError::CommunicationFailed(
                    "Server configuration not found".to_string(),
                ));
            }
        };

        // Kill existing process and attempt to respawn
        let servers = self.servers.read().await;
        let state = match servers.get(server_name) {
            Some(state) => state.clone(),
            None => {
                error!(server = %server_name, "Server state not found for respawn");
                return Err(McpServerError::CommunicationFailed(
                    "Server state not found".to_string(),
                ));
            }
        };
        drop(servers);

        {
            let mut server_state = state.lock().await;
            let _ = server_state.server.kill().await;

            match McpServer::spawn(
                config.name.clone(),
                config.command.clone(),
                config.args.clone(),
            )
            .await
            {
                Ok(new_server) => {
                    info!(server = %server_name, "Server respawned successfully");
                    server_state.server = new_server;
                    server_state.failed_attempts = 0;
                    server_state.is_degraded = false;
                    Ok(())
                }
                Err(e) => {
                    error!(server = %server_name, error = %e, "Server respawn failed");
                    server_state.is_degraded = true;
                    Err(e)
                }
            }
        }
    }

    /// Checks if a server is running and available.
    pub async fn is_server_available(&self, server_name: &str) -> bool {
        let servers = self.servers.read().await;
        match servers.get(server_name) {
            Some(state) => {
                let mut server_state = state.lock().await;
                !server_state.is_degraded && server_state.server.is_running().await
            }
            None => false,
        }
    }

    /// Returns names of all available servers.
    pub async fn available_servers(&self) -> Vec<String> {
        let servers = self.servers.read().await;
        let mut available = Vec::new();

        for (name, state) in servers.iter() {
            let mut server_state = state.lock().await;
            if !server_state.is_degraded && server_state.server.is_running().await {
                available.push(name.clone());
            }
        }

        available
    }

    /// Stops all servers and clears the registry.
    pub async fn shutdown(&self) {
        info!("Shutting down all MCP servers");
        let servers = self.servers.read().await;

        for (name, state) in servers.iter() {
            let mut server_state = state.lock().await;
            if let Err(e) = server_state.server.kill().await {
                error!(server = %name, error = %e, "Failed to kill server");
            }
        }
        drop(servers);

        self.servers.write().await.clear();
        self.tool_index.write().await.clear();
        info!("All MCP servers shutdown");
    }

    /// Helper function to check if a tool is available for a specific agent type.
    /// This will be implemented based on actual tool categorization.
    fn is_tool_available_for_agent(&self, _tool: &ToolDefinition, _agent_type: &str) -> bool {
        // For now, return all tools as available for all agents
        // This will be refined with actual agent tool constraints
        true
    }
}

impl Drop for McpRegistry {
    fn drop(&mut self) {
        // We can't use async code in drop, so we just log
        info!("MCP registry is being dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let config = vec![
            McpServerConfig {
                name: "server1".to_string(),
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
            },
            McpServerConfig {
                name: "server2".to_string(),
                command: "echo".to_string(),
                args: vec!["world".to_string()],
            },
        ];

        let registry = McpRegistry::new(config);
        assert_eq!(registry.config.len(), 2);
    }

    #[tokio::test]
    async fn test_start_all() {
        let config = vec![McpServerConfig {
            name: "invalid-server".to_string(),
            command: "invalid-command-that-does-not-exist".to_string(),
            args: vec![],
        }];

        let registry = McpRegistry::new(config);
        let failed_servers = registry.start_all().await;
        assert_eq!(failed_servers.len(), 1);
        assert_eq!(failed_servers[0], "invalid-server");
    }
}
