# AGENTS.md

Guidance for contributors and coding agents working in this repository.

## Platform & Architecture Boundary

- **Brand Identity**: The project and product name is strictly **`KeelDesk`**. Never translate `KeelDesk` or `Keel` into Chinese in UI copy, ARIA labels, comments, or documentation.
- **KeelDesk targets Windows 11 exclusively** (Build 22000+). Prefer authentic Windows 11 native integrations (`Win32` DWM, Registry, Job Objects, Windows ODR `odr.exe`) over lowest-common-denominator abstractions unless explicitly requested.
- Maintain the strict one-way dependency boundary across the Cargo + `pnpm` workspace monorepo:
  - **Framework spine (`crates/keeldesk-core/`, `crates/tauri-plugin-keeldesk/`, `packages/react/`)**: Pure Rust embedded `rmcp` + `axum` Streamable HTTP MCP server, tool safety registry (`Safe` / `ConfirmRequired`), telemetry events, Tauri v2 plugin wrapper, and the `@keeldesk/react` hooks + `<KeelDevTools />` package. **None of `keeldesk-core`, `tauri-plugin-keeldesk`, or `packages/react` may import or depend on `apps/env-workbench` business modules.**
  - **Workbench application (`apps/env-workbench/src-tauri/src/workbench/`, `apps/env-workbench/src/App.tsx`, `apps/env-workbench/src/components/`)**: Windows 11 host inspection, dev-environment tools, and desktop UI. `workbench` attaches tools to `KeelToolRegistry`, and `apps/env-workbench` consumes `@keeldesk/react`.

## Project Stack

- Dual Cargo + `pnpm` workspace monorepo:
  - Rust 2021 workspace (`crates/keeldesk-core`, `crates/tauri-plugin-keeldesk`, `apps/env-workbench/src-tauri`), Tauri v2 desktop app (single-instance + system tray), `rmcp` v3.10, `axum` v0.8, `tokio`, `windows` v0.61.
  - `pnpm` workspace (`packages/react` `@keeldesk/react` and `apps/env-workbench` `@keeldesk/env-workbench`), React 19, TypeScript 5.8, Vite 7, Tailwind CSS v4 (`@tailwindcss/vite`), `shadcn/ui` (`radix-nova`), Lucide icons.
- Use `pnpm` for all JavaScript package operations. Do not introduce `npm`, `yarn`, `package-lock.json`, or `yarn.lock`.
- Keep `pnpm-lock.yaml` committed whenever dependencies change.

## Common Commands

```bash
pnpm install
pnpm tauri dev
pnpm bindings
pnpm typecheck
pnpm build
cargo test --workspace
pnpm tauri build
```

- `pnpm bindings` runs `cargo run -p keeldesk --bin export-bindings` to regenerate both `packages/react/src/bindings.ts` (plugin commands/events) and `apps/env-workbench/src/bindings.ts` (workbench commands).
- `pnpm typecheck` runs `tsc --noEmit` across `@keeldesk/react` and `@keeldesk/env-workbench`.
- `pnpm build` runs `pnpm bindings`, builds `@keeldesk/react` (`packages/react/dist/`), and then runs `tsc && vite build` for `@keeldesk/env-workbench`.
- Tauri's `beforeDevCommand` and `beforeBuildCommand` in `apps/env-workbench/src-tauri/tauri.conf.json` must continue to use `pnpm`.

## TypeScript & Tauri Specta Bindings

- Use `tauri-specta` (`=2.0.0-rc.25`) for typed Rust-to-TypeScript commands **and** events, split strictly across the workspace boundary:
  - Core plugin commands (`plugin:keeldesk|*`) and events (`plugin:keeldesk:*`) are exported into `packages/react/src/bindings.ts` by `tauri-plugin-keeldesk`. Import them from `@keeldesk/react` (`keelCommands`, `keelEvents`, or the standardized hooks).
  - App-specific workbench commands are exported into `apps/env-workbench/src/bindings.ts` by `apps/env-workbench/src-tauri`. Import them from `@/bindings` inside `apps/env-workbench`:

