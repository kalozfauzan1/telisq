// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

//! Telisq plan parser and execution tracker.
//!
//! This crate provides functionality for parsing Telisq plan files, validating them, and
//! tracking execution state.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

/// Task dependency graph construction and validation.
pub mod graph;
/// Plan file parsing into task specifications.
pub mod parser;
/// Atomic marker updates for plan task status.
pub mod tracker;
/// Plan- and task-level validators.
pub mod validator;
