# AGENTS.md

Guidance for contributors and coding agents working in this repository.

## Platform & Architecture Boundary

- **KeelDesk targets Windows 11 exclusively** (Build 22000+). Prefer authentic Windows 11 native integrations (`Win32` DWM, Registry, Job Objects, Windows ODR `odr.exe`) over lowest-common-denominator abstractions unless explicitly requested.
- Maintain the strict one-way dependency boundary between the MCP runtime spine and the desktop workbench:
  - **Framework spine (`src-tauri/src/keel_core/`, `src/keel-sdk/`)**: Embedded `rmcp` + `axum` Streamable HTTP MCP server, tool safety registry (`Safe` / `ConfirmRequired`), telemetry events, and React hooks. **`keel_core` and `keel-sdk` must never import or depend on `workbench` business modules.**
  - **Workbench application (`src-tauri/src/workbench/`, `src/App.tsx`, `src/components/`)**: Windows 11 host inspection, dev-environment tools, and desktop UI. `workbench` attaches tools to `KeelToolRegistry`.

## Project Stack

- Tauri v2 desktop app (single-instance + system tray), Rust 2021, `rmcp` v3.10, `axum` v0.8, `tokio`, `windows` v0.61.
- React 19, TypeScript 5.8, Vite 7, Tailwind CSS v4 (`@tailwindcss/vite`), `shadcn/ui` (`radix-nova`), Lucide icons, and `pnpm`.
- Use `pnpm` for all JavaScript package operations. Do not introduce `npm`, `yarn`, `package-lock.json`, or `yarn.lock`.
- Keep `pnpm-lock.yaml` committed whenever dependencies change.

## Common Commands

```bash
pnpm install
pnpm tauri dev
pnpm bindings
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
pnpm tauri build
```

- `pnpm bindings` runs the `export-bindings` Rust binary to regenerate `src/bindings.ts`.
- `pnpm build` runs `pnpm bindings` before `tsc` and `vite build`.
- Tauri's `beforeDevCommand` and `beforeBuildCommand` must continue to use `pnpm`.

## TypeScript & Tauri Specta Bindings

- Use `tauri-specta` (`=2.0.0-rc.25`) for typed Rust-to-TypeScript commands **and** events.
- Do not hand-write frontend calls like `invoke("command_name", ...)` or raw untyped event strings when a typed binding exists.
- Import generated commands and events from `@/bindings`:

```ts
import { commands, events } from "@/bindings";
```

- `src/bindings.ts` is generated. Never edit it manually.
- When adding a new Rust command or event:
  - Commands: annotate with `#[tauri::command]` and `#[specta::specta]`, prefer owned inputs (`String` over `&str`), and register in `collect_commands![...]`.
  - Events: derive `serde::Serialize`, `serde::Deserialize`, `specta::Type`, and `tauri_specta::Event`, register in `collect_events![...]`, and keep `specta_builder.mount_events(app)` in `setup()`.
  - Run `pnpm bindings` and verify `tsc` passes.
- Keep Specta RC dependencies version-pinned with `=` unless intentionally upgrading the binding stack.

## Windows 11 Native Window Chrome Rules

- KeelDesk uses **authentic Windows 11 native window chrome** rather than custom HTML titlebars or transparent frameless overlays.
- `src-tauri/tauri.conf.json` must keep:
  - `"decorations": true`
  - `"transparent": false`
  - `"backgroundColor": "#111116"`
- This preserves native Windows 11 window dragging, Snap Layouts flyout on the Maximize button, native caption glyphs, and the **Windows 11 system-default outer window border**.
- In `src-tauri/src/win11.rs`, theme the native titlebar with `DwmSetWindowAttribute`:
  - `DWMWA_USE_IMMERSIVE_DARK_MODE` (`1`)
  - `DWMWA_CAPTION_COLOR` (`0x00161111`, matching `.dark` `--background: #111116` pixel-for-pixel)
  - `DWMWA_TEXT_COLOR` (`0x00F5F2F2`)
- **Do not** override `DWMWA_BORDER_COLOR` with a custom color, and **do not** reintroduce frameless drag handles or custom HTML Min/Max/Close buttons unless explicitly requested.
- Closing the main window hides it to the system tray (`tray.rs`) while the embedded MCP server continues running; only the tray `Quit` action shuts down `KeelRuntime` and exits the process.

## KeelDesk Design System ("The Graphite Spine" — `shadcn/ui` + `modern-web-guidance`)

Read [`PRODUCT.md`](PRODUCT.md) and [`DESIGN.md`](DESIGN.md) before designing or refactoring UI. Consult the `modern-web-guidance` / `impeccable` skills (when available) and enforce these unified system hierarchies across `src/styles/tokens.css`, `src/components/ui/`, and `src/components/ds/`:

1. **Dark-Only Graphite Ramp & `color-scheme` Contract**:
   - Keep `<html lang="zh-CN" class="dark">` and `<meta name="color-scheme" content="dark" />` in `index.html`, backed by single-track dark tokens in `src/styles/tokens.css` (`--surface-sunken: #0b0b0f`, `--surface-canvas: #111116` pixel-locked to Win32 `DWMWA_CAPTION_COLOR`, `--surface-subtle: #14141b`, `--surface-panel: #16161d`, `--surface-overlay: #1c1c25`).
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
   - Keep Stage 1 (`index.html` `#keel-critical-boot`) and Stage 2 (`src/components/WorkbenchSkeleton.tsx`) geometry and hex tokens synchronized whenever canvas/panel tokens change.
6. **Component Layering (`ui/` + `ds/`) & Accessibility (`forced-colors`)**:
   - Compose `shadcn/ui` (`radix-nova`) primitives in `src/components/ui/` and reusable KeelDesk domain patterns in `src/components/ds/` (`StatusDot`, `StatusBadge`, `SafetyTierBadge`, `TruncatedText`, `CodeWell`, `CopyButton`, `MetricPill`, `EmptyState`). Verify `cn` imports come from `@/lib/utils`.
   - Never use HTML `title` attributes — use `<TruncatedText>` / `<Tooltip>` (`TooltipProvider` mounted once at root). Announce copy/drill transitions via `aria-live="polite"`, respect `prefers-reduced-motion: reduce`, and preserve border-based active indicators under Windows High Contrast (`forced-colors: active`).

## Verification

Before considering a change complete, run:

```bash
pnpm bindings
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
```
