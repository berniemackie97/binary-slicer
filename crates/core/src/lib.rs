//! ritual-core
//!
//! Core library for slice-oriented reverse-engineering of native binaries.
//!
//! This crate defines the internal IR (model), analysis logic, ritual DSL engine,
//! database integration, and backend adapters for disassembly tools.
//!
//! The goal is to keep all substantive logic here so it is fully testable and
//! reusable from multiple frontends (CLI, Python bindings, etc.).

pub mod analysis;
pub mod backends;
pub mod db;
pub mod model;
pub mod rituals;
pub mod services;

/// Returns the library version as encoded at compile time.
///
/// Useful for tests and for frontends to report consistent version info.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
