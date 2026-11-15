# `keeldesk-core`

Pure Rust embedded MCP (Model Context Protocol) runtime spine for Windows 11 desktop applications—built on `rmcp` v3.10 and `axum` v0.8 with **zero `tauri` UI dependencies**.

Part of the [**KeelDesk**](../../README.md) workspace monorepo.

## Responsibilities

- **Embedded Streamable HTTP MCP Server (`mcp_server`)**: Binds strictly to `127.0.0.1` (`http://127.0.0.1:19527/mcp` by default, configurable via `KEELDESK_MCP_PORT`), provides `/healthz`, registers `keel_ping`, and enforces strict loopback `Host` and `Origin` allowlists against DNS rebinding.
- **Safety Registry & Human-in-the-Loop Gate (`runtime`)**: Classifies tools as `Safe` or `ConfirmRequired`. `ConfirmRequired` calls suspend on a `tokio::sync::oneshot` channel supporting **Approve Once**, **Allow Session**, **Deny**, timeout auto-denial, and caller cancellation (`RequestContext.ct`) cleanup.
- **Host Callback Injection (`KeelRuntimeOptions` & `RuntimeTaskSpawner`)**: Decouples window wakeup, snapshot telemetry, process-host updates, and protocol trace dispatch from any specific GUI framework.
- **Windows 11 ODR Integration (`win_odr`, `win32_env`)**: Probes OS build, Developer Mode, and `odr.exe` readiness, manages URI/manifest enrollment, and generates live MCPB `"0.3"` manifests (`_meta["com.microsoft.windows"].static_responses`).
- **Supervised Child MCP Process Host (`process_host`)**: Spawns external stdio MCP servers (`npx`, `uvx`, `.cmd`, executables) with `CREATE_NO_WINDOW`, merged `HKLM`/`HKCU` Registry `PATH`, Win32 Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) tree reclamation, `rmcp` stdio handshake/tool calls, redacted `env_keys`, and bounded `stderr_tail` diagnostics.
- **Bounded Protocol Telemetry (`devtools`)**: In-memory JSON-RPC 2.0 ring buffer (`MAX_PROTOCOL_TRACES = 128`, `16 KiB` UTF-8 safe payload cap) with `approval_wait_ms` vs `execution_ms` latency breakdowns and Streamable HTTP session tracking.

## Architectural Boundary

`keeldesk-core` must **never** depend on `tauri`, `tauri-specta`, or `apps/env-workbench` business modules.

```bash
cargo tree -p keeldesk-core
cargo test -p keeldesk-core
```

## License

[MIT](../../LICENSE)
