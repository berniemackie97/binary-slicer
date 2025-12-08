use serde::{Deserialize, Serialize};

/// High-level lifecycle status of a slice.
///
/// This is intentionally simple; finer-grained states can be added later.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SliceStatus {
    /// Slice name reserved / planned, but not yet actively worked.
    Planned,
    /// Slice exists but hasn't been fully fleshed out.
    Draft,
    /// Slice is actively maintained and trusted.
    Active,
    /// Slice is still stored for historical reference but is no longer maintained.
    Deprecated,
}

impl SliceStatus {
    /// Encode as an integer for storage in SQLite.
    pub fn to_i32(self) -> i32 {
        match self {
            SliceStatus::Planned => 0,
            SliceStatus::Draft => 1,
            SliceStatus::Active => 2,
            SliceStatus::Deprecated => 3,
        }
    }

    /// Decode from an integer stored in SQLite.
    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => SliceStatus::Planned,
            1 => SliceStatus::Draft,
            2 => SliceStatus::Active,
            3 => SliceStatus::Deprecated,
            _ => SliceStatus::Draft,
        }
    }
}

/// Record describing a binary known to the project.
///
/// Eventually this will map to a DB table. For now, it's both a schema hint
/// and a value type used in the DB layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BinaryRecord {
    /// Human-friendly name (e.g., "libExampleGame.so (armv7)").
    pub name: String,
    /// Path to the binary, relative to the project root if possible.
    pub path: String,
    /// Optional architecture string (e.g., "armv7", "x86_64").
    pub arch: Option<String>,
    /// Optional content hash for identity (e.g., SHA-256).
    pub hash: Option<String>,
}

impl BinaryRecord {
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self { name: name.into(), path: path.into(), arch: None, hash: None }
    }
}

/// Record describing a slice known to the project.
///
/// This captures the project-level view (name, status, description) and is
/// orthogonal to any specific analysis run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SliceRecord {
    /// Slice name (e.g., "AutoUpdateManager", "CUIManager", "Networking").
    pub name: String,
    /// Optional human-written description of this slice's purpose.
    pub description: Option<String>,
    /// Lifecycle status.
    pub status: SliceStatus,
}

impl SliceRecord {
    pub fn new(name: impl Into<String>, status: SliceStatus) -> Self {
        Self { name: name.into(), description: None, status }
    }

    /// Builder-style helper to attach a description when constructing a record.
    pub fn with_description(mut self, description: Option<String>) -> Self {
        self.description = description;
        self
    }
}

/// A high-level snapshot of project metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub config: crate::db::ProjectConfig,
    pub binaries: Vec<BinaryRecord>,
    pub slices: Vec<SliceRecord>,
}

/// Allowed status values for ritual runs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RitualRunStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Canceled,
    Stubbed,
}

impl RitualRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RitualRunStatus::Pending => "pending",
            RitualRunStatus::Running => "running",
            RitualRunStatus::Succeeded => "succeeded",
            RitualRunStatus::Failed => "failed",
            RitualRunStatus::Canceled => "canceled",
            RitualRunStatus::Stubbed => "stubbed",
        }
    }
}

/// Record describing a ritual run (analysis execution) for bookkeeping.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RitualRunRecord {
    pub binary: String,
    pub ritual: String,
    pub spec_hash: String,
    pub binary_hash: Option<String>,
    pub backend: String,
    pub status: RitualRunStatus,
    pub started_at: String,
    pub finished_at: String,
}
