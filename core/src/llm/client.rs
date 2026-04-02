//! OpenAI-compatible LLM client implementation.
//!
//! This module provides a client for interacting with OpenAI-compatible LLM APIs,
//! including support for tool calls, SSE streaming, multi-turn conversations,
//! and retry logic with exponential backoff.

use crate::llm::stream::{stream_chat_completion, StreamChunk};
use crate::llm::tools::execute_tool_calls;
use crate::llm::types::*;
use futures::StreamExt;
use mcp::registry::McpRegistry;
use reqwest::Client;
use serde_json::json;
use shared::config::LlmConfig;
use shared::errors::{LlmError, TelisqError};
use std::time::Duration;
use tokio::time::timeout;
use tracing::*;

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Initial delay between retries (milliseconds).
    pub initial_delay: Duration,
    /// Maximum delay between retries (milliseconds).
    pub max_delay: Duration,
    /// Multiplier for exponential backoff.
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        }
    }
}

/// Determines if an error is retryable.
/// 4xx client errors (except 429) are not retryable.
/// 5xx server errors and 429 rate limits are retryable.
fn is_retryable_error(status: reqwest::StatusCode) -> bool {
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return true;
    }
    status.is_server_error()
}

/// Calculates the delay for a retry attempt using exponential backoff.
fn calculate_retry_delay(config: &RetryConfig, attempt: u32) -> Duration {
    let delay_ms =
        config.initial_delay.as_millis() as f64 * config.backoff_multiplier.powi(attempt as i32);
    let delay = Duration::from_millis(delay_ms as u64);
    delay.min(config.max_delay)
}

/// Extracts Retry-After header value from response, if present.
fn get_retry_after(response: &reqwest::Response) -> Option<Duration> {
    response
        .headers()
        .get("Retry-After")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
}

pub struct LlmClient {
    client: Client,
    config: LlmConfig,
    timeout: Duration,
    retry_config: RetryConfig,
}

impl LlmClient {
    /// Creates a new LLM client with the given configuration.
    pub fn new(config: LlmConfig) -> Self {
        Self {
            client: Client::new(),
            config,
            timeout: Duration::from_secs(60), // Default timeout
            retry_config: RetryConfig::default(),
        }
    }

