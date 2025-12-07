//! Project database integration and project layout definitions.
//!
//! This module will eventually wrap a SQLite database storing:
//! - Binaries and their metadata
//! - Slices and their evolution across builds
//! - Functions, xrefs, strings, and evidence records
//! - Ritual run histories
//!
//! For now, we define:
//! - `DbConfig`: simple DB path wrapper (placeholder).
//! - `ProjectConfig`: serializable project metadata.
//! - `ProjectLayout`: computed paths for project directories/files.
//! - `ProjectDb`: a small SQLite wrapper with schema v1.
//! - Basic schema-like types (`BinaryRecord`, `SliceRecord`, etc.) representing
//!   what lives in the project database.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use thiserror::Error;

/// Error type for project database operations.
#[derive(Debug, Error)]
pub enum DbError {
    /// Underlying SQLite error.
    #[error("SQLite error: {0}")]
    Sql(#[from] rusqlite::Error),
}

/// Convenience result type for DB operations.
pub type DbResult<T> = Result<T, DbError>;

/// Placeholder for database configuration.
///
/// In future steps, this will likely be backed by a SQLite connection
/// and migration management.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DbConfig {
    /// Path to the project database file (typically relative to project root).
    pub path: String,
}

impl DbConfig {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

/// Serializable configuration describing a Ritual Slicer project.
///
/// This lives (for now) at `.ritual/project.json` in the project root.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectConfig {
    /// Human-friendly project name.
    pub name: String,
    /// Optional description / notes.
    pub description: Option<String>,
    /// Schema/config version. This is about the config format, not binary version.
    pub config_version: String,
    /// Database configuration (path is typically relative to project root).
    pub db: DbConfig,
}

impl ProjectConfig {
    /// Create a new project configuration using the given name and db path.
    pub fn new(name: impl Into<String>, db_path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            config_version: "0.1.0".to_string(),
            db: DbConfig::new(db_path),
        }
    }
}

/// Logical layout of a project on disk.
///
/// This is derived from a chosen root path. It does not perform any IO itself.
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

        Self {
            root,
            meta_dir,
            project_config_path,
            db_path,
            docs_dir,
            slices_docs_dir,
            reports_dir,
            graphs_dir,
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
}

/// High-level lifecycle status of a slice.
///
/// This is intentionally simple; finer-grained states can be added later.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum SliceStatus {
    /// Slice exists but hasn't been fleshed out.
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
            SliceStatus::Draft => 0,
            SliceStatus::Active => 1,
            SliceStatus::Deprecated => 2,
        }
    }

    /// Decode from an integer stored in SQLite.
    pub fn from_i32(value: i32) -> Self {
        match value {
            1 => SliceStatus::Active,
            2 => SliceStatus::Deprecated,
            _ => SliceStatus::Draft,
        }
    }
}

/// Record describing a binary known to the project.
///
/// Eventually this will map to a DB table. For now, it's both a schema hint
/// and a value type used in the DB layer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct BinaryRecord {
    /// Human-friendly name (e.g., "libCQ2Client.so (armv7)").
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
        Self {
            name: name.into(),
            path: path.into(),
            arch: None,
            hash: None,
        }
    }
}

/// Record describing a slice known to the project.
///
/// This captures the project-level view (name, status, description) and is
/// orthogonal to any specific analysis run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
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
        Self {
            name: name.into(),
            description: None,
            status,
        }
    }
}

/// A high-level snapshot of project metadata.
///
/// In future steps, this will likely be assembled from SQL queries. For now,
/// it serves as a convenient shape for serialization if needed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectSnapshot {
    pub config: ProjectConfig,
    pub binaries: Vec<BinaryRecord>,
    pub slices: Vec<SliceRecord>,
}

/// SQLite-backed project database.
///
/// This is a thin wrapper around `rusqlite::Connection` that is responsible for:
/// - Opening/creating the DB file.
/// - Applying schema migrations.
/// - Providing small, testable helpers for querying and updating records.
pub struct ProjectDb {
    conn: Connection,
}

impl ProjectDb {
    /// Open (or create) a project database at the given path and ensure the schema exists.
    pub fn open(path: &Path) -> DbResult<Self> {
        let conn = Connection::open(path)?;
        apply_migrations(&conn)?;
        Ok(Self { conn })
    }

    /// Expose a reference to the underlying connection for advanced callers.
    /// For most code, prefer higher-level helpers.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Insert a binary record and return its row id.
    pub fn insert_binary(&self, record: &BinaryRecord) -> DbResult<i64> {
        self.conn.execute(
            r#"
            INSERT INTO binaries (name, path, arch, hash)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![record.name, record.path, record.arch, record.hash],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// List all binaries (ordered by id).
    pub fn list_binaries(&self) -> DbResult<Vec<BinaryRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT name, path, arch, hash
            FROM binaries
            ORDER BY id
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(BinaryRecord {
                name: row.get(0)?,
                path: row.get(1)?,
                arch: row.get(2)?,
                hash: row.get(3)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Insert a slice record and return its row id.
    pub fn insert_slice(&self, record: &SliceRecord) -> DbResult<i64> {
        self.conn.execute(
            r#"
            INSERT INTO slices (name, description, status)
            VALUES (?1, ?2, ?3)
            "#,
            params![
                record.name,
                record.description,
                record.status.to_i32(),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// List all slices (ordered by id).
    pub fn list_slices(&self) -> DbResult<Vec<SliceRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT name, description, status
            FROM slices
            ORDER BY id
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let status_int: i32 = row.get(2)?;
            Ok(SliceRecord {
                name: row.get(0)?,
                description: row.get(1)?,
                status: SliceStatus::from_i32(status_int),
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

/// Apply schema migrations to bring the database to the latest version.
///
/// We use `PRAGMA user_version` as the schema version indicator.
///
/// Version map:
/// - 0: no schema
/// - 1: initial schema (binaries, slices)
fn apply_migrations(conn: &Connection) -> DbResult<()> {
    let current_version = current_schema_version(conn)?;

    if current_version == 0 {
        // Initial schema.
        conn.execute_batch(
            r#"
            BEGIN;
            CREATE TABLE IF NOT EXISTS binaries (
                id   INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                arch TEXT,
                hash TEXT
            );

            CREATE TABLE IF NOT EXISTS slices (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT NOT NULL UNIQUE,
                description TEXT,
                status      INTEGER NOT NULL
            );

            PRAGMA user_version = 1;
            COMMIT;
            "#,
        )?;
    }

    // Future versions:
    // if current_version < 2 { ... }

    Ok(())
}

/// Read the SQLite schema version from `PRAGMA user_version`.
fn current_schema_version(conn: &Connection) -> DbResult<i32> {
    let version: i32 = conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    Ok(version)
}

