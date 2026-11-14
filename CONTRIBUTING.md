# Contributing to KeelDesk

Thank you for your interest in contributing to **KeelDesk**! Whether you are fixing a Win32 edge case, tightening MCP protocol handling, or polishing UI details, we welcome thoughtful contributions.

## 1. Platform Scope

**KeelDesk targets Windows 11 exclusively (Build 22000+)**.

We deliberately embrace authentic Windows 11 native integrations—Win32 DWM titlebar theming (`DwmSetWindowAttribute`), Registry host detection, IP Helper (`GetExtendedTcpTable`), `CREATE_NO_WINDOW` process control, Windows Job Objects, and Windows On-Device Agent Registry (`odr.exe`)—rather than lowest-common-denominator cross-platform abstractions. Unless explicitly scoped in [`docs/ROADMAP.md`](docs/ROADMAP.md), please do not introduce macOS/Linux fallback shims that compromise Windows 11 behavior.

## 2. Development Environment Setup

- **Operating System**: Windows 11 (Build 22000 or later; 24H2 recommended)
- **Rust Toolchain**: Stable Rust with `rustfmt` and `clippy` (`rustup component add rustfmt clippy`)
- **Node.js & Package Manager**: Node.js 20+ (CI uses Node 22) and `pnpm` (managed via `packageManager` in `package.json`; never use `npm` or `yarn` or commit `package-lock.json` / `yarn.lock`)
- **WebView2**: Microsoft Edge WebView2 Runtime (pre-installed on Windows 11)

```bash
git clone https://github.com/ZengLiangYi/KeelDesk.git
cd KeelDesk
pnpm install
pnpm tauri dev
```

## 3. Architecture Boundary Rule

KeelDesk enforces a **strict one-way dependency boundary** between its runtime spine and the desktop workbench:

- **Framework Spine (`src-tauri/src/keel_core/`, `src/keel-sdk/`)**: Embedded `rmcp` + `axum` Streamable HTTP server, safety registry (`Safe` / `ConfirmRequired`), `oneshot` approval gate, telemetry events, and React SDK hooks.
- **Workbench Application (`src-tauri/src/workbench/`, `src/App.tsx`, `src/components/`)**: Windows 11 host inspection, port governance, and desktop UI attached to `KeelToolRegistry`.

> **`keel_core` and `keel-sdk` must NEVER import or depend on `workbench` modules.**

When adding a new MCP tool:
1. Place generic runtime/protocol primitives in `keel_core`, and host/domain tools in `workbench`.
2. Assign an explicit `ToolSafetyTier` (`Safe` for read-only/side-effect-free queries; `ConfirmRequired` for any state-mutating or destructive operation).

## 4. TypeScript & `tauri-specta` Bindings Workflow

KeelDesk uses `tauri-specta` (`=2.0.0-rc.25`) to generate typed IPC bindings in `src/bindings.ts`.

- **Never hand-edit `src/bindings.ts`**, and do not write raw `invoke("...")` or untyped event listeners when a binding exists.
- When adding or modifying a Rust command or event:
  - Commands: annotate with `#[tauri::command]` and `#[specta::specta]`, prefer owned parameters (`String` over `&str`), and register in `collect_commands![...]`.
  - Events: derive `serde::Serialize`, `serde::Deserialize`, `specta::Type`, and `tauri_specta::Event`, register in `collect_events![...]`, and mount in `setup()`.
  - Regenerate bindings immediately with `pnpm bindings` and commit the updated `src/bindings.ts`.

## 5. Design System & Native Chrome Rules

Before modifying frontend UI or styling, read [`PRODUCT.md`](PRODUCT.md), [`DESIGN.md`](DESIGN.md), and [`AGENTS.md`](AGENTS.md):

- **Brand Identifier**: Always refer to the product as **`KeelDesk`** (never translate `KeelDesk` or `Keel` in localized copy).
- **Native Windows 11 Chrome**: Keep `"decorations": true`, `"transparent": false`, `"backgroundColor": "#111116"` in `src-tauri/tauri.conf.json`, and preserve `DWMWA_CAPTION_COLOR` (`0x00161111`) in `src-tauri/src/win11.rs`. Do not add custom HTML titlebars or override `DWMWA_BORDER_COLOR`.
- **The Graphite Spine Tokens**: Use only the six typography utilities (`text-title-lg`, `text-title`, `text-body`, `text-caption`, `text-micro`, `text-code`), `font-mono tabular-nums` for dynamic telemetry, the `4px` spacing / `12px` plumb-line grid, concentric `2px` border radii, `100dvh` viewport lock (`overflow: clip`), and `<TruncatedText>` instead of native HTML `title` attributes.
- **Keep `pnpm-lock.yaml` in sync** if dependencies change.

## 6. Pre-PR Verification Checklist

Run all checks locally before opening a Pull Request (these mirror `.github/workflows/ci.yml`):

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm bindings
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
```

Ensure `git status` shows no uncommitted drift in `src/bindings.ts` after running `pnpm bindings`.

## 7. Pull Request & Commit Guidelines

- Keep PRs focused on a single concern (e.g., one feature, bug fix, or documentation improvement).
- Use clear, descriptive commit messages (Conventional Commits style such as `feat:`, `fix:`, `docs:`, `chore:` is encouraged).
- Complete all sections of [`.github/PULL_REQUEST_TEMPLATE.md`](.github/PULL_REQUEST_TEMPLATE.md) and link any related issue.
- By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
