<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="public/logo-readme-dark.svg">
    <img src="public/logo.svg" alt="KeelDesk logo" width="104">
  </picture>
</p>

<h1 align="center">KeelDesk</h1>

<p align="center">
  Native Windows 11 desktop MCP (Model Context Protocol) runtime spine and developer environment workbench.
</p>

<p align="center">
  <a href="https://github.com/ZengLiangYi/KeelDesk/actions/workflows/ci.yml"><img src="https://github.com/ZengLiangYi/KeelDesk/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-7c66ff.svg" alt="License: MIT"></a>
  <img src="https://img.shields.io/badge/Platform-Windows%2011%20(Build%2022000%2B)-0078D4.svg" alt="Platform: Windows 11 (Build 22000+)">
  <img src="https://img.shields.io/badge/Stack-Tauri%20v2%20%C2%B7%20Rust%20%C2%B7%20React%2019-18181b.svg" alt="Stack: Tauri v2 · Rust · React 19">
</p>

<p align="center">
  <strong>English</strong> · <a href="README.zh-CN.md">简体中文</a>
</p>

---

## Overview & Architecture

**KeelDesk** separates its embedded MCP runtime spine from the Windows 11 host workbench through a strict one-way dependency boundary:

- **Keel Spine (`src-tauri/src/keel_core/` + `src/keel-sdk/`)**: Provides the embedded `rmcp` + `axum` Streamable HTTP MCP server, loopback DNS-rebinding defense, `Safe` / `ConfirmRequired` tool safety registry, `tokio::sync::oneshot` human-in-the-loop approval gate, strongly typed `tauri-specta` IPC contracts, live telemetry event stream, and React state hooks. **`keel_core` and `keel-sdk` never import or depend on `workbench` modules.**
- **Workbench Application (`src-tauri/src/workbench/` + `src/App.tsx` + `src/components/`)**: Attaches Windows 11 host-inspection and port-governance tools to `KeelToolRegistry` and renders the native desktop UI (DWM immersive dark caption bar, Snap Layouts support, `shadcn/ui` + `ds/` domain components, and a two-stage zero-flash boot skeleton).

## Highlights

- **Embedded Streamable HTTP MCP Server**: Listens on `http://127.0.0.1:19527/mcp` by default (configurable via `KEELDESK_MCP_PORT`) with an integrated `/healthz` readiness endpoint.
- **Single-Instance System Tray Daemon**: Enforced via `tauri-plugin-single-instance`. Closing the main window hides it to the Windows 11 system tray while keeping the MCP listener online; only the tray `Quit` action gracefully stops `KeelRuntime` and exits.
- **Loopback DNS-Rebinding Defense**: Binds exclusively to `127.0.0.1` and enforces strict `Host` and `Origin` allowlists to block cross-site DNS-rebinding attempts.
- **Human-in-the-Loop `ConfirmRequired` Approval Gate**: Privileged MCP calls suspend cleanly on a `tokio::sync::oneshot` channel. If KeelDesk is minimized to the tray, it wakes and focuses the main window to surface an `AlertDialog` confirmation prompt supporting **Approve Once**, **Allow Session** allowlisting, **Deny**, immediate caller-cancellation cleanup (`RequestContext.ct`), and automatic timeout rejection (`60s` default).
- **Windows 11 Native Host Inspection**: Launches toolchain probes concurrently with `CREATE_NO_WINDOW` (zero console flash) and enumerates listening TCPv4/TCPv6 ports and full executable paths via Win32 `GetExtendedTcpTable` and `QueryFullProcessImageNameW`.
- **Windows 11 On-Device Agent Registry (`odr.exe`)**: Probes OS build, Developer Mode, and `odr.exe` readiness, supports one-click URI or manifest enrollment (`odr.exe mcp add`), and generates live MCPB `"0.3"` manifests with `_meta["com.microsoft.windows"].static_responses` for direct inspection or fallback file export.
- **Supervised Child MCP Process Host**: Spawns external stdio MCP servers (`npx`, `uvx`, `.cmd`, executables) with `CREATE_NO_WINDOW`, merged `HKLM`/`HKCU` Registry `PATH` resolution, Win32 Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) process-tree reclamation, live `rmcp` stdio handshake & tool discovery, interactive `tools/call` test invocation, redacted `env_keys`, and bounded `stderr_tail` logs.
- **Standardized React SDK & `<KeelDevTools />` Protocol Inspector**: Exports reusable `useMcpServerState`, `useToolApproval`, `useExternalMcpClients`, `useWinOdr`, and `useKeelDevTools` hooks from `src/keel-sdk/`, paired with the collapsible `<KeelDevTools />` inspector drawer (`Ctrl+\``) featuring live JSON-RPC request/response wells, `approval_wait_ms` vs `execution_ms` latency breakdowns, Streamable HTTP session tracking, `/healthz` probing, and one-click trace jumps from the call stream or child tool runner.
- **End-to-End Loopback Self-Test & Dry-Run Drill**: Ships with an in-app loopback client powered by Windows SChannel TLS (`reqwest` `native-tls`) to run one-click `keel_ping` connectivity checks and `kill_port_process` (`dry_run: true`) approval drills.
- **The Graphite Spine Design System**: Preserves authentic Windows 11 native window chrome (`decorations: true`, `DWMWA_CAPTION_COLOR` pixel-locked to `#111116`), a constrained `100dvh` desktop frame with `<ScrollArea>` containment, unified six-step typography, `4px` spacing / `12px` plumb-line alignment, concentric `2px` border radii, and Windows High Contrast (`forced-colors`) support. See [`PRODUCT.md`](PRODUCT.md), [`DESIGN.md`](DESIGN.md), and [`AGENTS.md`](AGENTS.md).

