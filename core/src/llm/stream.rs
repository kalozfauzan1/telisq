//! SSE streaming support for chat completions.
//!
//! This module provides streaming chat completion with SSE (Server-Sent Events)
//! parsing, including support for content chunks, tool calls, and stream completion.

#![allow(dead_code)]

use crate::llm::types::{ChatCompletionRequest, ToolCall};
use futures::Stream;
use reqwest::Client;
use serde::Deserialize;
use shared::config::LlmConfig;
use shared::errors::LlmError;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tracing::*;

/// A chunk from a streaming response.
#[derive(Debug, Clone)]
pub enum StreamChunk {
    /// A content delta from the stream.
    Content(String),
    /// A tool call received in the stream.
    ToolCall(ToolCall),
    /// The stream has completed.
    Done,
}

/// Internal SSE event structure for parsing.
#[derive(Debug, Deserialize)]
struct StreamEvent {
    id: Option<String>,
    choices: Option<Vec<StreamChoice>>,
    #[serde(rename = "done")]
    is_done: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    #[serde(rename = "finish_reason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    content: Option<String>,
    #[serde(rename = "tool_calls")]
    tool_calls: Option<Vec<StreamToolCall>>,
}

#[derive(Debug, Deserialize)]
struct StreamToolCall {
    index: Option<u32>,
    id: Option<String>,
    #[serde(rename = "type")]
    type_: Option<String>,
    function: Option<StreamFunctionCall>,
}

#[derive(Debug, Deserialize)]
struct StreamFunctionCall {
    name: Option<String>,
    arguments: Option<String>,
}

/// A streaming chat completion response.
///
/// Returns a stream of `StreamChunk` items that can be iterated over.
pub fn stream_chat_completion(
    config: &LlmConfig,
    request: ChatCompletionRequest,
) -> Pin<Box<dyn Stream<Item = Result<StreamChunk, LlmError>> + Send>> {
    let config = config.clone();
    let client = Client::new();
    let timeout_duration = Duration::from_secs(300); // 5 minute timeout for long streams

    Box::pin(SseStream::new(client, config, request, timeout_duration))
}

/// Internal SSE stream implementation.
struct SseStream {
    client: Client,
    config: LlmConfig,
    request: ChatCompletionRequest,
    timeout_duration: Duration,
    state: StreamState,
    buffer: String,
    response_body: Option<reqwest::Body>,
}

enum StreamState {
    Initial,
    Requesting,
    Streaming { body: reqwest::Body },
    Done,
    Error(LlmError),
}

impl SseStream {
    fn new(
        client: Client,
        config: LlmConfig,
        request: ChatCompletionRequest,
        timeout_duration: Duration,
    ) -> Self {
        Self {
            client,
            config,
            request,
            timeout_duration,
            state: StreamState::Initial,
            buffer: String::new(),
            response_body: None,
        }
    }

    fn build_request_body(&self) -> serde_json::Value {
        serde_json::json!({
            "model": self.config.model,
            "messages": self.request.messages,
            "temperature": self.config.temperature,
            "max_tokens": self.config.max_tokens,
            "tools": self.request.tools,
            "tool_choice": self.request.tool_choice,
            "stream": true,
        })
    }
}

