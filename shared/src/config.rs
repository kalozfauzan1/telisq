// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::errors::ConfigError;

/// Default configuration file name in user's home directory.
const DEFAULT_CONFIG_FILE: &str = ".telisq/config.yaml";

/// Project override configuration file name.
const PROJECT_CONFIG_FILE: &str = ".telisq.toml";

/// LLM configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmConfig {
    /// API key for the LLM.
    pub api_key: String,
    /// Base URL for the LLM API.
    pub base_url: String,
    /// Model to use.
    pub model: String,
    /// Temperature for generation.
    pub temperature: f64,
    /// Max tokens to generate.
    pub max_tokens: u32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            api_key: "".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o".to_string(),
            temperature: 0.1,
            max_tokens: 4096,
        }
    }
}

/// Individual MCP server configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name for identification.
    pub name: String,
    /// Command to spawn the server.
    pub command: String,
    /// Arguments to pass to the server command.
    pub args: Vec<String>,
}

/// MCP configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpConfig {
    /// List of configured MCP servers.
    pub servers: Vec<McpServerConfig>,
    /// Timeout for requests.
    pub timeout: u64,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            command: "".to_string(),
            args: Vec::new(),
        }
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            timeout: 30,
        }
    }
}

/// Agent configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Default agent to use.
    pub default: String,
    /// List of available agents.
    pub agents: Vec<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            default: "default".to_string(),
            agents: vec!["default".to_string()],
        }
    }
}

/// Index configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexConfig {
    /// Path to the index directory.
    pub path: String,
    /// Whether to enable auto-indexing.
    pub auto_index: bool,
    /// Interval between index updates (in seconds).
    pub update_interval: u64,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            path: "~/.telisq/index".to_string(),
            auto_index: true,
            update_interval: 3600,
        }
    }
}

/// Application configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AppConfig {
    /// LLM configuration.
    pub llm: LlmConfig,
    /// MCP configuration.
    pub mcp: McpConfig,
    /// Agent configuration.
    pub agent: AgentConfig,
    /// Index configuration.
    pub index: IndexConfig,
}

/// Returns the path to the user config file.
pub fn config_path() -> Result<PathBuf, ConfigError> {
    let home_dir = home::home_dir()
        .ok_or_else(|| ConfigError::LoadError("Home directory not found".into()))?;
    Ok(home_dir.join(DEFAULT_CONFIG_FILE))
}

