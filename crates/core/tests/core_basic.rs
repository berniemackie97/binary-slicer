use ritual_core::{analysis, version};

#[test]
fn version_is_non_empty() {
    let v = version();
    assert!(!v.is_empty());
}

#[test]
fn hello_slice_returns_stub_result() {
    let result = analysis::hello_slice("TestSlice");
    assert_eq!(result.slice.name, "TestSlice");
    assert_eq!(result.functions.len(), 1);
    assert_eq!(result.functions[0].name, "root_function_stub");
}
