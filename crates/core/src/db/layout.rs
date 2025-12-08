use std::path::{Path, PathBuf};

/// Logical layout of a project on disk.
///
/// This is derived from a chosen root path. It does *not* perform any IO itself.
/// The CLI or other frontends are responsible for actually creating directories
/// and files based on this layout.
#[derive(Debug, Clone)]
pub struct ProjectLayout {
    /// Root directory of the project.
    pub root: PathBuf,
    /// Directory for internal metadata (.ritual).
    pub meta_dir: PathBuf,
    /// Path to the project config file (JSON).
    pub project_config_path: PathBuf,
    /// Path to the project database file.
    pub db_path: PathBuf,
    /// Directory for documentation (docs).
    pub docs_dir: PathBuf,
    /// Directory for slice-specific documentation (docs/slices).
    pub slices_docs_dir: PathBuf,
    /// Directory for structured reports (reports).
    pub reports_dir: PathBuf,
    /// Directory for graph artifacts (graphs).
    pub graphs_dir: PathBuf,
    /// Directory for ritual specs/pipelines.
    pub rituals_dir: PathBuf,
    /// Directory for analysis outputs (organized by binary).
    pub outputs_dir: PathBuf,
    /// Directory for per-binary output artifacts.
    pub outputs_binaries_dir: PathBuf,
}

impl ProjectLayout {
    /// Compute the default layout for a project rooted at `root`.
    ///
    /// This does *not* touch the filesystem.
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let meta_dir = root.join(".ritual");
        let project_config_path = meta_dir.join("project.json");
        let db_path = meta_dir.join("project.db");
        let docs_dir = root.join("docs");
        let slices_docs_dir = docs_dir.join("slices");
        let reports_dir = root.join("reports");
        let graphs_dir = root.join("graphs");
        let rituals_dir = root.join("rituals");
        let outputs_dir = root.join("outputs");
        let outputs_binaries_dir = outputs_dir.join("binaries");

        Self {
            root,
            meta_dir,
            project_config_path,
            db_path,
            docs_dir,
            slices_docs_dir,
            reports_dir,
            graphs_dir,
            rituals_dir,
            outputs_dir,
            outputs_binaries_dir,
        }
    }

    /// Compute a database path string suitable for storing in `ProjectConfig`,
    /// typically as a path relative to `root`.
    pub fn db_path_relative_string(&self) -> String {
        match self.db_path.strip_prefix(&self.root) {
            Ok(rel) => rel.to_string_lossy().to_string(),
            Err(_) => self.db_path.to_string_lossy().to_string(),
        }
    }

    /// Helper to compute a per-binary output root directory.
    pub fn binary_output_root(&self, binary_name: &str) -> PathBuf {
        self.outputs_binaries_dir.join(binary_name)
    }
}
