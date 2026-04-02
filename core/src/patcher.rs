// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use shared::types::FilePath;
use std::fs;

/// Patch for a specific file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilePatch {
    /// Path to the file being patched.
    pub file_path: FilePath,
    /// Original content to replace.
    pub original: String,
    /// New content.
    pub replacement: String,
}

/// Result of applying a patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchResult {
    /// Patch applied successfully.
    Success,
    /// Patch failed to apply.
    Failure(String),
    /// File not found.
    FileNotFound,
    /// Content mismatch.
    ContentMismatch,
}

/// Patcher for applying surgical patches to files.
pub struct Patcher;

impl Patcher {
    /// Applies a single patch to a file.
    pub fn apply_patch(patch: &FilePatch) -> PatchResult {
        let file_path = &patch.file_path;
        if !file_path.exists() {
            return PatchResult::FileNotFound;
        }

        match fs::read_to_string(file_path) {
            Ok(content) => {
                if !content.contains(&patch.original) {
                    return PatchResult::ContentMismatch;
                }

                let new_content = content.replace(&patch.original, &patch.replacement);
                match fs::write(file_path, new_content) {
                    Ok(_) => PatchResult::Success,
                    Err(e) => PatchResult::Failure(format!("Failed to write file: {}", e)),
                }
            }
            Err(e) => PatchResult::Failure(format!("Failed to read file: {}", e)),
        }
    }

    /// Applies multiple patches to files.
    pub fn apply_patches(patches: &[FilePatch]) -> Vec<(FilePath, PatchResult)> {
        patches
            .iter()
            .map(|patch| {
                let result = Self::apply_patch(patch);
                (patch.file_path.clone(), result)
            })
            .collect()
    }

    /// Verifies that a patch can be applied without errors.
    pub fn verify_patch(patch: &FilePatch) -> PatchResult {
        let file_path = &patch.file_path;
        if !file_path.exists() {
            return PatchResult::FileNotFound;
        }

        match fs::read_to_string(file_path) {
            Ok(content) => {
                if content.contains(&patch.original) {
                    PatchResult::Success
                } else {
                    PatchResult::ContentMismatch
                }
            }
            Err(e) => PatchResult::Failure(format!("Failed to read file: {}", e)),
        }
    }

    /// Verifies multiple patches.
    pub fn verify_patches(patches: &[FilePatch]) -> Vec<(FilePath, PatchResult)> {
        patches
            .iter()
            .map(|patch| {
                let result = Self::verify_patch(patch);
                (patch.file_path.clone(), result)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_patcher_applies_simple_patch() {
        // Create a temporary file
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        writeln!(temp_file, "Hello, world!").expect("Failed to write to temp file");
        let temp_path = temp_file.path().to_path_buf();

        // Create a patch
        let patch = FilePatch {
            file_path: temp_path.clone(),
            original: "Hello, world!".to_string(),
            replacement: "Hello, Rust!".to_string(),
        };

        // Apply the patch
        let result = Patcher::apply_patch(&patch);
        assert!(matches!(result, PatchResult::Success));

        // Verify the patch was applied
        let content = fs::read_to_string(temp_path).expect("Failed to read temp file");
        assert_eq!(content.trim(), "Hello, Rust!");
    }

    #[test]
    fn test_patcher_verify_content_mismatch() {
        // Create a temporary file
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        writeln!(temp_file, "Hello, world!").expect("Failed to write to temp file");
        let temp_path = temp_file.path().to_path_buf();

        // Create a patch with incorrect original content
        let patch = FilePatch {
            file_path: temp_path.clone(),
            original: "Incorrect content".to_string(),
            replacement: "Hello, Rust!".to_string(),
        };

        // Verify the patch
        let result = Patcher::verify_patch(&patch);
        assert!(matches!(result, PatchResult::ContentMismatch));
    }

    #[test]
    fn test_patcher_verify_file_not_found() {
        let patch = FilePatch {
            file_path: "nonexistent_file.txt".into(),
            original: "Hello".to_string(),
            replacement: "Hi".to_string(),
        };

        let result = Patcher::verify_patch(&patch);
        assert!(matches!(result, PatchResult::FileNotFound));
    }
}