```ts
import { commands } from "@/bindings";
import { keelCommands, keelEvents } from "@keeldesk/react";
```

- Do not hand-write frontend calls like `invoke("command_name", ...)` or raw untyped event strings when a typed binding exists.
- `packages/react/src/bindings.ts` and `apps/env-workbench/src/bindings.ts` are generated. Never edit either file manually.
- When adding a new Rust command or event:
  - Commands: annotate with `#[tauri::command]` and `#[specta::specta]`, prefer owned inputs (`String` over `&str`), and register in `collect_commands![...]` in the appropriate crate (`crates/tauri-plugin-keeldesk` for core spine, `apps/env-workbench/src-tauri` for workbench).
  - Events: derive `serde::Serialize`, `serde::Deserialize`, `specta::Type`, and `tauri_specta::Event`, register in `collect_events![...]`, and keep `specta_builder.mount_events(...)` active.
  - Run `pnpm bindings` and verify `pnpm typecheck` passes.
- Keep Specta RC dependencies version-pinned with `=` unless intentionally upgrading the binding stack.

## Windows 11 Native Window Chrome Rules

- KeelDesk uses **authentic Windows 11 native window chrome** rather than custom HTML titlebars or transparent frameless overlays.
- `apps/env-workbench/src-tauri/tauri.conf.json` must keep:
  - `"decorations": true`
  - `"transparent": false`
  - `"backgroundColor": "#111116"`
- This preserves native Windows 11 window dragging, Snap Layouts flyout on the Maximize button, native caption glyphs, and the **Windows 11 system-default outer window border**.
- In `apps/env-workbench/src-tauri/src/win11.rs`, theme the native titlebar with `DwmSetWindowAttribute`:
  - `DWMWA_USE_IMMERSIVE_DARK_MODE` (`1`)
  - `DWMWA_CAPTION_COLOR` (`0x00161111`, matching `.dark` `--background: #111116` pixel-for-pixel)
  - `DWMWA_TEXT_COLOR` (`0x00F5F2F2`)
- **Do not** override `DWMWA_BORDER_COLOR` with a custom color, and **do not** reintroduce frameless drag handles or custom HTML Min/Max/Close buttons unless explicitly requested.
- Closing the main window hides it to the system tray (`apps/env-workbench/src-tauri/src/tray.rs`) while the embedded MCP server continues running; only the tray `Quit` action shuts down `KeelRuntime` and exits the process.

## KeelDesk Design System ("The Graphite Spine" — `shadcn/ui` + `modern-web-guidance`)

Read [`PRODUCT.md`](PRODUCT.md) and [`DESIGN.md`](DESIGN.md) before designing or refactoring UI. Consult the `modern-web-guidance` / `impeccable` skills (when available) and enforce these unified system hierarchies across `apps/env-workbench/src/styles/tokens.css`, `apps/env-workbench/src/components/ui/`, `apps/env-workbench/src/components/ds/`, and `packages/react/src/`:

1. **Dark-Only Graphite Ramp & `color-scheme` Contract**:
   - Keep `<html lang="zh-CN" class="dark">` and `<meta name="color-scheme" content="dark" />` in `apps/env-workbench/index.html`, backed by single-track dark tokens in `apps/env-workbench/src/styles/tokens.css` (`--surface-sunken: #0b0b0f`, `--surface-canvas: #111116` pixel-locked to Win32 `DWMWA_CAPTION_COLOR`, `--surface-subtle: #14141b`, `--surface-panel: #16161d`, `--surface-overlay: #1c1c25`).
   - Keep `@source "../../../packages/react/src/**/*.{ts,tsx}";` in `apps/env-workbench/src/index.css` so Tailwind CSS v4 scans `@keeldesk/react` components such as `<KeelDevTools />`.
   - Maintain ≥95% monochrome Cursor/Vercel restraint; reserve `--primary` (`#7c66ff` Keel Indigo) for active focus/primary CTAs (≤5%) and `--status-{ok,warn,danger,info}` strictly for live telemetry states.