impl Stream for SseStream {
    type Item = Result<StreamChunk, LlmError>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            match &mut this.state {
                StreamState::Initial => {
                    let body = this.build_request_body();
                    let request = this
                        .client
                        .post(format!("{}/chat/completions", this.config.base_url))
                        .header("Content-Type", "application/json")
                        .header("Authorization", format!("Bearer {}", this.config.api_key))
                        .json(&body)
                        .build()
                        .map_err(|e| LlmError::ConnectionError(e.to_string()));

                    let request = match request {
                        Ok(r) => r,
                        Err(e) => {
                            this.state = StreamState::Error(e);
                            continue;
                        }
                    };

                    let _future = this.client.execute(request);
                    // We need to poll the future here, but since we can't await in poll,
                    // we'll use a simpler approach with blocking for now
                    // In production, this should be properly async
                    this.state = StreamState::Requesting;
                    // For now, return a placeholder - proper async streaming requires
                    // more complex state management
                    return Poll::Ready(Some(Err(LlmError::ConnectionError(
                        "Streaming not yet fully implemented".to_string(),
                    ))));
                }
                StreamState::Requesting => {
                    // Transition to streaming state
                    this.state = StreamState::Done;
                    return Poll::Ready(Some(Ok(StreamChunk::Done)));
                }
                StreamState::Streaming { .. } => {
                    // Parse SSE events from buffer
                    if let Some(line_end) = this.buffer.find('\n') {
                        let line = this.buffer[..line_end].trim().to_string();
                        this.buffer = this.buffer[line_end + 1..].to_string();

                        if line.starts_with("data: ") {
                            let data = &line[6..];
                            if data == "[DONE]" {
                                this.state = StreamState::Done;
                                return Poll::Ready(Some(Ok(StreamChunk::Done)));
                            }

                            match serde_json::from_str::<StreamEvent>(data) {
                                Ok(event) => {
                                    if let Some(choices) = event.choices {
                                        for choice in choices {
                                            if let Some(content) = choice.delta.content {
                                                if !content.is_empty() {
                                                    return Poll::Ready(Some(Ok(
                                                        StreamChunk::Content(content),
                                                    )));
                                                }
                                            }
                                            if let Some(tool_calls) = choice.delta.tool_calls {
                                                for tc in tool_calls {
                                                    if let (Some(id), Some(type_), Some(func)) =
                                                        (tc.id, tc.type_, tc.function)
                                                    {
                                                        if let (Some(name), Some(arguments)) =
                                                            (func.name, func.arguments)
                                                        {
                                                            let tool_call = ToolCall {
                                                                id,
                                                                type_,
                                                                function: crate::llm::types::FunctionCall {
                                                                    name,
                                                                    arguments,
                                                                },
                                                            };
                                                            return Poll::Ready(Some(Ok(
                                                                StreamChunk::ToolCall(tool_call),
                                                            )));
                                                        }
                                                    }
                                                }
                                            }
                                            if let Some(reason) = choice.finish_reason {
                                                if reason == "stop" || reason == "tool_calls" {
                                                    this.state = StreamState::Done;
                                                    return Poll::Ready(Some(Ok(
                                                        StreamChunk::Done,
                                                    )));
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!(error = %e, data = %data, "Failed to parse SSE event");
                                }
                            }
                        }
                    } else {
                        // Need more data
                        return Poll::Pending;
                    }
                }
                StreamState::Done => {
                    return Poll::Ready(None);
                }
                StreamState::Error(e) => {
                    let error = LlmError::ConnectionError(e.to_string());
                    this.state = StreamState::Done;
                    return Poll::Ready(Some(Err(error)));
                }
            }
        }
    }
}

/// Parses SSE events from a response body string.
///
/// This is a helper function for parsing SSE responses from non-streaming
/// responses that might contain tool calls.
pub fn parse_sse_events(body: &str) -> Vec<StreamChunk> {
    let mut chunks = Vec::new();

    for line in body.lines() {
        let line = line.trim();
        if line.starts_with("data: ") {
            let data = &line[6..];
            if data == "[DONE]" {
                chunks.push(StreamChunk::Done);
                break;
            }

            match serde_json::from_str::<StreamEvent>(data) {
                Ok(event) => {
                    if let Some(choices) = event.choices {
                        for choice in choices {
                            if let Some(content) = choice.delta.content {
                                if !content.is_empty() {
                                    chunks.push(StreamChunk::Content(content));
                                }
                            }
                            if let Some(tool_calls) = choice.delta.tool_calls {
                                for tc in tool_calls {
                                    if let (Some(id), Some(type_), Some(func)) =
                                        (tc.id, tc.type_, tc.function)
                                    {
                                        if let (Some(name), Some(arguments)) =
                                            (func.name, func.arguments)
                                        {
                                            let tool_call = ToolCall {
                                                id,
                                                type_,
                                                function: crate::llm::types::FunctionCall {
                                                    name,
                                                    arguments,
                                                },
                                            };
                                            chunks.push(StreamChunk::ToolCall(tool_call));
                                        }
                                    }
                                }
                            }
                            if let Some(reason) = choice.finish_reason {
                                if reason == "stop" || reason == "tool_calls" {
                                    chunks.push(StreamChunk::Done);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e, data = %data, "Failed to parse SSE event");
                }
            }
        }
    }

    chunks
}
