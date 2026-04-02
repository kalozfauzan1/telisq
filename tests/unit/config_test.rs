#[cfg(test)]
mod tests {
    use shared::config::Config;
    use tempdir::TempDir;

    #[test]
    fn test_config_creation() {
        let temp_dir = TempDir::new("config-test").expect("Failed to create temp dir");
        let config = Config::new(temp_dir.path());
        
        assert!(config.is_ok());
    }

    #[test]
    fn test_config_save_load() {
        let temp_dir = TempDir::new("config-test").expect("Failed to create temp dir");
        let mut config = Config::new(temp_dir.path()).expect("Failed to create config");
        
        // Set some values
        config.set_llm_model("gpt-4").expect("Failed to set LLM model");
        config.set_api_key("test-key").expect("Failed to set API key");
        config.set_max_retries(5).expect("Failed to set max retries");
        
        // Save and reload
        config.save().expect("Failed to save config");
        let loaded_config = Config::new(temp_dir.path()).expect("Failed to load config");
        
        assert_eq!(loaded_config.get_llm_model(), "gpt-4");
        assert_eq!(loaded_config.get_api_key(), "test-key");
        assert_eq!(loaded_config.get_max_retries(), 5);
    }

    #[test]
    fn test_config_default_values() {
        let temp_dir = TempDir::new("config-test").expect("Failed to create temp dir");
        let config = Config::new(temp_dir.path()).expect("Failed to create config");
        
        // Check that default values are set
        assert!(!config.get_llm_model().is_empty());
        assert!(config.get_api_key().is_empty()); // Default should be empty
        assert!(config.get_max_retries() > 0);
    }

    #[test]
    fn test_config_invalid_values() {
        let temp_dir = TempDir::new("config-test").expect("Failed to create temp dir");
        let mut config = Config::new(temp_dir.path()).expect("Failed to create config");
        
        // Should not allow negative retries
        let result = config.set_max_retries(-1);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_overrides() {
        let temp_dir = TempDir::new("config-test").expect("Failed to create temp dir");
        let mut config = Config::new(temp_dir.path()).expect("Failed to create config");
        
        // Test setting and overriding values
        config.set_llm_model("gpt-3.5-turbo").expect("Failed to set LLM model");
        assert_eq!(config.get_llm_model(), "gpt-3.5-turbo");
        
        config.set_llm_model("gpt-4").expect("Failed to set LLM model");
        assert_eq!(config.get_llm_model(), "gpt-4");
    }
}