2. **Unified Six-Step Desktop Typography Hierarchy**:
   - Use exclusively the six semantic Tailwind v4 type utilities: `text-title-lg` (`18px/24px`), `text-title` (`15px/22px`), `text-body` (`13px/20px` desktop baseline), `text-caption` (`12px/16px`), `text-micro` (`11px/16px` strict legibility floor), and `text-code` (`12px/20px` `Cascadia Code`).
   - **Prohibited**: Arbitrary bracketed font sizes (`text-[10px]`, `text-[11px]`, `text-[0.8rem]`) and fluid `clamp()` scales. Always pair dynamic numbers/ports/PIDs/latencies/timers with `font-mono tabular-nums`.
3. **Unified `4px` Spacing Grid, Control Heights, & `12px` Plumb Line**:
   - Align `CardHeader`, filter toolbars, outer `Table` columns (`pl-3` / `pr-3`), and `CardFooter` on the unified `12px` (`--space-card-pad` / `px-3`) vertical plumb line (`20px` inside modals).
   - Standardize interactive control heights to `xs: 24px` (`h-6`), `sm: 28px` (`h-7`), `md: 32px` (`h-8`), and `lg: 36px` (`h-9`). Use `gap-*` (never `space-x-*`/`space-y-*`).
4. **Unified Concentric `2px` Border-Radius Hierarchy**:
   - Step nested radii down concentrically: floating modals/sheets `rounded-xl` (`12px`) -> primary `Card` panels `rounded-lg` (`10px`) -> nested `CodeWell` / `Alert` `rounded-md` (`8px`) -> interactive `Button` / `Input` / menu items `rounded-sm` (`6px`) -> technical `Badge` / `Kbd` / `MetricPill` `rounded-xs` (`4px`). Reserve `rounded-full` strictly for circular `StatusDot` and `Switch` elements; never use `rounded-[min(...)]` formulas or bubbly pill text badges.
5. **Constrained `100dvh` Viewport, Scroll Containment & Two-Stage Skeleton**:
   - Lock `html, body, #root` and the outer desktop shell to `100dvh` (`h-dvh`) with `overflow: clip`. Place `min-h-0 min-w-0` on every nested Flex/Grid/Tabs track, wrap scrollable inner lists/tables in `<ScrollArea>`, and keep `overscroll-behavior: contain` + WCAG ≥ 3.5:1 `scrollbar-color`.
   - Keep Stage 1 (`apps/env-workbench/index.html` `#keel-critical-boot`) and Stage 2 (`apps/env-workbench/src/components/WorkbenchSkeleton.tsx`) geometry and hex tokens synchronized whenever canvas/panel tokens change.
6. **Component Layering (`ui/` + `ds/`) & Accessibility (`forced-colors`)**:
   - Compose `shadcn/ui` (`radix-nova`) primitives in `apps/env-workbench/src/components/ui/` and reusable KeelDesk domain patterns in `apps/env-workbench/src/components/ds/` (`StatusDot`, `StatusBadge`, `SafetyTierBadge`, `TruncatedText`, `CodeWell`, `CopyButton`, `MetricPill`, `EmptyState`). Verify `cn` imports come from `@/lib/utils` (and `./lib/utils` inside `packages/react`).
   - Never use HTML `title` attributes — use `<TruncatedText>` / `<Tooltip>` (`TooltipProvider` mounted once at root). Announce copy/drill transitions via `aria-live="polite"`, respect `prefers-reduced-motion: reduce`, and preserve border-based active indicators under Windows High Contrast (`forced-colors: active`).

## Verification

Before considering a change complete, run:

```bash
pnpm bindings
pnpm typecheck
pnpm build
cargo test --workspace
```