## Built-in MCP Tools

| Tool Name | Layer | Safety Tier | Description |
| :--- | :--- | :--- | :--- |
| `keel_ping` | `keel_core` | `Safe` | Verifies local Streamable HTTP MCP connectivity and returns server runtime status and echo payload. |
| `scan_dev_env` | `workbench` | `Safe` | Concurrently probes Windows 11 developer toolchains (`git`, `node`, `pnpm`, `rustc`, `cargo`, `python`, `uv`, `pwsh`, `winget`, `wsl`) without console flashes, decoding UTF-16LE output and filtering `WindowsApps` stubs. |
| `list_active_ports` | `workbench` | `Safe` | Enumerates active listening TCPv4/TCPv6 ports, owning PIDs, process names, and full executable paths via Win32 `GetExtendedTcpTable`. |
| `kill_port_process` | `workbench` | `ConfirmRequired` | Terminates the process owning a listening TCP port, gated by desktop human approval, `expected_pid` TOCTOU validation, self/system process guards, and `dry_run` drill mode. |

## Getting Started

### Requirements

- **OS**: Windows 11 (Build 22000+, 24H2 recommended)
- **Toolchain**: Rust (`stable`), Node.js 20+, `pnpm`, and Microsoft Edge WebView2 Runtime

### Development & Build Commands

```bash
# Install frontend dependencies
pnpm install

# Launch Tauri + Vite development environment (with HMR and TS binding generation)
pnpm tauri dev

# Regenerate src/bindings.ts from Rust commands and events via tauri-specta
pnpm bindings

# Run production frontend build (pnpm bindings -> tsc -> vite build)
pnpm build

# Run Rust unit tests and embedded MCP HTTP loopback integration tests
cargo test --manifest-path src-tauri/Cargo.toml

# Build Windows 11 desktop installer bundles
pnpm tauri build
```

## Connecting AI Clients

While KeelDesk is running (or resident in the system tray), connect local MCP-compatible AI clients to the loopback endpoint:

### Claude Code CLI

```bash
claude mcp add --transport http keeldesk http://127.0.0.1:19527/mcp
```

### Cursor (`mcp.json`)

```json
{
  "mcpServers": {
    "keeldesk": {
      "url": "http://127.0.0.1:19527/mcp"
    }
  }
}
```

### PowerShell Health Probe

```powershell
Invoke-RestMethod -Uri "http://127.0.0.1:19527/healthz"
```

## Project Structure

