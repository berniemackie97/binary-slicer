use rusqlite::Connection;
use tempfile::tempdir;

use ritual_core::db::{ProjectDb, RitualRunRecord};

#[test]
fn ritual_runs_insert_and_list_round_trip() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("project.db");

    let db = ProjectDb::open(&db_path).expect("open db");

    // Insert two runs for two binaries.
    let run_a = RitualRunRecord {
        binary: "BinA".into(),
        ritual: "Run1".into(),
        spec_hash: "specA".into(),
        binary_hash: Some("binhashA".into()),
        status: "stubbed".into(),
        started_at: "t0".into(),
        finished_at: "t1".into(),
    };
    let run_b = RitualRunRecord {
        binary: "BinB".into(),
        ritual: "Run2".into(),
        spec_hash: "specB".into(),
        binary_hash: None,
        status: "stubbed".into(),
        started_at: "t2".into(),
        finished_at: "t3".into(),
    };

    db.insert_ritual_run(&run_a).expect("insert run a");
    db.insert_ritual_run(&run_b).expect("insert run b");

    let all = db.list_ritual_runs(None).expect("list runs");
    assert_eq!(all.len(), 2);

    let only_a = db.list_ritual_runs(Some("BinA")).expect("filter runs");
    assert_eq!(only_a.len(), 1);
    assert_eq!(only_a[0].binary, "BinA");
    assert_eq!(only_a[0].binary_hash.as_deref(), Some("binhashA"));

    // Update status and finished_at.
    let updated = db
        .update_ritual_run_status("BinA", "Run1", "succeeded", Some("t9"))
        .expect("update status");
    assert_eq!(updated, 1);
    let only_a_after = db.list_ritual_runs(Some("BinA")).expect("filter runs");
    assert_eq!(only_a_after[0].status, "succeeded");
    assert_eq!(only_a_after[0].finished_at, "t9");
}

#[test]
fn existing_schema_is_migrated_to_v2() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("project.db");

    // Create a v1-like schema manually, set user_version = 1.
    {
        let conn = Connection::open(&db_path).expect("open sqlite");
        conn.execute_batch(
            r#"
            BEGIN;
            CREATE TABLE binaries (
                id   INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                arch TEXT,
                hash TEXT
            );
            CREATE TABLE slices (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT NOT NULL UNIQUE,
                description TEXT,
                status      INTEGER NOT NULL
            );
            PRAGMA user_version = 1;
            COMMIT;
            "#,
        )
        .expect("create v1 schema");
    }

    // Opening via ProjectDb should migrate to v2 and create ritual_runs table.
    let db = ProjectDb::open(&db_path).expect("open and migrate");
    let version: i32 =
        db.connection().query_row("PRAGMA user_version;", [], |row| row.get(0)).unwrap();
    assert_eq!(version, 2);

    // Table should accept inserts post-migration.
    let run = RitualRunRecord {
        binary: "BinA".into(),
        ritual: "RunX".into(),
        spec_hash: "sh".into(),
        binary_hash: None,
        status: "ok".into(),
        started_at: "t".into(),
        finished_at: "t".into(),
    };
    db.insert_ritual_run(&run).expect("insert after migration");
    let all = db.list_ritual_runs(None).expect("list runs");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].status, "ok");
}
