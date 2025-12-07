//! Core data model (IR) for binaries, functions, slices, and evidence.
//!
//! This module will eventually contain:
//! - Binary / segment representation
//! - Function and basic block structures
//! - Slice definitions and membership
//! - Evidence records explaining why something was classified a certain way

/// Placeholder type for a binary identifier.
///
/// Later this will likely include hashes, path, architecture, etc.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BinaryId {
    pub name: String,
}

/// Placeholder type for a slice identifier.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SliceId {
    pub name: String,
}

/// Very small placeholder IR type for a function.
///
/// This will grow to include address, size, xrefs, etc.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Function {
    pub name: String,
}

impl Function {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}