    /// Creates a new LLM client with custom retry configuration.
    pub fn with_retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.retry_config = retry_config;
        self
    }

    /// Sends a chat completion request to the LLM with retry logic.
    pub async fn chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, TelisqError> {
        self.chat_completion_with_retry(request).await
    }

    /// Internal method: sends a chat completion request with retry logic.
    async fn chat_completion_with_retry(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, TelisqError> {
        let mut attempt = 0;

        loop {
            match self.send_chat_completion_request(&request).await {
                Ok(response) => return Ok(response),
                Err((error, is_retryable, retry_after)) => {
                    if attempt >= self.retry_config.max_retries || !is_retryable {
                        error!(
                            attempt = %attempt,
                            max_retries = %self.retry_config.max_retries,
                            is_retryable = %is_retryable,
                            error = %error,
                            "Chat completion failed, no more retries"
                        );
                        return Err(error);
                    }

                    let delay = retry_after
                        .unwrap_or_else(|| calculate_retry_delay(&self.retry_config, attempt));

                    warn!(
                        attempt = %attempt,
                        max_retries = %self.retry_config.max_retries,
                        delay_ms = %delay.as_millis(),
                        error = %error,
                        "Chat completion failed, retrying"
                    );

                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
            }
        }
    }

    /// Sends the actual HTTP request for chat completion.
    /// Returns (Error, is_retryable, optional_retry_after) on failure.
    async fn send_chat_completion_request(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, (TelisqError, bool, Option<Duration>)> {
        info!(
            model = %self.config.model,
            messages = %request.messages.len(),
            "Sending chat completion request"
        );

        let body = json!({
            "model": self.config.model,
            "messages": request.messages,
            "temperature": self.config.temperature,
            "max_tokens": self.config.max_tokens,
            "tools": request.tools,
            "tool_choice": request.tool_choice,
            "stream": request.stream,
        });

        let response = timeout(
            self.timeout,
            self.client
                .post(format!("{}/chat/completions", self.config.base_url))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .json(&body)
                .send(),
        )
        .await
        .map_err(|_| {
            (
                LlmError::ConnectionError("Request timed out".to_string()).into(),
                true,
                None,
            )
        })?
        .map_err(|e| (LlmError::ConnectionError(e.to_string()).into(), true, None))?;

        let status = response.status();
        if !status.is_success() {
            let retry_after = if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                get_retry_after(&response)
            } else {
                None
            };

            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            let error = LlmError::ApiError(format!("HTTP {}: {}", status, error_text));
            return Err((error.into(), is_retryable_error(status), retry_after));
        }

        let completion: ChatCompletionResponse = response
            .json()
            .await
            .map_err(|e| (LlmError::ParseError(e.to_string()).into(), false, None))?;

        info!(
            model = %self.config.model,
            usage = ?completion.usage,
            "Chat completion received"
        );

        Ok(completion)
    }

    /// Sends a chat completion request with tool calling support.
    /// Handles the full multi-turn tool call loop:
    /// 1. Send request with tools
    /// 2. Parse tool_calls from response
    /// 3. Execute each tool call
    /// 4. Send results back to LLM
    /// 5. Repeat until no more tool calls
    pub async fn chat_completion_with_tools(
        &self,
        request: ChatCompletionRequest,
        registry: &McpRegistry,
        max_turns: u32,
    ) -> Result<ChatCompletionResponse, TelisqError> {
        info!(
            model = %self.config.model,
            tools = ?request.tools.as_ref().map(|t| t.len()).unwrap_or(0),
            "Starting chat completion with tools"
        );

        let mut messages = request.messages.clone();
        let mut turn_count = 0;

        loop {
            if turn_count >= max_turns {
                return Err(LlmError::ApiError(format!(
                    "Exceeded maximum tool call turns ({})",
                    max_turns
                ))
                .into());
            }

            // Build request with current messages
            let current_request = ChatCompletionRequest {
                messages: messages.clone(),
                tools: request.tools.clone(),
                tool_choice: request.tool_choice.clone(),
                stream: false,
            };

            let response = self.chat_completion(current_request).await?;

            // Check if there are tool calls in the response
            let choice = response.choices.first();
            let tool_calls = match choice {
                Some(c) => &c.message.tool_calls,
                None => &None,
            };

            match tool_calls {
                Some(tool_calls) if !tool_calls.is_empty() => {
                    info!(
                        tool_calls = %tool_calls.len(),
                        turn = %turn_count,
                        "Processing tool calls"
                    );

                    // Add assistant message with tool calls
                    messages.push(Message {
                        role: Role::Assistant,
                        content: choice
                            .and_then(|c| c.message.content.clone())
                            .unwrap_or_default(),
                        name: None,
                    });

                    // Execute all tool calls
                    let results = execute_tool_calls(tool_calls, registry).await;

                    // Add tool results as function messages
                    for (i, (_tool_call_id, result)) in results.iter().enumerate() {
                        let tool_call = &tool_calls[i];
                        let content = match result {
                            Ok(result) => result.clone(),
                            Err(e) => format!("Error: {}", e),
                        };

                        messages.push(Message {
                            role: Role::Function,
                            content,
                            name: Some(tool_call.function.name.clone()),
                        });
                    }

                    turn_count += 1;
                    // Continue loop to send results back to LLM
                }
                _ => {
                    // No tool calls, return the response
                    info!(
                        turn = %turn_count,
                        "Chat completion with tools completed, no more tool calls"
                    );
                    return Ok(response);
                }
            }
        }
    }

    /// Helper method to create a simple chat completion with a single message.
    pub async fn simple_chat_completion(
        &self,
        role: Role,
        content: &str,
    ) -> Result<String, TelisqError> {
        let request = ChatCompletionRequest {
            messages: vec![Message {
                role,
                content: content.to_string(),
                name: None,
            }],
            tools: None,
            tool_choice: None,
            stream: false,
        };

        let response = self.chat_completion(request).await?;
        response
            .choices
            .first()
            .and_then(|choice| choice.message.content.as_ref())
            .ok_or_else(|| LlmError::ParseError("No response content".to_string()).into())
            .map(|s| s.to_string())
    }

    /// Streams a chat completion response.
    /// Returns a stream of StreamChunk items.
    pub fn stream_chat_completion(
        &self,
        request: ChatCompletionRequest,
    ) -> impl futures::Stream<Item = Result<StreamChunk, LlmError>> {
        stream_chat_completion(&self.config, request)
    }

    /// Collects all content from a streaming response into a single string.
    pub async fn collect_stream_content(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<String, TelisqError> {
        let mut content = String::new();
        let mut stream = self.stream_chat_completion(request);

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(StreamChunk::Content(text)) => {
                    content.push_str(&text);
                }
                Ok(StreamChunk::ToolCall(tool_call)) => {
                    warn!(
                        tool = %tool_call.function.name,
                        "Received tool call in streaming response"
                    );
                }
                Ok(StreamChunk::Done) => {
                    break;
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }

        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::config::LlmConfig;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[tokio::test]
    async fn test_chat_completion() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "chatcmpl-123",
                "object": "chat.completion",
                "created": 1677652288,
                "model": "gpt-4o",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello! How can I help you?",
                        "toolCalls": null
                    },
                    "finishReason": "stop"
                }],
                "usage": {
                    "promptTokens": 10,
                    "completionTokens": 15,
                    "totalTokens": 25
                }
            })))
            .mount(&mock_server)
            .await;

        let config = LlmConfig {
            api_key: "test-key".to_string(),
            base_url: mock_server.uri(),
            model: "gpt-4o".to_string(),
            temperature: 0.1,
            max_tokens: 4096,
        };

        let client = LlmClient::new(config);

        let request = ChatCompletionRequest {
            messages: vec![Message {
                role: Role::User,
                content: "Hello!".to_string(),
                name: None,
            }],
            tools: None,
            tool_choice: None,
            stream: false,
        };

        let response = client.chat_completion(request).await;
        assert!(response.is_ok());

        let completion = response.unwrap();
        assert_eq!(completion.choices.len(), 1);
        assert_eq!(
            completion.choices[0].message.content,
            Some("Hello! How can I help you?".to_string())
        );
        assert_eq!(completion.choices[0].message.role, Role::Assistant);
    }

    #[tokio::test]
    async fn test_simple_chat_completion() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "chatcmpl-456",
                "object": "chat.completion",
                "created": 1677652300,
                "model": "gpt-4o",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "42",
                        "toolCalls": null
                    },
                    "finishReason": "stop"
                }],
                "usage": {
                    "promptTokens": 5,
                    "completionTokens": 1,
                    "totalTokens": 6
                }
            })))
            .mount(&mock_server)
            .await;

        let config = LlmConfig {
            api_key: "test-key".to_string(),
            base_url: mock_server.uri(),
            model: "gpt-4o".to_string(),
            temperature: 0.1,
            max_tokens: 4096,
        };

        let client = LlmClient::new(config);
        let response = client
            .simple_chat_completion(Role::User, "What's 6*7?")
            .await;

        assert!(response.is_ok());
        assert_eq!(response.unwrap(), "42");
    }

    #[tokio::test]
    async fn test_retry_on_500() {
        let mock_server = MockServer::start().await;

        // First request fails with 500
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(500).set_body_json(json!({
                "error": "Internal server error"
            })))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        // Second request succeeds
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "chatcmpl-789",
                "object": "chat.completion",
                "created": 1677652400,
                "model": "gpt-4o",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Success after retry",
                        "toolCalls": null
                    },
                    "finishReason": "stop"
                }],
                "usage": {
                    "promptTokens": 10,
                    "completionTokens": 10,
                    "totalTokens": 20
                }
            })))
            .mount(&mock_server)
            .await;

        let config = LlmConfig {
            api_key: "test-key".to_string(),
            base_url: mock_server.uri(),
            model: "gpt-4o".to_string(),
            temperature: 0.1,
            max_tokens: 4096,
        };

        let client = LlmClient::new(config).with_retry_config(RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
        });

        let request = ChatCompletionRequest {
            messages: vec![Message {
                role: Role::User,
                content: "Test retry".to_string(),
                name: None,
            }],
            tools: None,
            tool_choice: None,
            stream: false,
        };

        let response = client.chat_completion(request).await;
        assert!(response.is_ok());
        let completion = response.unwrap();
        assert_eq!(
            completion.choices[0].message.content,
            Some("Success after retry".to_string())
        );
    }

    #[tokio::test]
    async fn test_no_retry_on_400() {
        let mock_server = MockServer::start().await;
        let mut request_count = 0;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "error": "Bad request"
            })))
            .mount(&mock_server)
            .await;

        let config = LlmConfig {
            api_key: "test-key".to_string(),
            base_url: mock_server.uri(),
            model: "gpt-4o".to_string(),
            temperature: 0.1,
            max_tokens: 4096,
        };

        let client = LlmClient::new(config).with_retry_config(RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
        });

        let request = ChatCompletionRequest {
            messages: vec![Message {
                role: Role::User,
                content: "Test no retry".to_string(),
                name: None,
            }],
            tools: None,
            tool_choice: None,
            stream: false,
        };

        let response = client.chat_completion(request).await;
        assert!(response.is_err());
        // Should fail immediately without retrying
        assert_eq!(request_count, 0);
    }

    #[test]
    fn test_retry_config_defaults() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay, Duration::from_millis(1000));
        assert_eq!(config.max_delay, Duration::from_secs(30));
        assert_eq!(config.backoff_multiplier, 2.0);
    }

    #[test]
    fn test_calculate_retry_delay() {
        let config = RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
        };

        // Attempt 0: 100ms * 2^0 = 100ms
        assert_eq!(
            calculate_retry_delay(&config, 0),
            Duration::from_millis(100)
        );
        // Attempt 1: 100ms * 2^1 = 200ms
        assert_eq!(
            calculate_retry_delay(&config, 1),
            Duration::from_millis(200)
        );
        // Attempt 2: 100ms * 2^2 = 400ms
        assert_eq!(
            calculate_retry_delay(&config, 2),
            Duration::from_millis(400)
        );
    }

    #[test]
    fn test_is_retryable_error() {
        assert!(is_retryable_error(reqwest::StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_error(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR
        ));
        assert!(is_retryable_error(reqwest::StatusCode::BAD_GATEWAY));
        assert!(is_retryable_error(reqwest::StatusCode::SERVICE_UNAVAILABLE));

        assert!(!is_retryable_error(reqwest::StatusCode::BAD_REQUEST));
        assert!(!is_retryable_error(reqwest::StatusCode::UNAUTHORIZED));
        assert!(!is_retryable_error(reqwest::StatusCode::FORBIDDEN));
        assert!(!is_retryable_error(reqwest::StatusCode::NOT_FOUND));
    }
}
