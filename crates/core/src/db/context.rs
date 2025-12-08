use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::db::{open_project_db, ProjectConfig, ProjectDb, ProjectLayout};

/// Convenience wrapper bundling layout, config, db path, and an open ProjectDb.
#[derive(Debug)]
pub struct ProjectContext {
    pub layout: ProjectLayout,
    pub config: ProjectConfig,
    pub db_path: PathBuf,
    pub db: ProjectDb,
}

impl ProjectContext {
    /// Load project config and open the database for a given root.
    pub fn from_root(root: impl AsRef<Path>) -> Result<Self> {
        let layout = ProjectLayout::new(root);
        let (config, db_path, db) = open_project_db(&layout)?;
        Ok(Self { layout, config, db_path, db })
    }
}
