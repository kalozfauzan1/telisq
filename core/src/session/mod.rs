// Copyright 2026 Your Name.
// SPDX-License-Identifier: MIT

//! Session persistence module using SQLite via sqlx.
//!
//! This module provides:
//! - `SessionStore` for managing SQLite database connections
//! - Session save/load operations
//! - Event logging
//! - Session resume capability

pub mod store;

pub use store::SessionStore;
