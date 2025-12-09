use serde::{Deserialize, Serialize};

/// Placeholder for database configuration.
///
/// In future steps, this will likely be backed by a SQLite connection
/// and migration management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbConfig {
    /// Path to the project database file (typically relative to project root).
    pub path: String,
}

impl DbConfig {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

/// Serializable configuration describing a Binary Slicer project.
///
/// This lives (for now) at `.ritual/project.json` in the project root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Human-friendly project name.
    pub name: String,
    /// Optional description / notes.
    pub description: Option<String>,
    /// Schema/config version. This is about the config format, not binary version.
    pub config_version: String,
    /// Database configuration (path is typically relative to project root).
    pub db: DbConfig,
    /// Optional default analysis backend to use when none is provided in CLI or spec.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_backend: Option<String>,
}

impl ProjectConfig {
    /// Create a new project configuration using the given name and db path.
    pub fn new(name: impl Into<String>, db_path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            config_version: "0.1.0".to_string(),
            db: DbConfig::new(db_path),
            default_backend: None,
        }
    }
}
