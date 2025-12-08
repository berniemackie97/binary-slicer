use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::db::{ProjectConfig, ProjectDb, ProjectLayout};

/// Load the project config JSON from disk for a given layout.
pub fn load_project_config(layout: &ProjectLayout) -> Result<ProjectConfig> {
    let config_json = std::fs::read_to_string(&layout.project_config_path).with_context(|| {
        format!("Failed to read project config at {}", layout.project_config_path.display())
    })?;
    let config: ProjectConfig =
        serde_json::from_str(&config_json).context("Failed to parse project config JSON")?;
    Ok(config)
}

/// Resolve the DB path (respecting relative/absolute config) and open a ProjectDb.
pub fn open_project_db(layout: &ProjectLayout) -> Result<(ProjectConfig, PathBuf, ProjectDb)> {
    let config = load_project_config(layout)?;
    let config_db_path = std::path::Path::new(&config.db.path);
    let db_path = if config_db_path.is_absolute() {
        config_db_path.to_path_buf()
    } else {
        layout.root.join(config_db_path)
    };
    let db = ProjectDb::open(&db_path)
        .with_context(|| format!("Failed to open project database at {}", db_path.display()))?;
    Ok((config, db_path, db))
}