impl AppConfig {
    /// Saves the configuration to the user config file.
    pub fn save(&self) -> Result<(), ConfigError> {
        let config_path = config_path()?;

        // Create directories if they don't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ConfigError::LoadError(format!("Failed to create config directory: {}", e))
            })?;
        }

        let content = serde_yaml::to_string(self)
            .map_err(|e| ConfigError::ParseError(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(&config_path, content)
            .map_err(|e| ConfigError::LoadError(format!("Failed to write config file: {}", e)))?;

        Ok(())
    }

    /// Loads the application configuration, merging defaults, user config, and project config.
    pub fn load() -> Result<Self, ConfigError> {
        let mut config = AppConfig::default();

        // Load user configuration
        if let Ok(user_config) = Self::load_user_config() {
            config.merge(user_config);
        }

        // Load project configuration
        if let Ok(project_config) = Self::load_project_config() {
            config.merge(project_config);
        }

        // Interpolate environment variables
        config.interpolate_env()?;

        Ok(config)
    }

    /// Loads the user configuration from ~/.telisq/config.yaml.
    fn load_user_config() -> Result<Self, ConfigError> {
        let home_dir = home::home_dir()
            .ok_or_else(|| ConfigError::LoadError("Home directory not found".into()))?;
        let config_path = home_dir.join(DEFAULT_CONFIG_FILE);

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| ConfigError::LoadError(format!("Failed to read user config: {}", e)))?;

        let config: Self = serde_yaml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(format!("Failed to parse user config: {}", e)))?;

        Ok(config)
    }

    /// Loads the project configuration from .telisq.toml.
    fn load_project_config() -> Result<Self, ConfigError> {
        let current_dir = std::env::current_dir().map_err(|e| {
            ConfigError::LoadError(format!("Failed to get current directory: {}", e))
        })?;
        let config_path = current_dir.join(PROJECT_CONFIG_FILE);

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)
            .map_err(|e| ConfigError::LoadError(format!("Failed to read project config: {}", e)))?;

        let config: Self = toml::from_str(&content).map_err(|e| {
            ConfigError::ParseError(format!("Failed to parse project config: {}", e))
        })?;

        Ok(config)
    }

    /// Merges another config into this one.
    fn merge(&mut self, other: Self) {
        // Merge LLM config
        if !other.llm.api_key.is_empty() {
            self.llm.api_key = other.llm.api_key;
        }
        if !other.llm.base_url.is_empty() {
            self.llm.base_url = other.llm.base_url;
        }
        if !other.llm.model.is_empty() {
            self.llm.model = other.llm.model;
        }
        if other.llm.temperature != 0.0 {
            self.llm.temperature = other.llm.temperature;
        }
        if other.llm.max_tokens != 0 {
            self.llm.max_tokens = other.llm.max_tokens;
        }

        // Merge MCP config
        if !other.mcp.servers.is_empty() {
            self.mcp.servers = other.mcp.servers;
        }
        if other.mcp.timeout != 0 {
            self.mcp.timeout = other.mcp.timeout;
        }

        // Merge agent config
        if !other.agent.default.is_empty() {
            self.agent.default = other.agent.default;
        }
        if !other.agent.agents.is_empty() {
            self.agent.agents = other.agent.agents;
        }

        // Merge index config
        if !other.index.path.is_empty() {
            self.index.path = other.index.path;
        }
        if other.index.auto_index {
            self.index.auto_index = other.index.auto_index;
        }
        if other.index.update_interval != 0 {
            self.index.update_interval = other.index.update_interval;
        }
    }

    /// Interpolates environment variables in the configuration.
    fn interpolate_env(&mut self) -> Result<(), ConfigError> {
        // Interpolate LLM config
        self.llm.api_key = Self::interpolate_string(&self.llm.api_key)?;
        self.llm.base_url = Self::interpolate_string(&self.llm.base_url)?;
        self.llm.model = Self::interpolate_string(&self.llm.model)?;

        // Interpolate MCP config
        self.mcp.servers = self
            .mcp
            .servers
            .iter()
            .map(|s| {
                let mut server = s.clone();
                server.name = Self::interpolate_string(&server.name)?;
                server.command = Self::interpolate_string(&server.command)?;
                server.args = server
                    .args
                    .iter()
                    .map(|arg| Self::interpolate_string(arg))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(server)
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Interpolate agent config
        self.agent.default = Self::interpolate_string(&self.agent.default)?;
        self.agent.agents = self
            .agent
            .agents
            .iter()
            .map(|s| Self::interpolate_string(s))
            .collect::<Result<Vec<_>, _>>()?;

        // Interpolate index config
        self.index.path = Self::interpolate_string(&self.index.path)?;

        Ok(())
    }

    /// Interpolates environment variables in a string.
    fn interpolate_string(s: &str) -> Result<String, ConfigError> {
        let mut result = String::new();
        let mut iter = s.chars().peekable();

        while let Some(c) = iter.next() {
            if c == '$' && iter.peek() == Some(&'{') {
                // Skip $ and {
                iter.next();

                let mut var_name = String::new();
                for c in iter.by_ref() {
                    if c == '}' {
                        break;
                    }
                    var_name.push(c);
                }

                // Get the environment variable
                match std::env::var(&var_name) {
                    Ok(value) => result.push_str(&value),
                    Err(_) => {
                        return Err(ConfigError::MissingField(format!(
                            "Environment variable '{}' not found",
                            var_name
                        )))
                    }
                }
            } else {
                result.push(c);
            }
        }

        Ok(result)
    }
}
