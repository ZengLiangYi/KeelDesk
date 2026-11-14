**English** · [简体中文](ROADMAP.zh-CN.md)

# KeelDesk Architecture & Roadmap

> **Vision**: Build a secure, native **Windows 11** desktop MCP (Model Context Protocol) runtime spine and local developer environment workbench—allowing desktop apps to expose trusted system capabilities to local AI agents while gating destructive operations behind unmistakable human-in-the-loop confirmation.

---

## 1. Architectural Layering & One-Way Dependency Contract

```text
┌───────────────────────────────────────────────────────────────────┐
│                    Workbench Layer (Workbench App)                │
│  Rust: src-tauri/src/workbench/   │  React: src/App.tsx           │
│  - Win11 dev-env & port tools     │  - Native Win11 workbench UI  │
│  - Registers tools in registry    │  - Consumes keel-sdk hooks    │
└───────────────────────┬───────────┴──────────────┬────────────────┘
                        │ One-way dependency       │
                        ▼                          ▼
┌───────────────────────────────────────────────────────────────────┐
│                    Runtime Spine (Keel Spine)                     │
│  Rust: src-tauri/src/keel_core/   │  React: src/keel-sdk/         │
│  - Embedded rmcp + axum HTTP MCP  │  - useKeelRuntime() hooks     │
│  - Loopback DNS-rebinding guard   │  - Typed tauri-specta events  │
│  - Safe / ConfirmRequired tiers   │  - Tray lifecycle & shutdown  │
└───────────────────────────────────────────────────────────────────┘
```

**Core Engineering Principles**:
1. **Strict One-Way Boundary**: `src-tauri/src/keel_core/` and `src/keel-sdk/` contain zero domain-specific imports from `workbench/`, preserving clean extraction paths for standalone crates and packages.
2. **Windows 11 Native UX & The Graphite Spine**: Authentic Win32 window chrome (`decorations: true`, native Snap Layouts, system border), `DWMWA_CAPTION_COLOR` pixel-locked to `#111116`, a constrained `100dvh` frame (`overflow: clip`), `shadcn/ui` (`radix-nova`) + `ds/` components, Windows High Contrast (`forced-colors`) support, and a two-stage zero-flash boot skeleton (normative rules in [`DESIGN.md`](../DESIGN.md), [`PRODUCT.md`](../PRODUCT.md), and [`AGENTS.md`](../AGENTS.md)).

---

## 2. Roadmap (Phase 1 – Phase 5)

### ✅ Phase 1: Runtime Spine & Loopback Foundation

- **Goal**: Establish the one-way architectural boundary between `keel_core` and `workbench`, delivering a single-instance system-tray daemon hosting an embedded Streamable HTTP MCP server inside a native Windows 11 dark workbench.
- **Delivered**:
  - `src-tauri/src/keel_core/`: `KeelConfig` (default `127.0.0.1:19527/mcp`, `KEELDESK_MCP_PORT` override), `KeelMcpServer` built on `rmcp` v3.10 + `axum` v0.8, `Host`/`Origin` DNS-rebinding allowlist, `/healthz` endpoint, `keel_ping` tool, and embedded loopback self-test client (`reqwest` `native-tls`).
  - `src-tauri/src/workbench/`: Business tool registration entry point and Registry-based Windows 11 host build detection.
  - Single-instance lock (`tauri-plugin-single-instance`), hide-to-tray on window close, and clean tray `Quit` shutdown.
  - Win32 DWM dark titlebar theming (`src-tauri/src/win11.rs`) and typed `tauri-specta` command/event bindings.

---

### ✅ Phase 2: Host Tools & Human-in-the-Loop Approval Gate

