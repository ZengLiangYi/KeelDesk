# Security Policy

## Supported Versions

KeelDesk is currently in pre-1.0 development on Windows 11 (Build 22000+). Security fixes are applied to the latest `0.1.x` release and `main` branch.

| Version | Supported |
| :--- | :--- |
| `0.1.x` | ✅ |
| `< 0.1.0` | ❌ |

## Security Architecture & Threat Model

Because KeelDesk exposes host inspection and privileged local process controls over a local Streamable HTTP MCP endpoint, security boundaries are enforced in depth:

1. **Loopback-Only Binding**: The embedded `axum` + `rmcp` HTTP server binds strictly to `127.0.0.1` (default port `19527`, overridable via `KEELDESK_MCP_PORT`) and never listens on `0.0.0.0` or external network interfaces.
2. **DNS-Rebinding Defense (`Host` & `Origin` Allowlists)**: Incoming HTTP requests are validated against strict loopback `Host` (`localhost`, `127.0.0.1`, `::1`) and `Origin` allowlists to prevent cross-origin browser requests or DNS-rebinding attacks from reaching local MCP endpoints.
3. **Human-in-the-Loop Safety Tiers (`Safe` vs `ConfirmRequired`)**:
   - `Safe` tools (`keel_ping`, `scan_dev_env`, `list_active_ports`) perform read-only diagnostics without modifying host state.
   - `ConfirmRequired` tools (`kill_port_process`) suspend execution on a `tokio::sync::oneshot` channel, wake and focus the KeelDesk desktop window if hidden in the system tray, and require explicit human approval (**Approve Once** or **Allow Session**). Timeouts (`60s` default) and caller cancellations (`RequestContext.ct`) fail closed and deny execution.
4. **TOCTOU & Process Safeguards**: `kill_port_process` supports `expected_pid` validation to guard against time-of-check-to-time-of-use PID reuse, protects KeelDesk's own PID and critical Windows system processes from termination, and provides a `dry_run` verification mode.
5. **Local IPC Boundary**: Tauri IPC commands and events are statically typed via `tauri-specta`, and the MCP runtime spine (`keel_core`) remains isolated from `workbench` implementations.

## Reporting a Vulnerability

**Please do NOT report security vulnerabilities through public GitHub issues, discussions, or pull requests.**

Instead, please submit a report privately via **[GitHub Private Vulnerability Reporting](https://github.com/ZengLiangYi/KeelDesk/security/advisories/new)**.

When reporting, please include:
- A clear description of the vulnerability and affected component (`keel_core`, `workbench`, Win32 integration, or frontend/IPC).
- Windows 11 build (`winver`) and KeelDesk version or commit SHA.
- Minimal reproduction steps or proof-of-concept payload.
- Potential impact (e.g., DNS-rebinding bypass, approval gate bypass, arbitrary process termination, privilege escalation).

### Response Timeline

- **Acknowledgment**: Within **72 hours** of receiving your report.
- **Triage & Assessment**: Initial severity assessment and reproduction confirmation within **7 days**.
- **Coordinated Disclosure**: We will coordinate fix verification and advisory publication with you once a patch is ready for release.
