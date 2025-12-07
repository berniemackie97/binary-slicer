//! ritual-core
//!
//! Core library for slice-oriented reverse-engineering of native binaries.
//!
//! This crate defines the internal IR (model), analysis logic, ritual DSL engine,
//! database integration, and backend adapters for disassembly tools.
//!
//! The goal is to keep all substantive logic here so it is fully testable and
//! reusable from multiple frontends (CLI, Python bindings, etc.).

pub mod model;
pub mod analysis;
pub mod rituals;
pub mod db;
pub mod backends;

/// Returns the library version as encoded at compile time.
///
/// Useful for tests and for frontends to report consistent version info.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

