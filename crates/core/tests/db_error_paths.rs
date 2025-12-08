// crates/core/tests/db_error_paths.rs

use ritual_core::db::{DbError, ProjectDb, ProjectLayout};
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn project_db_open_errors_on_unsupported_schema_version() {
    // Arrange: temp project layout + DB with an unsupported user_version.
    let tmp = tempdir().expect("temp dir");
    let layout = ProjectLayout::new(tmp.path());

    // Ensure the .ritual directory exists.
    std::fs::create_dir_all(&layout.meta_dir).expect("create .ritual dir");

    // Manually create a DB and set user_version higher than we support.
    {
        let conn = Connection::open(&layout.db_path).expect("open raw sqlite db");
        conn.pragma_update(None, "user_version", 99_i32).expect("set user_version pragma");
    }

    // Act: attempt to open via ProjectDb::open.
    let open_result = ProjectDb::open(&layout.db_path);

    match open_result {
        Err(DbError::UnsupportedSchemaVersion { found, min_supported, max_supported }) => {
            assert_eq!(found, 99, "unexpected found schema version");
            assert_eq!(min_supported, 0, "unexpected min_supported schema version");
            assert_eq!(max_supported, 2, "unexpected max_supported schema version");
        }
        Err(err) => {
            panic!("expected UnsupportedSchemaVersion error, got different DbError: {err}");
        }
        Ok(_) => {
            panic!("expected UnsupportedSchemaVersion error, got Ok(_)");
        }
    }
}