```text
KeelDesk/
├── PRODUCT.md                         # Product positioning, anti-references, and accessibility targets
├── DESIGN.md                          # "The Graphite Spine" normative design system tokens & rules
├── AGENTS.md                          # Engineering boundaries, Windows 11 chrome, and Specta contracts
├── docs/
│   ├── ROADMAP.md                     # Architecture layers and Phase 1–5 roadmap (English)
│   └── ROADMAP.zh-CN.md               # Architecture layers and Phase 1–5 roadmap (Simplified Chinese)
├── src-tauri/
│   └── src/
│       ├── keel_core/                 # Spine: MCP HTTP server, approval gate, devtools, Win11 ODR & process host
│       │   ├── devtools.rs            # Bounded JSON-RPC protocol trace store & latency/session telemetry
│       │   ├── win32_env.rs           # Win32 Registry PATH merging, CREATE_NO_WINDOW & CLI decoding
│       │   ├── win_odr.rs             # Windows 11 odr.exe probing, enrollment & MCPB "0.3" manifest export
│       │   └── process_host.rs        # Supervised stdio child MCP host with Win32 Job Object & rmcp tool calls
│       ├── workbench/                 # Workbench: Win11 dev-env scanner, port inspector, host summary
│       ├── bin/export-bindings.rs     # tauri-specta TypeScript binding exporter binary
│       ├── win11.rs                   # Win32 DWM dark titlebar theming and Registry build detection
│       ├── tray.rs                    # System tray menu and background daemon lifecycle
│       └── lib.rs                     # Single-instance bootstrap and Specta command/event registration
└── src/
    ├── keel-sdk/                      # Frontend SDK: standardized hooks & <KeelDevTools /> inspector
    │   ├── useMcpServerState.ts       # Embedded MCP server status, active sessions, /healthz & self-test hook
    │   ├── useToolApproval.ts         # ConfirmRequired approval queue, session allowlist & drill hook
    │   ├── useExternalMcpClients.ts   # Supervised child MCP lifecycle, stderr stream & tool invocation hook
    │   ├── useWinOdr.ts               # Windows 11 ODR readiness, enrollment & manifest export hook
    │   ├── useKeelDevTools.ts         # Protocol trace capture, filtering, pause/resume & call linking hook
    │   └── KeelDevTools.tsx           # Collapsible JSON-RPC payload, latency & session inspector drawer (Ctrl+`)
    ├── styles/tokens.css              # Single-track dark graphite tokens & typography utilities
    ├── components/
    │   ├── ui/                        # shadcn/ui (radix-nova) primitives
    │   ├── ds/                        # KeelDesk domain design-system components
    │   ├── ToolApprovalDialog.tsx     # ConfirmRequired interactive desktop approval modal
    │   ├── OdrManifestSheet.tsx       # Windows 11 ODR readiness, enrollment & MCPB "0.3" manifest sheet
    │   ├── ProcessHostPanel.tsx       # Supervised stdio child MCP host table & tool/stderr drawer
    │   ├── PortsGovernancePanel.tsx   # Win32 listening port table & dry-run drill panel
    │   ├── DevEnvPanel.tsx            # Windows 11 local toolchain scanner panel
    │   └── WorkbenchSkeleton.tsx      # Stage 2 React boot skeleton
    ├── bindings.ts                    # Generated tauri-specta typed commands & events (do not edit)
    └── App.tsx                        # Main Windows 11 KeelDesk workbench shell
```

## Roadmap

Full architecture specifications and phase breakdowns live in [`docs/ROADMAP.md`](docs/ROADMAP.md):

| Milestone | Scope | Status |
| :--- | :--- | :--- |
| **Phase 1 — Runtime Spine** | One-way `keel_core` / `workbench` decoupling, single-instance tray daemon, embedded Streamable HTTP MCP server, and native Windows 11 workbench | ✅ Completed |
| **Phase 2 — Host Tools & Approval Gate** | Local dev-env & Win32 port governance tools plus tray-wakeup `ConfirmRequired` human-in-the-loop approval loop | ✅ Completed |
| **Design System — The Graphite Spine** | Unified dark-only DWM pixel-locked tokens, six-step desktop typography, `12px` plumb line, concentric radii, `ui/` + `ds/` layering, and two-stage boot skeleton | ✅ Completed |
| **Phase 3 — Windows 11 ODR & Process Host** | Native Windows Agent Registry (`odr.exe`) enrollment + MCPB `"0.3"` manifest generation and `CREATE_NO_WINDOW` + Job Object child MCP process hosting | ✅ Completed |
| **Phase 4 — React SDK & DevTools** | Standardized `useMcpServerState`, `useToolApproval`, `useExternalMcpClients`, `useKeelDevTools`, and `<KeelDevTools />` JSON-RPC protocol inspector (`Ctrl+\``) | ✅ Completed |
| **Phase 5 — Workspace Modularization** | Cargo Workspace + `pnpm` Monorepo split into independent core crates, React SDK, and workbench app | 📋 Planned |

## Contributing & Security

- Read [`CONTRIBUTING.md`](CONTRIBUTING.md) before opening a pull request—especially the one-way `keel_core` boundary, `pnpm bindings` workflow, and Graphite Spine UI rules.
- Community participation is governed by our [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
- To report a security issue, please follow [`SECURITY.md`](SECURITY.md) and submit a private advisory instead of opening a public issue.

## License

Released under the [MIT License](LICENSE). Copyright (c) 2026 Leon Zeng.
