// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

#![allow(missing_docs)]

use thiserror::Error;

/// Errors that can occur in Telisq.
#[derive(Error, Debug)]
pub enum TelisqError {
    /// Error from LLM interaction.
    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),

    /// Error from MCP server interaction.
    #[error("MCP error: {0}")]
    Mcp(#[from] McpError),

    /// Error from parsing.
    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),

    /// Error from configuration.
    #[error("Config error: {0}")]
    Config(#[from] ConfigError),

    /// Error from session management.
    #[error("Session error: {0}")]
    Session(#[from] SessionError),

    /// Error from file guard.
    #[error("File guard error: {0}")]
    FileGuard(#[from] FileGuardError),

    /// Error from other sources.
    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

/// Alias for backwards compatibility.
pub type AgentError = TelisqError;

/// Result type for Telisq operations.
pub type Result<T = (), E = TelisqError> = std::result::Result<T, E>;

/// Errors from LLM interaction.
#[derive(Error, Debug)]
pub enum LlmError {
    /// Failed to connect to LLM API.
    #[error("Failed to connect to LLM API: {0}")]
    ConnectionError(String),

    /// Failed to parse LLM response.
    #[error("Failed to parse LLM response: {0}")]
    ParseError(String),

    /// LLM returned an error.
    #[error("LLM returned an error: {0}")]
    ApiError(String),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded")]
    RateLimit,

    /// Invalid API key.
    #[error("Invalid API key")]
    InvalidApiKey,
}

/// Errors from MCP server interaction.
#[derive(Error, Debug)]
pub enum McpError {
    /// Failed to connect to MCP server.
    #[error("Failed to connect to MCP server: {0}")]
    ConnectionError(String),

    /// Failed to parse MCP response.
    #[error("Failed to parse MCP response: {0}")]
    ParseError(String),

    /// MCP returned an error.
    #[error("MCP returned an error: {0}")]
    ApiError(String),

    /// Method not found.
    #[error("Method not found: {0}")]
    MethodNotFound(String),

    /// Invalid parameters.
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),
}

/// Errors from parsing.
#[derive(Error, Debug)]
pub enum ParseError {
    /// Invalid syntax.
    #[error("Invalid syntax at line {line}: {message}")]
    SyntaxError { line: u32, message: String },

    /// Duplicate identifier.
    #[error("Duplicate identifier at line {line}: {id}")]
    DuplicateId { line: u32, id: String },

    /// Missing required field.
    #[error("Missing required field at line {line}: {field}")]
    MissingField { line: u32, field: String },

    /// Invalid field value.
    #[error("Invalid field value at line {line}: {field} = {value}")]
    InvalidFieldValue {
        line: u32,
        field: String,
        value: String,
    },

    /// Invalid plan structure.
    #[error("Invalid plan structure at line {line}: {message}")]
    InvalidStructure { line: u32, message: String },
}

/// Errors from configuration.
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Failed to load config file.
    #[error("Failed to load config file: {0}")]
    LoadError(String),

    /// Failed to parse config file.
    #[error("Failed to parse config file: {0}")]
    ParseError(String),

    /// Invalid config value.
    #[error("Invalid config value for {key}: {value}")]
    InvalidValue { key: String, value: String },

    /// Missing required config field.
    #[error("Missing required config field: {0}")]
    MissingField(String),
}

/// Errors from session management.
#[derive(Error, Debug)]
pub enum SessionError {
    /// Failed to create session.
    #[error("Failed to create session: {0}")]
    CreateError(String),

    /// Failed to load session.
    #[error("Failed to load session: {0}")]
    LoadError(String),

    /// Failed to save session.
    #[error("Failed to save session: {0}")]
    SaveError(String),

    /// Session not found.
    #[error("Session not found: {0}")]
    NotFound(String),

    /// Session is invalid.
    #[error("Session is invalid: {0}")]
    Invalid(String),
}

/// Errors from file guard.
#[derive(Error, Debug)]
pub enum FileGuardError {
    /// File is already modified.
    #[error("File is already modified: {0}")]
    AlreadyModified(String),

    /// Failed to lock file.
    #[error("Failed to lock file: {0}")]
    LockError(String),

    /// Failed to unlock file.
    #[error("Failed to unlock file: {0}")]
    UnlockError(String),
}
