# `@keeldesk/env-workbench`

The native **Windows 11** developer environment and port governance desktop workbench built on [**KeelDesk**](../../README.md) (`keeldesk-core`, `tauri-plugin-keeldesk`, and `@keeldesk/react`).

## Overview

- **Tauri Host (`src-tauri/`)**:
  - Registers Windows 11 workbench MCP tools on `KeelToolRegistry`: `scan_dev_env` (`Safe`), `list_active_ports` (`Safe`), and `kill_port_process` (`ConfirmRequired`).
  - Configures single-instance execution (`tauri-plugin-single-instance`), system tray residency (`src/tray.rs`), and authentic Windows 11 dark window chrome (`src/win11.rs`, `DWMWA_CAPTION_COLOR = 0x00161111` pixel-locked to `#111116`).
  - Exports both plugin and workbench `tauri-specta` TypeScript bindings via `src/bin/export-bindings.rs`.
- **React 19 Workbench UI (`src/`)**:
  - Implements **The Graphite Spine** design system (`src/styles/tokens.css`, `src/components/ui/`, `src/components/ds/`) inside a `100dvh` viewport-locked desktop shell (`overflow: clip`) with synchronized Stage 1 (`index.html`) and Stage 2 (`WorkbenchSkeleton.tsx`) zero-flash boot skeletons.
  - Consumes `@keeldesk/react` hooks and mounts `<KeelDevTools />` (`Ctrl+\``) alongside `ToolApprovalDialog`, `OdrManifestSheet`, `ProcessHostPanel`, `PortsGovernancePanel`, and `DevEnvPanel`.

## Commands (from repository root)

```bash
pnpm tauri dev
pnpm bindings
pnpm typecheck
pnpm build
pnpm tauri build
```

## License

[MIT](../../LICENSE)
