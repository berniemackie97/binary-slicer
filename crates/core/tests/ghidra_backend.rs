#![cfg(feature = "ghidra-backend")]

use ritual_core::services::analysis::default_backend_registry;

#[test]
fn ghidra_backend_is_registered_when_feature_enabled() {
    let registry = default_backend_registry();
    assert!(
        registry.names().contains(&"ghidra".to_string()),
        "ghidra backend should be registered when feature is enabled"
    );
}
