# `@keeldesk/react`

Standalone React 19 hooks, typed `tauri-specta` bindings, and the `<KeelDevTools />` JSON-RPC protocol inspector drawer for [**KeelDesk**](../../README.md) Tauri v2 applications on Windows 11.

Pairs with [`tauri-plugin-keeldesk`](../../crates/tauri-plugin-keeldesk/README.md).

## Exported Hooks & Components

- **`useMcpServerState()`** (`useKeelRuntime` alias): Tracks embedded Streamable HTTP MCP server status, bound port, `activeSessions`, `/healthz` readiness probes (`checkHealth`), and loopback `keel_ping` self-tests.
- **`useToolApproval()`**: Subscribes to `ConfirmRequired` tool approval requests with **Approve Once**, **Allow Session**, **Deny**, session allowlist clearing, and dry-run approval drills.
- **`useExternalMcpClients()`** (`useProcessHost` alias): Manages supervised stdio child MCP servers, redacted `env_keys` snapshots, live `stderrStream`, built-in echo probe specs (`buildBuiltinEchoProbeSpec`), and interactive `callClientTool()` invocations.
- **`useWinOdr()`**: Probes Windows 11 `odr.exe` readiness, enrolls/unenrolls MCP servers, and exports MCPB `"0.3"` manifests.
- **`useKeelDevTools()` & `<KeelDevTools />`**: Real-time JSON-RPC 2.0 frame inspector (`Ctrl+\``) featuring split request/response wells, `approval_wait_ms` vs `execution_ms` latency bars, transport/status/search filters, pause/resume buffering, and `selectByLinkedCallId()` jump-to-frame navigation.
- **`keelCommands` & `keelEvents`**: Generated `tauri-specta` bindings targeting `plugin:keeldesk`.

## Tailwind CSS v4 Setup

When consuming `<KeelDevTools />` in a Tailwind CSS v4 host app aligned with [The Graphite Spine](../../DESIGN.md) tokens, register the package source path in your stylesheet:

```css
@import "tailwindcss";
@source "../../../packages/react/src/**/*.{ts,tsx}";
```

## Build & Typecheck

```bash
pnpm --filter @keeldesk/react typecheck
pnpm --filter @keeldesk/react build
```

## License

[MIT](../../LICENSE)
