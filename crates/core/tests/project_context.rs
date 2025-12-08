use ritual_core::db::{ProjectConfig, ProjectContext, ProjectLayout};

#[test]
fn project_context_loads_config_and_db() {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProjectLayout::new(temp.path());
    std::fs::create_dir_all(&layout.meta_dir).unwrap();

    let config = ProjectConfig::new("CtxProject", layout.db_path_relative_string());
    std::fs::write(&layout.project_config_path, serde_json::to_string_pretty(&config).unwrap())
        .unwrap();

    let ctx = ProjectContext::from_root(temp.path()).expect("context");
    assert_eq!(ctx.config.name, "CtxProject");
    assert!(ctx.db_path.is_file());

    // DB should be initialized and usable.
    ctx.db.list_binaries().expect("list binaries");
}
