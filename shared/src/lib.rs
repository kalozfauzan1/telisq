// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

//! Telisq shared domain contracts and utilities.
//!
//! This crate contains the core data structures, errors, and utilities used across all Telisq
//! components. It provides a common foundation for working with plans, tasks, agents, and
//! sessions.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

/// Brief and result contracts exchanged between orchestrator and agents.
pub mod brief;
/// Configuration models and loading/merging helpers.
pub mod config;
/// Shared error types used across crates.
pub mod errors;
/// Core domain types used by plans, sessions, and orchestration.
pub mod types;
