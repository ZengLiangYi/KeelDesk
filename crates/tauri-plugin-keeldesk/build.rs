const COMMANDS: &[&str] = &[
    "get_keel_runtime_snapshot",
    "self_test_keel_ping",
    "resolve_keel_tool_approval",
    "clear_keel_session_allowlist",
    "get_keel_odr_status",
    "register_keel_with_odr",
    "unregister_keel_from_odr",
    "export_keel_odr_manifest",
    "get_keel_process_host_snapshot",
    "start_keel_child_server",
    "stop_keel_child_server",
    "remove_keel_child_server",
    "check_keel_healthz",
    "get_keel_protocol_traces",
    "clear_keel_protocol_traces",
    "call_keel_child_server_tool",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();

    // Tauri's Windows dependencies (e.g. TaskDialogIndirect / subclassing) require
    // Common-Controls v6 in the executable manifest. Host apps embed this via
    // `tauri-build`, while standalone plugin test binaries on MSVC need it for tests.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let manifest_path = out_dir.join("windows-test-manifest.xml");
        std::fs::write(
            &manifest_path,
            r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
</assembly>
"#,
        )
        .expect("failed to write windows test manifest");
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
            manifest_path.display()
        );
    }
}
