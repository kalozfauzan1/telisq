#[cfg(test)]
mod tests {
    use core::llm::client::LLMClient;
    use core::llm::types::{ChatRequest, ChatResponse};
    use shared::config::Config;
    use tempdir::TempDir;

    #[test]
    fn test_llm_client_initialization() {
        let temp_dir = TempDir::new("llm-mock-test").expect("Failed to create temp dir");
        let config = Config::new(temp_dir.path()).expect("Failed to create config");
        
        let client = LLMClient::new(&config);
        
        assert!(client.is_ok());
    }

    #[test]
    fn test_llm_mock_response() {
        let temp_dir = TempDir::new("llm-mock-test").expect("Failed to create temp dir");
        let config = Config::new(temp_dir.path()).expect("Failed to create config");
        
        let mut client = LLMClient::new(&config).expect("Failed to create LLM client");
        
        // Use mock mode
        client.set_mock_mode(true);
        
        let request = ChatRequest {
            messages: vec![],
            temperature: 0.7,
            max_tokens: Some(500),
        };
        
        let response = client.chat(request);
        
        assert!(response.is_ok());
        
        let chat_response = response.unwrap();
        assert!(!chat_response.content.is_empty());
    }

    #[test]
    fn test_llm_mock_streaming() {
        let temp_dir = TempDir::new("llm-mock-test").expect("Failed to create temp dir");
        let config = Config::new(temp_dir.path()).expect("Failed to create config");
        
        let mut client = LLMClient::new(&config).expect("Failed to create LLM client");
        client.set_mock_mode(true);
        
        let request = ChatRequest {
            messages: vec![],
            temperature: 0.7,
            max_tokens: Some(500),
        };
        
        let mut stream = client.stream_chat(request).expect("Failed to create streaming response");
        
        let mut full_content = String::new();
        while let Some(chunk) = stream.next() {
            let chunk = chunk.expect("Failed to get chunk");
            full_content.push_str(&chunk.content);
        }
        
        assert!(!full_content.is_empty());
    }

    #[test]
    #[cfg(feature = "real-llm-tests")]
    fn test_real_llm_response() {
        // This test requires real API keys and will only run if the feature is enabled
        let temp_dir = TempDir::new("llm-real-test").expect("Failed to create temp dir");
        let config = Config::new(temp_dir.path()).expect("Failed to create config");
        
        let mut client = LLMClient::new(&config).expect("Failed to create LLM client");
        client.set_mock_mode(false);
        
        let request = ChatRequest {
            messages: vec![("user", "Hello, world!").into()],
            temperature: 0.7,
            max_tokens: Some(100),
        };
        
        let response = client.chat(request);
        
        assert!(response.is_ok());
        
        let chat_response = response.unwrap();
        assert!(!chat_response.content.is_empty());
        assert!(chat_response.content.contains("Hello") || chat_response.content.contains("world"));
    }
}
