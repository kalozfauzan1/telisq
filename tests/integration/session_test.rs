#[cfg(test)]
mod tests {
    use cli::commands::session::SessionManager;
    use tempdir::TempDir;

    #[test]
    fn test_session_manager_initialization() {
        let temp_dir = TempDir::new("session-test").expect("Failed to create temp dir");
        let manager = SessionManager::new(temp_dir.path());
        
        assert!(manager.is_ok());
    }

    #[test]
    fn test_session_creation_and_listing() {
        let temp_dir = TempDir::new("session-test").expect("Failed to create temp dir");
        let mut manager = SessionManager::new(temp_dir.path()).expect("Failed to create session manager");
        
        // Create a new session
        let session_id = manager.create_session().expect("Failed to create session");
        assert!(!session_id.is_empty());
        
        // List sessions
        let sessions = manager.list_sessions().expect("Failed to list sessions");
        assert!(!sessions.is_empty());
        assert!(sessions.contains(&session_id));
    }

    #[test]
    fn test_session_resume() {
        let temp_dir = TempDir::new("session-test").expect("Failed to create temp dir");
        let mut manager = SessionManager::new(temp_dir.path()).expect("Failed to create session manager");
        
        // Create and resume a session
        let session_id = manager.create_session().expect("Failed to create session");
        let resumed_session = manager.resume_session(&session_id);
        
        assert!(resumed_session.is_ok());
    }

    #[test]
    fn test_nonexistent_session_resume() {
        let temp_dir = TempDir::new("session-test").expect("Failed to create temp dir");
        let manager = SessionManager::new(temp_dir.path()).expect("Failed to create session manager");
        
        let result = manager.resume_session("nonexistent-session");
        
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_sessions() {
        let temp_dir = TempDir::new("session-test").expect("Failed to create temp dir");
        let mut manager = SessionManager::new(temp_dir.path()).expect("Failed to create session manager");
        
        // Create multiple sessions
        let session1 = manager.create_session().expect("Failed to create session 1");
        let session2 = manager.create_session().expect("Failed to create session 2");
        let session3 = manager.create_session().expect("Failed to create session 3");
        
        let sessions = manager.list_sessions().expect("Failed to list sessions");
        
        assert!(sessions.contains(&session1));
        assert!(sessions.contains(&session2));
        assert!(sessions.contains(&session3));
        assert_eq!(sessions.len(), 3);
    }
}
