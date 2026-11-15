use tauri_plugin_keeldesk::{export_typescript_bindings_to, PLUGIN_NAME};

#[test]
fn specta_builder_exports_namespaced_plugin_commands_and_events_only() {
    assert_eq!(PLUGIN_NAME, "keeldesk");

    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!("keel_plugin_specta_test_{nonce}"));
    let out_file = temp_dir.join("bindings.ts");

    export_typescript_bindings_to(&out_file);
    let content = std::fs::read_to_string(&out_file).expect("bindings.ts should be written");

    assert!(content.contains("plugin:keeldesk|get_keel_runtime_snapshot"));
    assert!(content.contains("plugin:keeldesk|call_keel_child_server_tool"));
    assert!(
        content.contains("plugin:keeldesk:keel-runtime-updated-event")
            || content.contains("plugin:keeldesk:")
    );
    assert!(!content.contains("Win11HostSummary"));
    assert!(!content.contains("ActivePortEntry"));

    let _ = std::fs::remove_dir_all(&temp_dir);
}
