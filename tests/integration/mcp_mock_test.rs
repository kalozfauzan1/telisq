#[cfg(test)]
mod tests {
    use mcp::server::MCPServer;
    use mcp::protocol::MCPMessage;
    use tempdir::TempDir;

    #[test]
    fn test_mcp_server_initialization() {
        let temp_dir = TempDir::new("mcp-mock-test").expect("Failed to create temp dir");
        let server = MCPServer::new(temp_dir.path());
        
        assert!(server.is_ok());
    }

    #[test]
    fn test_mcp_server_start_stop() {
        let temp_dir = TempDir::new("mcp-mock-test").expect("Failed to create temp dir");
        let mut server = MCPServer::new(temp_dir.path()).expect("Failed to create MCP server");
        
        let result = server.start();
        assert!(result.is_ok());
        
        let result = server.stop();
        assert!(result.is_ok());
    }

    #[test]
    fn test_mcp_registry() {
        let temp_dir = TempDir::new("mcp-mock-test").expect("Failed to create temp dir");
        let mut server = MCPServer::new(temp_dir.path()).expect("Failed to create MCP server");
        
        // Start server
        server.start().expect("Failed to start server");
        
        // Get registry
        let registry = server.get_registry();
        
        assert!(!registry.get_tools().is_empty());
        
        // Check for standard tools
        let tools = registry.get_tools();
        assert!(tools.iter().any(|t| t.name == "read_file"));
        assert!(tools.iter().any(|t| t.name == "write_file"));
        assert!(tools.iter().any(|t| t.name == "run_command"));
        
        server.stop().expect("Failed to stop server");
    }

    #[test]
    #[cfg(feature = "real-mcp-tests")]
    fn test_real_mcp_connection() {
        // This test requires real MCP server connection and will only run if feature is enabled
        let temp_dir = TempDir::new("mcp-real-test").expect("Failed to create temp dir");
        let mut server = MCPServer::new(temp_dir.path()).expect("Failed to create MCP server");
        
        server.start().expect("Failed to start server");
        
        // Test that we can create a connection
        let result = server.create_connection();
        assert!(result.is_ok());
        
        let mut connection = result.unwrap();
        
        // Test sending a simple message
        let test_message = MCPMessage::Hello {
            version: "1.0".to_string(),
            capabilities: vec![],
        };
        
        let send_result = connection.send_message(test_message);
        assert!(send_result.is_ok());
        
        server.stop().expect("Failed to stop server");
    }
}
