use ritual_core::db::{BinaryRecord, ProjectDb, SliceRecord, SliceStatus};
use tempfile::tempdir;

#[test]
fn project_db_initializes_and_handles_binaries_and_slices() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("project.db");

    // First open should create schema and allow inserts.
    {
        let db = ProjectDb::open(&db_path).expect("open db");
        let conn = db.connection();

        // Check that user_version is set to 1.
        let version: i32 = conn
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("schema version");
        assert_eq!(version, 1);

        // Insert a binary.
        let bin = BinaryRecord::new("libCQ2Client.so", "binaries/libCQ2Client.so");
        let id = db.insert_binary(&bin).expect("insert binary");
        assert!(id > 0);

        let binaries = db.list_binaries().expect("list binaries");
        assert_eq!(binaries.len(), 1);
        assert_eq!(binaries[0].name, bin.name);
        assert_eq!(binaries[0].path, bin.path);

        // Insert a slice.
        let slice = SliceRecord::new("AutoUpdateManager", SliceStatus::Active);
        let sid = db.insert_slice(&slice).expect("insert slice");
        assert!(sid > 0);

        let slices = db.list_slices().expect("list slices");
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].name, slice.name);
        assert_eq!(slices[0].status, SliceStatus::Active);
    }

    // Second open should see existing schema and data.
    {
        let db = ProjectDb::open(&db_path).expect("re-open db");
        let conn = db.connection();

        let version: i32 = conn
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .expect("schema version");
        assert_eq!(version, 1);

        let binaries = db.list_binaries().expect("list binaries");
        assert_eq!(binaries.len(), 1);
        assert_eq!(binaries[0].name, "libCQ2Client.so");

        let slices = db.list_slices().expect("list slices");
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0].name, "AutoUpdateManager");
    }
}
