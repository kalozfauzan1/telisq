#[cfg(test)]
mod tests {
    use core::patcher::Patcher;
    use tempdir::TempDir;

    #[test]
    fn test_patcher_creation() {
        let temp_dir = TempDir::new("patcher-test").expect("Failed to create temp dir");
        let patcher = Patcher::new(temp_dir.path());
        
        assert!(patcher.is_ok());
    }

    #[test]
    fn test_file_patching() {
        let temp_dir = TempDir::new("patcher-test").expect("Failed to create temp dir");
        let patcher = Patcher::new(temp_dir.path()).expect("Failed to create patcher");
        
        // Create test file
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "original content").expect("Failed to write test file");
        
        // Patch file
        let result = patcher.patch_file("test.txt", "original content", "patched content");
        
        assert!(result.is_ok());
        
        // Verify patch
        let content = std::fs::read_to_string(&test_file).expect("Failed to read test file");
        assert_eq!(content, "patched content");
    }

    #[test]
    fn test_file_patch_reversal() {
        let temp_dir = TempDir::new("patcher-test").expect("Failed to create temp dir");
        let patcher = Patcher::new(temp_dir.path()).expect("Failed to create patcher");
        
        // Create test file
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "original content").expect("Failed to write test file");
        
        // Patch file
        let result = patcher.patch_file("test.txt", "original content", "patched content");
        assert!(result.is_ok());
        
        // Revert patch
        let result = patcher.revert();
        assert!(result.is_ok());
        
        // Verify original content
        let content = std::fs::read_to_string(&test_file).expect("Failed to read test file");
        assert_eq!(content, "original content");
    }

    #[test]
    fn test_patch_multiple_files() {
        let temp_dir = TempDir::new("patcher-test").expect("Failed to create temp dir");
        let patcher = Patcher::new(temp_dir.path()).expect("Failed to create patcher");
        
        // Create test files
        let test_file1 = temp_dir.path().join("test1.txt");
        let test_file2 = temp_dir.path().join("test2.txt");
        std::fs::write(&test_file1, "content1").expect("Failed to write test file 1");
        std::fs::write(&test_file2, "content2").expect("Failed to write test file 2");
        
        // Patch both files
        let result1 = patcher.patch_file("test1.txt", "content1", "patched1");
        let result2 = patcher.patch_file("test2.txt", "content2", "patched2");
        assert!(result1.is_ok());
        assert!(result2.is_ok());
        
        // Verify patches
        let content1 = std::fs::read_to_string(&test_file1).expect("Failed to read test file 1");
        let content2 = std::fs::read_to_string(&test_file2).expect("Failed to read test file 2");
        assert_eq!(content1, "patched1");
        assert_eq!(content2, "patched2");
        
        // Revert all patches
        let result = patcher.revert();
        assert!(result.is_ok());
        
        // Verify original content
        let content1 = std::fs::read_to_string(&test_file1).expect("Failed to read test file 1");
        let content2 = std::fs::read_to_string(&test_file2).expect("Failed to read test file 2");
        assert_eq!(content1, "content1");
        assert_eq!(content2, "content2");
    }

    #[test]
    fn test_patch_nonexistent_file() {
        let temp_dir = TempDir::new("patcher-test").expect("Failed to create temp dir");
        let patcher = Patcher::new(temp_dir.path()).expect("Failed to create patcher");
        
        let result = patcher.patch_file("nonexistent.txt", "original", "patched");
        
        assert!(result.is_err());
    }

    #[test]
    fn test_patch_no_match() {
        let temp_dir = TempDir::new("patcher-test").expect("Failed to create temp dir");
        let patcher = Patcher::new(temp_dir.path()).expect("Failed to create patcher");
        
        // Create test file
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "original content").expect("Failed to write test file");
        
        let result = patcher.patch_file("test.txt", "no match", "patched content");
        
        assert!(result.is_err());
    }
}
