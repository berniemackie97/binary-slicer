use std::fs;
use std::path::Path;

use binary_slicer::{canonicalize_or_current, infer_project_name};
use tempfile::tempdir;

#[test]
fn canonicalize_or_current_returns_cwd_for_dot() {
    let original = std::env::current_dir().expect("cwd");
    let tmp = tempdir().expect("tempdir");
    std::env::set_current_dir(tmp.path()).expect("chdir tmp");

    let result = canonicalize_or_current(".").expect("canonicalize").canonicalize().expect("canon");
    let expected = tmp.path().canonicalize().expect("canon tmp");
    assert_eq!(result, expected);

    std::env::set_current_dir(original).expect("restore cwd");
}

#[test]
fn canonicalize_or_current_resolves_existing_relative_path() {
    let original = std::env::current_dir().expect("cwd");
    let tmp = tempdir().expect("tempdir");
    let subdir = tmp.path().join("nested");
    fs::create_dir_all(&subdir).expect("create nested");
    std::env::set_current_dir(tmp.path()).expect("chdir tmp");

    let result = canonicalize_or_current("nested").expect("canonicalize nested");
    assert_eq!(result, subdir.canonicalize().expect("canonicalize subdir"));

    std::env::set_current_dir(original).expect("restore cwd");
}

#[test]
fn infer_project_name_uses_last_path_component() {
    assert_eq!(infer_project_name(Path::new("C:/work/binary-slicer")), "binary-slicer");
    assert_eq!(infer_project_name(Path::new("/tmp/project-root")), "project-root");
}

#[test]
fn infer_project_name_falls_back_when_missing() {
    assert_eq!(infer_project_name(Path::new("/")), "unnamed-project");
}
