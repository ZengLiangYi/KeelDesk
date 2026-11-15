# Changelog

All notable changes to **KeelDesk** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Phase 5 — Cargo + `pnpm` Workspace Monorepo Modularization**:
  - **Cargo Workspace (`Cargo.toml`)**:
    - `crates/keeldesk-core/`: Pure Rust MCP runtime spine with zero `tauri` UI dependencies, introducing `KeelRuntimeOptions` and `RuntimeTaskSpawner` for host window wakeup and telemetry dispatch.
    - `crates/tauri-plugin-keeldesk/`: Standard Tauri v2 plugin (`plugin:keeldesk`) exposing namespaced Specta-typed commands, events, capability permissions, and graceful runtime shutdown.
    - `apps/env-workbench/src-tauri/`: Desktop workbench application crate (`keeldesk`) integrating `keeldesk-core` and `tauri-plugin-keeldesk` with Win11 host inspection tools.
  - **`pnpm` Workspace (`pnpm-workspace.yaml`)**:
    - `packages/react/` (`@keeldesk/react`): Standalone React 19 SDK + `<KeelDevTools />` package compiling `.js` + `.d.ts` into `packages/react/dist/` with isolated `packages/react/src/bindings.ts` and zero `apps/env-workbench` dependencies.
    - `apps/env-workbench/` (`@keeldesk/env-workbench`): Desktop workbench frontend consuming `@keeldesk/react` (`workspace:*`), preserving `apps/env-workbench/src/bindings.ts` for workbench commands, and configuring Tailwind CSS v4 `@source` scanning for `packages/react/src/`.
  - **CI & Release Workflows (`.github/workflows/`)**: Updated `ci.yml` and `release.yml` for workspace-wide Rust/TypeScript checks, split bindings drift verification, and Tauri bundling from `apps/env-workbench`.
- **Phase 4 — Standardized React SDK & `<KeelDevTools />` Protocol Inspector**:
  - `keeldesk_core::devtools`: Bounded JSON-RPC protocol trace store (`MAX_PROTOCOL_TRACES = 128`, `16 KiB` payload cap) recording Streamable HTTP and stdio child MCP frames with `approval_wait_ms` vs `execution_ms` latency breakdowns (`McpLatencyBreakdown`) and linked call IDs.
  - Runtime & Child Tool Telemetry (`keeldesk_core::mcp_server`, `keeldesk_core::process_host`, `tauri_plugin_keeldesk::commands`): Streamable HTTP session tracking (`McpSessionSummary`), `/healthz` readiness probe (`check_keel_healthz`), protocol trace query/clear commands, and interactive child stdio tool invocation (`call_keel_child_server_tool`).
  - Standardized Frontend SDK (`@keeldesk/react`): `useMcpServerState`, `useToolApproval`, `useExternalMcpClients`, and `useKeelDevTools` hooks plus the collapsible `<KeelDevTools />` inspector drawer (`Ctrl+\``) featuring split request/response `CodeWell`s, latency breakdown bars, session inspection, and `/healthz` probing.
  - Workbench Integration (`apps/env-workbench/src/App.tsx`, `ProcessHostPanel.tsx`): Top command bar active session pill, clickable live call stream entries jumping straight to matching JSON-RPC frames, bottom status-bar `<KeelDevTools />` dock toggle, and inline child MCP tool test-call (`试调`) with `查看报文 #seq` trace jump.
