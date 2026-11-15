# Changelog

All notable changes to **KeelDesk** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Phase 3 — Windows 11 ODR & Supervised Child MCP Process Host**:
  - `keel_core::win32_env`: Shared Win32 `HKLM`/`HKCU` Registry `PATH` reconstruction, `PATHEXT` executable resolution, `CREATE_NO_WINDOW` command builder, and Windows 11 build/UBR/Developer Mode detection.
  - `keel_core::win_odr`: Windows 11 `odr.exe` readiness probing, URI/manifest registration & unregistration, and live MCPB `"0.3"` manifest generation (`_meta["com.microsoft.windows"].static_responses`) with local JSON export fallback.
  - `keel_core::process_host`: Supervised child stdio MCP process host featuring `CREATE_NO_WINDOW`, Win32 Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) tree reclamation, Registry `PATH` injection, `rmcp` `initialize`/`list_tools` handshake discovery, redacted `env_keys`, and bounded `stderr_tail` capture.
  - Frontend SDK & Workbench UI: `useWinOdr` and `useProcessHost` hooks (`src/keel-sdk/`), `Windows ODR` integration snippet tab, `OdrManifestSheet`, and `ProcessHostPanel` (`src/components/`).
- Bilingual English and Simplified Chinese documentation (`README.md`, `README.zh-CN.md`, `docs/ROADMAP.md`, `docs/ROADMAP.zh-CN.md`).
- Community governance and security files (`CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, `CHANGELOG.md`).
- GitHub Issue Forms, Pull Request template, and GitHub Actions CI & Windows release workflows (`.github/`).

## [0.1.0] - 2026-11-14

### Added
- **Runtime Spine (`src-tauri/src/keel_core/`)**:
  - Embedded `rmcp` v3.10 + `axum` v0.8 Streamable HTTP MCP server bound to `127.0.0.1:19527/mcp` (overridable via `KEELDESK_MCP_PORT`) with `/healthz` readiness probe.
  - Strict loopback `Host` and `Origin` allowlist checks to block DNS-rebinding requests.
  - `KeelToolRegistry` with `Safe` and `ConfirmRequired` safety tiers and built-in `keel_ping` loopback diagnostic tool.
  - Asynchronous human-in-the-loop approval interceptor (`tokio::sync::oneshot`) supporting **Approve Once**, **Allow Session**, **Deny**, timeout auto-denial, caller cancellation cleanup, and system-tray window wakeup.
  - Embedded SChannel TLS (`reqwest` `native-tls`) loopback client for self-tests and dry-run drills.
- **Windows 11 Workbench & Native Integration (`src-tauri/src/workbench/`, `win11.rs`, `tray.rs`)**:
  - Single-instance system tray daemon (`tauri-plugin-single-instance`) with hide-to-tray window close behavior and clean tray `Quit` shutdown.
  - Authentic Windows 11 native window chrome with `DwmSetWindowAttribute` dark titlebar theming (`DWMWA_CAPTION_COLOR` pixel-locked to `#111116`) and Registry build detection.
  - `scan_dev_env` (`Safe`) concurrent toolchain probe (`git`, `node`, `pnpm`, `rustc`, `cargo`, `python`, `uv`, `pwsh`, `winget`, `wsl`) using `CREATE_NO_WINDOW`, UTF-16LE output decoding, and `WindowsApps` alias filtering.
  - `list_active_ports` (`Safe`) TCPv4/TCPv6 listener enumeration via Win32 `GetExtendedTcpTable` and `QueryFullProcessImageNameW`.
  - `kill_port_process` (`ConfirmRequired`) guarded process termination with `expected_pid` TOCTOU validation, self/system PID protection, and `dry_run` simulation mode.
- **Frontend SDK & "The Graphite Spine" Design System (`src/keel-sdk/`, `src/styles/tokens.css`, `src/components/`)**:
  - `tauri-specta` (`=2.0.0-rc.25`) typed IPC contract generation (`src/bindings.ts`).
  - `useKeelRuntime` and `useToolApproval` React 19 hooks.
  - Single-track dark graphite token engine, six-step desktop typography, `4px` spacing / `12px` plumb-line alignment, concentric `2px` border radii, `100dvh` constrained viewport with `<ScrollArea>`, `ui/` (`shadcn/ui`) + `ds/` domain components, `ToolApprovalDialog`, `PortsGovernancePanel`, `DevEnvPanel`, and synchronized two-stage boot skeletons (`index.html` + `WorkbenchSkeleton.tsx`).

[Unreleased]: https://github.com/ZengLiangYi/KeelDesk/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ZengLiangYi/KeelDesk/releases/tag/v0.1.0