- **Goal**: Leverage the core advantage of a desktop GUI runtime over headless CLI scripts—**real-time visual human confirmation for privileged local actions**.
- **Delivered**:
  1. **Local Dev Environment & Port Governance Tools (`workbench`)**:
     - `scan_dev_env` (`Safe`): Concurrently probes `git`, `node`, `pnpm`, `rustc`, `cargo`, `python`, `uv`, `pwsh`, `winget`, and `wsl` via `CREATE_NO_WINDOW` with UTF-16LE decoding and `WindowsApps` stub filtering.
     - `list_active_ports` (`Safe`): Enumerates listening TCPv4/TCPv6 ports, PIDs, and full image paths via Win32 `GetExtendedTcpTable` and `QueryFullProcessImageNameW`.
     - `kill_port_process` (`ConfirmRequired`): Protected by `expected_pid` TOCTOU validation, self/system PID safeguards, Win32 `TerminateProcess`, and `dry_run` simulation mode.
  2. **Asynchronous Approval Interceptor (`keel_core`)**:
     - `KeelMcpServer::call_tool` suspends `ConfirmRequired` invocations on a `tokio::sync::oneshot` channel, supporting `ApproveOnce`, `ApproveForSession`, `Deny`, timeout rejection, immediate caller cancellation (`RequestContext.ct`) cleanup, and automatic tray-to-foreground window focus.
  3. **Desktop Approval Modal & Three-Tab Workbench (`keel-sdk` & UI)**:
     - `useToolApproval()` hook, `ToolApprovalDialog` live countdown modal, and the Contract / Ports Governance / Local Env workbench tabs.

---

### ✅ Design System Foundation: "The Graphite Spine"

- **Goal**: Standardize every visual primitive, layout frame, and boot state across the desktop workbench.
- **Delivered**:
  - Single-track dark graphite ramp (`src/styles/tokens.css`) pixel-locked to Win32 `DWMWA_CAPTION_COLOR` (`#111116`), unified six-step typography scale, `4px` spacing grid with `12px` plumb-line alignment, concentric `2px` border radii, `src/components/ui/` normalization, `src/components/ds/` domain primitives (`StatusBadge`, `SafetyTierBadge`, `TruncatedText`, `CodeWell`, `CopyButton`, `MetricPill`, `EmptyState`), and synchronized Stage 1 (`index.html`) / Stage 2 (`WorkbenchSkeleton.tsx`) boot skeletons.

---

### ⏳ Phase 3: Windows 11 Native ODR & Process Host

- **Goal**: Integrate with the Windows 11 On-Device Agent Registry ecosystem and eliminate console flashes or orphaned child trees when hosting third-party MCP servers.
- **Planned Scope**:
  1. **`keel_core::win_odr` Module**:
     - Detect Windows 11 build capabilities, Developer Mode status, and system `odr.exe` availability.
     - Register/unregister KeelDesk MCP manifests in the Windows On-Device Agent Registry with clean fallback to standard client config exports when unavailable.
  2. **`keel_core::process_host` Child Process Supervisor**:
     - Spawn external `npx` / `uvx` / `.cmd` MCP servers with `CREATE_NO_WINDOW` (`0x08000000`) and full user-level `PATH` propagation.
     - Bind child processes to a **Windows Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`)** so the entire process tree is reclaimed by the kernel if KeelDesk exits or crashes.

---

### 📋 Phase 4: Standardized React SDK & `<KeelDevTools />`

- **Goal**: Productize reusable frontend hooks and an in-app MCP protocol inspector.
- **Planned Scope**:
  1. **`src/keel-sdk/` Standard Hooks**:
     - `useMcpServerState()`: Server status, bound port, active sessions, and health controls.
     - `useToolApproval()`: Pending approval queue, single-grant / session allowlist / deny controls.
     - `useExternalMcpClients()`: Lifecycle control, status telemetry, and stderr streams for hosted child MCP processes.
  2. **`<KeelDevTools />` Protocol Inspector Drawer**:
     - Collapsible in-app drawer displaying live JSON-RPC request/response payloads, latency breakdowns, and error diagnostics.

---

### 📋 Phase 5: Workspace Modularization

- **Goal**: Split the repository into a Cargo Workspace and `pnpm` Monorepo so the runtime spine, Tauri plugin, React SDK, and workbench app can be consumed independently:

```text
keeldesk/
├── crates/
│   ├── keeldesk-core/          # Pure Rust MCP runtime, approval gate, Win11 ODR & process host
│   └── tauri-plugin-keeldesk/  # Standard Tauri v2 plugin wrapper
├── packages/
│   └── react/                  # @keeldesk/react (Hooks + <KeelDevTools />)
└── apps/
    └── env-workbench/          # KeelDesk local developer environment workbench
```
