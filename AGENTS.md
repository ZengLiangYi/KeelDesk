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

## `shadcn/ui` + `modern-web-guidance` UI Specification

Consult the `modern-web-guidance` skill (when available in your agent environment) and follow these mandatory desktop web standards whenever implementing or refactoring HTML, CSS layout, scrollbars, skeletons, accessibility, or client-side interactions:

1. **Root Dark-Mode & `color-scheme` Contract**:
   - Keep `<html lang="zh-CN" class="dark">` and `<meta name="color-scheme" content="dark" />` in `index.html`, paired with `:root { color-scheme: dark; }` and `.dark` tokens in `src/index.css`.
   - This guarantees WebView2 paints root surfaces, Radix `document.body` portals, native form controls, and fallback scrollbars in dark mode from frame 0.
2. **Constrained Desktop Viewport & Zero Page Scrollbars**:
   - Lock `html, body, #root` (`src/App.css`) and the outer desktop shell to `block-size: 100dvh` (`h-dvh`) with `overflow: clip`.
   - Always place `min-h-0` and `min-w-0` on nested Flex/Grid tracks so child scroll containers shrink within the window rather than triggering a browser-level scrollbar.
3. **Scroll Containment & Scrollbar Contrast**:
   - Wrap scrollable inner lists and tables in `shadcn/ui`'s `<ScrollArea>`.
   - Apply `overscroll-behavior: contain` on nested scrollports and maintain WCAG ≥ 3.5:1 scrollbar thumb contrast (`scrollbar-color`) against dark card surfaces.
4. **Two-Stage Zero-Flash Boot Skeleton**:
   - **Stage 1 (`index.html`)**: Inline `<style id="keel-critical-boot">` + pure HTML 3-card skeleton (`role="progressbar"`) rendered immediately at 0ms before JS/CSS bundles finish loading, and removed on React mount; honor `prefers-reduced-motion: reduce`.
   - **Stage 2 (`src/components/WorkbenchSkeleton.tsx`)**: Geometry-matched `grid-cols-12` (`col-span-7` / `col-span-5`) skeleton built with `shadcn/ui` `Card` + `Skeleton` (`motion-safe:animate-pulse`) displayed while initial Tauri IPC snapshots resolve.
5. **Genuine `shadcn/ui` (`radix-nova`) Discipline**:
   - Check `pnpm dlx shadcn@latest info --json` before assuming a component is installed; add new components via `pnpm dlx shadcn@latest add <component>`.
   - After running `shadcn add`, verify generated files in `src/components/ui/` import `cn` from `@/lib/utils` (and remove any spurious `"cn"` npm package if added by upstream registry quirks).
   - Prefer `shadcn/ui` primitives (`Card`, `Tabs`, `Table`, `ScrollArea`, `Tooltip`, `Alert`, `Skeleton`, `Badge`, `Button`, `Input`, `Separator`) over ad-hoc styled `<div>` panels.
   - Use semantic tokens (`bg-background`, `bg-card`, `text-foreground`, `text-muted-foreground`, `border-border`), `gap-*` instead of `space-x-*`/`space-y-*`, `size-*` when width equals height, `tabular-nums` on dynamic metrics/ports/latencies, and `data-icon="inline-start"` / `data-icon="inline-end"` on Lucide icons inside `Button`.
6. **Accessibility & Windows High Contrast (`forced-colors`)**:
   - Do not rely on native HTML `title` attributes for truncated descriptions or status badges — use keyboard-accessible `shadcn/ui` `<Tooltip>` (`TooltipProvider` mounted once at the app root) with `aria-hidden="true"` when tooltip text duplicates existing accessible content.
   - Announce clipboard copy state transitions and runtime self-test results via `aria-live="polite"`.
   - Prefer border-based active indicators (`border-block-end`) over background-painted pseudo-elements so active tab states survive Windows High Contrast (`forced-colors: active`).

## Verification

Before considering a change complete, run:

```bash
pnpm bindings
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
```
