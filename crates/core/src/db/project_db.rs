use std::path::Path;

use rusqlite::{params, Connection};
use thiserror::Error;

use crate::db::{BinaryRecord, RitualRunRecord, RitualRunStatus, SliceRecord, SliceStatus};

/// Minimum schema version we know how to handle.
///
/// `0` means "no schema yet" (fresh DB).
const MIN_SUPPORTED_SCHEMA_VERSION: i32 = 0;

/// Latest schema version this crate knows about.
pub const CURRENT_SCHEMA_VERSION: i32 = 8;

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
            INSERT INTO slices (name, description, default_binary, status)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![record.name, record.description, record.default_binary, record.status.to_i32(),],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// List all slices (ordered by id).
    pub fn list_slices(&self) -> DbResult<Vec<SliceRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT name, description, default_binary, status
            FROM slices
            ORDER BY id
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let status_int: i32 = row.get(3)?;
            Ok(SliceRecord {
                name: row.get(0)?,
                description: row.get(1)?,
                default_binary: row.get(2)?,
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
            INSERT INTO ritual_runs (binary, ritual, spec_hash, binary_hash, backend, backend_version, backend_path, status, started_at, finished_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                record.binary,
                record.ritual,
                record.spec_hash,
                record.binary_hash,
                record.backend,
                record.backend_version,
                record.backend_path,
                record.status.as_str(),
                record.started_at,
                record.finished_at
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Persist analysis results for a given ritual run id.
    pub fn insert_analysis_result(
        &self,
        run_id: i64,
        result: &crate::services::analysis::AnalysisResult,
    ) -> DbResult<()> {
        let tx = self.conn.unchecked_transaction()?;

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT OR REPLACE INTO analysis_functions (run_id, address, name, size)
                VALUES (?1, ?2, ?3, ?4)
                "#,
            )?;
            for f in &result.functions {
                stmt.execute(params![run_id, f.address as i64, f.name, f.size.map(|s| s as i64)])?;
            }
        }

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT OR REPLACE INTO analysis_call_edges (run_id, from_addr, to_addr, is_cross_slice)
                VALUES (?1, ?2, ?3, ?4)
                "#,
            )?;
            for e in &result.call_edges {
                stmt.execute(params![
                    run_id,
                    e.from as i64,
                    e.to as i64,
                    if e.is_cross_slice { 1 } else { 0 }
                ])?;
            }
        }

        {
            let mut stmt_block = tx.prepare(
                r#"
                INSERT OR REPLACE INTO analysis_basic_blocks (run_id, start, len)
                VALUES (?1, ?2, ?3)
                "#,
            )?;
            let mut stmt_edge = tx.prepare(
                r#"
                INSERT OR REPLACE INTO analysis_basic_block_edges (run_id, from_start, target, kind)
                VALUES (?1, ?2, ?3, ?4)
                "#,
            )?;
            for bb in &result.basic_blocks {
                stmt_block.execute(params![run_id, bb.start as i64, bb.len as i64])?;
                for succ in &bb.successors {
                    stmt_edge.execute(params![
                        run_id,
                        bb.start as i64,
                        succ.target as i64,
                        format!("{:?}", succ.kind)
                    ])?;
                }
            }
        }

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT OR REPLACE INTO analysis_evidence (run_id, address, description, kind)
                VALUES (?1, ?2, ?3, ?4)
                "#,
            )?;
            for ev in &result.evidence {
                let kind_str = ev.kind.as_ref().map(evidence_kind_to_str);
                stmt.execute(params![run_id, ev.address as i64, ev.description, kind_str])?;
            }
        }

        {
            let mut stmt = tx.prepare(
                r#"
                INSERT OR REPLACE INTO analysis_roots (run_id, idx, root)
                VALUES (?1, ?2, ?3)
                "#,
            )?;
            for (idx, root) in result.roots.iter().enumerate() {
                stmt.execute(params![run_id, idx as i64, root])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Load the most recent run id for a given binary/ritual name.
    pub fn latest_run_id(&self, binary: &str, ritual: &str) -> DbResult<Option<i64>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id FROM ritual_runs
            WHERE binary = ?1 AND ritual = ?2
            ORDER BY id DESC
            LIMIT 1
            "#,
        )?;
        let mut rows = stmt.query(params![binary, ritual])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    /// Load persisted analysis result for a given binary/ritual, if present.
    pub fn load_analysis_result(
        &self,
        binary: &str,
        ritual: &str,
    ) -> DbResult<Option<crate::services::analysis::AnalysisResult>> {
        let run_id = if let Some(id) = self.latest_run_id(binary, ritual)? {
            id
        } else {
            return Ok(None);
        };

        // Functions
        let mut functions = Vec::new();
        {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT address, name, size FROM analysis_functions
                WHERE run_id = ?1
                "#,
            )?;
            let rows = stmt.query_map(params![run_id], |row| {
                Ok(crate::services::analysis::FunctionRecord {
                    address: row.get::<_, i64>(0)? as u64,
                    name: row.get(1)?,
                    size: row.get::<_, Option<i64>>(2)?.map(|v| v as u32),
                    in_slice: true,
                    is_boundary: false,
                })
            })?;
            for r in rows {
                functions.push(r?);
            }
        }

        // Call edges
        let mut call_edges = Vec::new();
        {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT from_addr, to_addr, is_cross_slice FROM analysis_call_edges
                WHERE run_id = ?1
                "#,
            )?;
            let rows = stmt.query_map(params![run_id], |row| {
                Ok(crate::services::analysis::CallEdge {
                    from: row.get::<_, i64>(0)? as u64,
                    to: row.get::<_, i64>(1)? as u64,
                    is_cross_slice: row.get::<_, i64>(2)? != 0,
                })
            })?;
            for r in rows {
                call_edges.push(r?);
            }
        }

        // Basic blocks and edges
        let mut basic_blocks = Vec::new();
        {
            let mut blocks_stmt = self.conn.prepare(
                r#"
                SELECT start, len FROM analysis_basic_blocks
                WHERE run_id = ?1
                "#,
            )?;
            let mut edges_stmt = self.conn.prepare(
                r#"
                SELECT from_start, target, kind FROM analysis_basic_block_edges
                WHERE run_id = ?1
                "#,
            )?;

            let edge_rows: Vec<(u64, crate::services::analysis::BlockEdge)> = edges_stmt
                .query_map(params![run_id], |row| {
                    Ok((
                        row.get::<_, i64>(0)? as u64,
                        crate::services::analysis::BlockEdge {
                            target: row.get::<_, i64>(1)? as u64,
                            kind: parse_edge_kind(row.get::<_, String>(2)?.as_str()),
                        },
                    ))
                })?
                .collect::<Result<_, _>>()?;

            let rows = blocks_stmt.query_map(params![run_id], |row| {
                Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u32))
            })?;
            for r in rows {
                let (start, len) = r?;
                let successors = edge_rows
                    .iter()
                    .filter_map(
                        |(from, edge)| if *from == start { Some(edge.clone()) } else { None },
                    )
                    .collect();
                basic_blocks.push(crate::services::analysis::BasicBlock { start, len, successors });
            }
        }

        // Evidence
        let mut evidence = Vec::new();
        {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT address, description, kind FROM analysis_evidence
                WHERE run_id = ?1
                "#,
            )?;
            let rows = stmt.query_map(params![run_id], |row| {
                Ok(crate::services::analysis::EvidenceRecord {
                    address: row.get::<_, i64>(0)? as u64,
                    description: row.get(1)?,
                    kind: parse_evidence_kind(row.get::<_, Option<String>>(2)?),
                })
            })?;
            for r in rows {
                evidence.push(r?);
            }
        }

        // Roots
        let mut roots = Vec::new();
        {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT root FROM analysis_roots
                WHERE run_id = ?1
                ORDER BY idx
                "#,
            )?;
            let rows = stmt.query_map(params![run_id], |row| row.get::<_, String>(0))?;
            for r in rows {
                roots.push(r?);
            }
        }

        let (backend_version, backend_path) = self.conn.query_row(
            "SELECT backend_version, backend_path FROM ritual_runs WHERE id = ?1",
            params![run_id],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
        )?;

        Ok(Some(crate::services::analysis::AnalysisResult {
            functions,
            call_edges,
            evidence,
            basic_blocks,
            roots,
            backend_version,
            backend_path,
        }))
    }
    /// List ritual runs, optionally filtered by binary name.
    pub fn list_ritual_runs(&self, binary: Option<&str>) -> DbResult<Vec<RitualRunRecord>> {
        fn map_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<RitualRunRecord> {
            Ok(RitualRunRecord {
                binary: row.get(0)?,
                ritual: row.get(1)?,
                spec_hash: row.get(2)?,
                binary_hash: row.get(3)?,
                backend: row.get(4)?,
                backend_version: row.get(5).ok(),
                backend_path: row.get(6).ok(),
                status: {
                    let s: String = row.get(7)?;
                    s.parse::<RitualRunStatusString>()?.0
                },
                started_at: row.get(8)?,
                finished_at: row.get(9)?,
            })
        }

        let mut stmt = if binary.is_some() {
            self.conn.prepare(
                r#"
                SELECT binary, ritual, spec_hash, binary_hash, backend, backend_version, backend_path, status, started_at, finished_at
                FROM ritual_runs
                WHERE binary = ?1
                ORDER BY id
                "#,
            )?
        } else {
            self.conn.prepare(
                r#"
                SELECT binary, ritual, spec_hash, binary_hash, backend, backend_version, backend_path, status, started_at, finished_at
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
/// - 3: add backend column to ritual_runs
/// - 4: add backend_version, backend_path to ritual_runs
/// - 5: add analysis tables (functions, call edges, basic blocks, evidence)
/// - 6: add default_binary column to slices (guarded in code)
/// - 7: add evidence kind column
/// - 8: add analysis_roots table for persisted roots per run
fn apply_migrations(conn: &Connection) -> DbResult<()> {
    let mut current_version = current_schema_version(conn)?;

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
                default_binary TEXT,
                status      INTEGER NOT NULL
            );

            PRAGMA user_version = 1;
            COMMIT;
            "#,
        )?;
        current_version = 1;
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
        current_version = 2;
    }

    if current_version < 3 {
        conn.execute_batch(
            r#"
            BEGIN;
            ALTER TABLE ritual_runs ADD COLUMN backend TEXT NOT NULL DEFAULT 'validate-only';
            -- existing rows get default; future inserts should provide backend
            UPDATE ritual_runs SET backend = 'validate-only' WHERE backend IS NULL;
            PRAGMA user_version = 3;
            COMMIT;
            "#,
        )?;
        current_version = 3;
    }

    if current_version < 4 {
        conn.execute_batch(
            r#"
            BEGIN;
            ALTER TABLE ritual_runs ADD COLUMN backend_version TEXT;
            ALTER TABLE ritual_runs ADD COLUMN backend_path TEXT;
            PRAGMA user_version = 4;
            COMMIT;
            "#,
        )?;
        current_version = 4;
    }

    if current_version < 5 {
        conn.execute_batch(
            r#"
            BEGIN;
            CREATE TABLE IF NOT EXISTS analysis_functions (
                run_id  INTEGER NOT NULL,
                address INTEGER NOT NULL,
                name    TEXT,
                size    INTEGER,
                PRIMARY KEY(run_id, address)
            );
            CREATE TABLE IF NOT EXISTS analysis_call_edges (
                run_id         INTEGER NOT NULL,
                from_addr      INTEGER NOT NULL,
                to_addr        INTEGER NOT NULL,
                is_cross_slice INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS analysis_basic_blocks (
                run_id INTEGER NOT NULL,
                start  INTEGER NOT NULL,
                len    INTEGER NOT NULL,
                PRIMARY KEY(run_id, start)
            );
            CREATE TABLE IF NOT EXISTS analysis_basic_block_edges (
                run_id    INTEGER NOT NULL,
                from_start INTEGER NOT NULL,
                target    INTEGER NOT NULL,
                kind      TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS analysis_evidence (
                run_id     INTEGER NOT NULL,
                address    INTEGER NOT NULL,
                description TEXT NOT NULL,
                kind        TEXT
            );
            PRAGMA user_version = 5;
            COMMIT;
            "#,
        )?;
        current_version = 5;
    }

    if current_version < 6 {
        let has_column = column_exists(conn, "slices", "default_binary")?;
        conn.execute_batch(
            r#"
            BEGIN;
            -- add column if absent; guarded in Rust code to avoid duplicates
            PRAGMA user_version = 6;
            COMMIT;
            "#,
        )?;
        if !has_column {
            conn.execute("ALTER TABLE slices ADD COLUMN default_binary TEXT;", [])?;
        }
    }

    if current_version < 7 {
        let has_kind = column_exists(conn, "analysis_evidence", "kind")?;
        if !has_kind {
            conn.execute("ALTER TABLE analysis_evidence ADD COLUMN kind TEXT;", [])?;
        }
        conn.execute("PRAGMA user_version = 7;", [])?;
    }

    if current_version < 8 {
        conn.execute_batch(
            r#"
            BEGIN;
            CREATE TABLE IF NOT EXISTS analysis_roots (
                run_id INTEGER NOT NULL,
                idx    INTEGER NOT NULL,
                root   TEXT NOT NULL,
                PRIMARY KEY(run_id, idx)
            );
            PRAGMA user_version = 8;
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

fn column_exists(conn: &Connection, table: &str, column: &str) -> DbResult<bool> {
    let pragma = format!("PRAGMA table_info({table});");
    let mut stmt = conn.prepare(&pragma)?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for name in rows {
        if name? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn parse_edge_kind(kind: &str) -> crate::services::analysis::BlockEdgeKind {
    match kind {
        "Jump" | "jump" => crate::services::analysis::BlockEdgeKind::Jump,
        "ConditionalJump" | "cjump" => crate::services::analysis::BlockEdgeKind::ConditionalJump,
        "IndirectJump" | "ijump" => crate::services::analysis::BlockEdgeKind::IndirectJump,
        "Call" | "call" => crate::services::analysis::BlockEdgeKind::Call,
        "IndirectCall" | "icall" => crate::services::analysis::BlockEdgeKind::IndirectCall,
        _ => crate::services::analysis::BlockEdgeKind::Fallthrough,
    }
}

fn evidence_kind_to_str(kind: &crate::services::analysis::EvidenceKind) -> &'static str {
    match kind {
        crate::services::analysis::EvidenceKind::String => "string",
        crate::services::analysis::EvidenceKind::Import => "import",
        crate::services::analysis::EvidenceKind::Call => "call",
        crate::services::analysis::EvidenceKind::Other => "other",
    }
}

fn parse_evidence_kind(kind: Option<String>) -> Option<crate::services::analysis::EvidenceKind> {
    match kind.as_deref() {
        Some("string") => Some(crate::services::analysis::EvidenceKind::String),
        Some("import") => Some(crate::services::analysis::EvidenceKind::Import),
        Some("call") => Some(crate::services::analysis::EvidenceKind::Call),
        Some("other") => Some(crate::services::analysis::EvidenceKind::Other),
        _ => None,
    }
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
