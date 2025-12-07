//! Ritual DSL: configuration and execution of analysis pipelines.
//!
//! For now this is just a placeholder. Eventually this module will:
//! - Define a data structure for rituals (probably serde-friendly)
//! - Parse rituals from YAML/JSON files
//! - Execute rituals by driving the analysis layer

/// Placeholder type representing a ritual identifier.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RitualId {
    pub name: String,
}