- **Phase 3 — Windows 11 ODR & Supervised Child MCP Process Host**:
  - `keeldesk_core::win32_env`: Shared Win32 `HKLM`/`HKCU` Registry `PATH` reconstruction, `PATHEXT` executable resolution, `CREATE_NO_WINDOW` command builder, and Windows 11 build/UBR/Developer Mode detection.
  - `keeldesk_core::win_odr`: Windows 11 `odr.exe` readiness probing, URI/manifest registration & unregistration, and live MCPB `"0.3"` manifest generation (`_meta["com.microsoft.windows"].static_responses`) with local JSON export fallback.
  - `keeldesk_core::process_host`: Supervised child stdio MCP process host featuring `CREATE_NO_WINDOW`, Win32 Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) tree reclamation, Registry `PATH` injection, `rmcp` `initialize`/`list_tools` handshake discovery, redacted `env_keys`, and bounded `stderr_tail` capture.
  - Frontend SDK & Workbench UI: `useWinOdr` and `useProcessHost` hooks (`@keeldesk/react`), `Windows ODR` integration snippet tab, `OdrManifestSheet`, and `ProcessHostPanel` (`apps/env-workbench/src/components/`).
- Bilingual English and Simplified Chinese documentation (`README.md`, `README.zh-CN.md`, `docs/ROADMAP.md`, `docs/ROADMAP.zh-CN.md`).
- Community governance and security files (`CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `CHANGELOG.md`).
- GitHub Issue Forms, Pull Request template, and GitHub Actions CI & Windows release workflows (`.github/`).

## [0.1.0] - 2026-11-14

### Added
- **Runtime Spine (`keel_core`)**:
  - Embedded `rmcp` v3.10 + `axum` v0.8 Streamable HTTP MCP server bound to `127.0.0.1:19527/mcp` (overridable via `KEELDESK_MCP_PORT`) with `/healthz` readiness probe.
  - Strict loopback `Host` and `Origin` allowlist checks to block DNS-rebinding requests.
  - `KeelToolRegistry` with `Safe` and `ConfirmRequired` safety tiers and built-in `keel_ping` loopback diagnostic tool.
  - Asynchronous human-in-the-loop approval interceptor (`tokio::sync::oneshot`) supporting **Approve Once**, **Allow Session**, **Deny**, timeout auto-denial, caller cancellation cleanup, and system-tray window wakeup.
  - Embedded SChannel TLS (`reqwest` `native-tls`) loopback client for self-tests and dry-run drills.
- **Windows 11 Workbench & Native Integration (`workbench`, `win11.rs`, `tray.rs`)**:
  - Single-instance system tray daemon (`tauri-plugin-single-instance`) with hide-to-tray window close behavior and clean tray `Quit` shutdown.
  - Authentic Windows 11 native window chrome with `DwmSetWindowAttribute` dark titlebar theming (`DWMWA_CAPTION_COLOR` pixel-locked to `#111116`) and Registry build detection.
  - `scan_dev_env` (`Safe`) concurrent toolchain probe (`git`, `node`, `pnpm`, `rustc`, `cargo`, `python`, `uv`, `pwsh`, `winget`, `wsl`) using `CREATE_NO_WINDOW`, UTF-16LE output decoding, and `WindowsApps` alias filtering.
  - `list_active_ports` (`Safe`) TCPv4/TCPv6 listener enumeration via Win32 `GetExtendedTcpTable` and `QueryFullProcessImageNameW`.
  - `kill_port_process` (`ConfirmRequired`) guarded process termination with `expected_pid` TOCTOU validation, self/system PID protection, and `dry_run` simulation mode.
- **Frontend SDK & "The Graphite Spine" Design System**:
  - `tauri-specta` (`=2.0.0-rc.25`) typed IPC contract generation (`bindings.ts`).
  - `useKeelRuntime` and `useToolApproval` React 19 hooks.
  - Single-track dark graphite token engine, six-step desktop typography, `4px` spacing / `12px` plumb-line alignment, concentric `2px` border radii, `100dvh` constrained viewport with `<ScrollArea>`, `ui/` (`shadcn/ui`) + `ds/` domain components, `ToolApprovalDialog`, `PortsGovernancePanel`, `DevEnvPanel`, and synchronized two-stage boot skeletons (`index.html` + `WorkbenchSkeleton.tsx`).

[Unreleased]: https://github.com/ZengLiangYi/KeelDesk/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ZengLiangYi/KeelDesk/releases/tag/v0.1.0
