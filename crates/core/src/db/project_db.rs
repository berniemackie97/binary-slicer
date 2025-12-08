use std::path::Path;

use rusqlite::{params, Connection};
use thiserror::Error;

use crate::db::{BinaryRecord, RitualRunRecord, RitualRunStatus, SliceRecord, SliceStatus};

/// Minimum schema version we know how to handle.
///
/// `0` means "no schema yet" (fresh DB).
const MIN_SUPPORTED_SCHEMA_VERSION: i32 = 0;

/// Latest schema version this crate knows about.
const CURRENT_SCHEMA_VERSION: i32 = 2;

/// Error type for project database operations.
#[derive(Debug, Error)]
pub enum DbError {
    /// Underlying SQLite error.
    #[error("SQLite error: {0}")]
    Sql(#[from] rusqlite::Error),

    /// The database was created with a newer schema version than we support.
    #[error(
        "Unsupported schema version {found}; supported range is {min_supported}..={max_supported}"
    )]
    UnsupportedSchemaVersion { found: i32, min_supported: i32, max_supported: i32 },
}

/// Convenience result type for DB operations.
pub type DbResult<T> = Result<T, DbError>;

/// SQLite-backed project database.
///
/// This is a thin wrapper around `rusqlite::Connection` that is responsible for:
/// - Opening/creating the DB file.
/// - Applying schema migrations.
/// - Providing small, testable helpers for querying and updating records.
#[derive(Debug)]
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
            params![record.name, record.description, record.status.to_i32(),],
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

    /// Insert a ritual run record and return its row id.
    pub fn insert_ritual_run(&self, record: &RitualRunRecord) -> DbResult<i64> {
        self.conn.execute(
            r#"
            INSERT INTO ritual_runs (binary, ritual, spec_hash, binary_hash, status, started_at, finished_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                record.binary,
                record.ritual,
                record.spec_hash,
                record.binary_hash,
                record.status.as_str(),
                record.started_at,
                record.finished_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// List ritual runs, optionally filtered by binary name.
    pub fn list_ritual_runs(&self, binary: Option<&str>) -> DbResult<Vec<RitualRunRecord>> {
        fn map_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<RitualRunRecord> {
            Ok(RitualRunRecord {
                binary: row.get(0)?,
                ritual: row.get(1)?,
                spec_hash: row.get(2)?,
                binary_hash: row.get(3)?,
                status: {
                    let s: String = row.get(4)?;
                    s.parse::<RitualRunStatusString>()?.0
                },
                started_at: row.get(5)?,
                finished_at: row.get(6)?,
            })
        }

        let mut stmt = if binary.is_some() {
            self.conn.prepare(
                r#"
                SELECT binary, ritual, spec_hash, binary_hash, status, started_at, finished_at
                FROM ritual_runs
                WHERE binary = ?1
                ORDER BY id
                "#,
            )?
        } else {
            self.conn.prepare(
                r#"
                SELECT binary, ritual, spec_hash, binary_hash, status, started_at, finished_at
                FROM ritual_runs
                ORDER BY id
                "#,
            )?
        };

        let rows = if let Some(bin) = binary {
            stmt.query_map(params![bin], map_run)?
        } else {
            stmt.query_map([], map_run)?
        };

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Update status (and optionally finished_at) for a ritual run.
    ///
    /// Returns the number of rows affected.
    pub fn update_ritual_run_status(
        &self,
        binary: &str,
        ritual: &str,
        status: &str,
        finished_at: Option<&str>,
    ) -> DbResult<usize> {
        let affected = if let Some(finish) = finished_at {
            self.conn.execute(
                r#"
                UPDATE ritual_runs
                SET status = ?1, finished_at = ?2
                WHERE binary = ?3 AND ritual = ?4
                "#,
                params![status, finish, binary, ritual],
            )?
        } else {
            self.conn.execute(
                r#"
                UPDATE ritual_runs
                SET status = ?1
                WHERE binary = ?2 AND ritual = ?3
                "#,
                params![status, binary, ritual],
            )?
        };
        Ok(affected)
    }
}

/// Apply schema migrations to bring the database to the latest version.
///
/// We use `PRAGMA user_version` as the schema version indicator.
///
/// Version map:
/// - 0: no schema
/// - 1: initial schema (binaries, slices)
/// - 2: add ritual_runs table
fn apply_migrations(conn: &Connection) -> DbResult<()> {
    let current_version = current_schema_version(conn)?;

    // Reject DBs created with a newer schema than we support.
    if current_version > CURRENT_SCHEMA_VERSION {
        return Err(DbError::UnsupportedSchemaVersion {
            found: current_version,
            min_supported: MIN_SUPPORTED_SCHEMA_VERSION,
            max_supported: CURRENT_SCHEMA_VERSION,
        });
    }

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

    if current_version < 2 {
        conn.execute_batch(
            r#"
            BEGIN;
            CREATE TABLE IF NOT EXISTS ritual_runs (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                binary       TEXT NOT NULL,
                ritual       TEXT NOT NULL,
                spec_hash    TEXT NOT NULL,
                binary_hash  TEXT,
                status       TEXT NOT NULL,
                started_at   TEXT NOT NULL,
                finished_at  TEXT NOT NULL
            );

            PRAGMA user_version = 2;
            COMMIT;
            "#,
        )?;
    }

    Ok(())
}

/// Read the SQLite schema version from `PRAGMA user_version`.
fn current_schema_version(conn: &Connection) -> DbResult<i32> {
    let version: i32 = conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
    Ok(version)
}

/// Helper for parsing status strings into RitualRunStatus with better errors.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RitualRunStatusString(pub RitualRunStatus);

impl std::str::FromStr for RitualRunStatusString {
    type Err = rusqlite::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let status = match s.to_lowercase().as_str() {
            "pending" => RitualRunStatus::Pending,
            "running" => RitualRunStatus::Running,
            "succeeded" => RitualRunStatus::Succeeded,
            "failed" => RitualRunStatus::Failed,
            "canceled" => RitualRunStatus::Canceled,
            "stubbed" => RitualRunStatus::Stubbed,
            _other => {
                return Err(rusqlite::Error::InvalidQuery);
            }
        };
        Ok(RitualRunStatusString(status))
    }
}
